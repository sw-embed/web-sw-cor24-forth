//! Measurements, not checks (all #[ignore]): instructions each kernel
//! really spends, with input fed the moment the kernel polls for it, so
//! UART pacing does not inflate the count. The numbers in sw-cor24-forth's
//! forth-from-forth/docs/kernel-sizes.md come from here. Run:
//!   cargo test --release --test boot_cost -- --ignored --nocapture

use cor24_assembler::Assembler;
use cor24_emulator::EmulatorCore;
use web_sw_cor24_forth::config::ForthTier;
use web_sw_cor24_forth::demos::{
    FFF_CORE_FILES, FFF_KERNEL_SRC, FIF_CORE_FILES, FIF_KERNEL_SRC, FOF_CORE_FILES, FOF_KERNEL_SRC,
};
use web_sw_cor24_forth::pump::{Pump, UART_STATUS_ADDR};

struct Run {
    emu: EmulatorCore,
    pump: Pump,
    idle_instr: u64,
}

impl Run {
    fn new(src: &str) -> Run {
        let mut asm = Assembler::new();
        let r = asm.assemble(src);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let mut pump = Pump::new();
        pump.reset(&r.labels);
        let mut emu = EmulatorCore::new();
        emu.hard_reset();
        emu.load_program(0, &r.bytes);
        emu.load_program_extent(r.bytes.len() as u32);
        emu.set_pc(0);
        emu.resume();
        Run {
            emu,
            pump,
            idle_instr: 0,
        }
    }

    /// Run until the kernel is parked in a poll loop with nothing left to
    /// feed. Bytes are handed over as soon as the UART is free; while the
    /// PC is in a poll loop only 16 instructions run at a time, and those
    /// are counted as waiting, not work.
    fn drain(&mut self) {
        loop {
            let pc = self.emu.snapshot().pc;
            let idle = self.pump.is_idle(pc);
            if idle && self.pump.queue.is_empty() {
                let r = self.emu.run_batch(64);
                self.idle_instr += r.instructions_run;
                if self.pump.is_idle(self.emu.snapshot().pc) {
                    return;
                }
                continue;
            }
            if idle && self.emu.read_byte(UART_STATUS_ADDR) & 1 == 0 {
                let b = self.pump.queue.pop_front().unwrap();
                self.emu.send_uart_byte(b);
            }
            let n = if idle { 16 } else { 50 };
            let r = self.emu.run_batch(n);
            if idle {
                self.idle_instr += r.instructions_run;
            }
            assert!(r.instructions_run > 0, "kernel stopped: {:?}", r.reason);
        }
    }
}

fn measure(name: &str, src: &str, core: &[(&str, &str)]) {
    let mut run = Run::new(src);
    run.pump.queue_core_files(core);
    let t0 = std::time::Instant::now();
    run.drain();
    let boot_total = run.emu.instructions_count();
    let boot_idle = run.idle_instr;
    run.emu.clear_uart_output();
    run.pump.queue_line("7777 .");
    run.drain();
    let line_total = run.emu.instructions_count() - boot_total;
    let line_idle = run.idle_instr - boot_idle;
    println!(
        "{name:18} boot: {:>12} work + {:>9} waiting   `7777 .`: {:>9} work + {:>7} waiting   out={:?}   ({:?})",
        boot_total - boot_idle,
        boot_idle,
        line_total - line_idle,
        line_idle,
        run.emu.get_uart_output().trim(),
        t0.elapsed()
    );
}

fn lines(name: &str, src: &str, core: &[(&str, &str)]) {
    let mut run = Run::new(src);
    run.pump.queue_core_files(core);
    run.drain();
    for l in [
        "",
        "DROP",
        "7",
        "7777",
        "7777 DROP",
        "7777 .",
        "7 .",
        "QUIT-VECTOR DROP",
        "DUP DROP",
    ] {
        let before = run.emu.instructions_count();
        let idle_before = run.idle_instr;
        run.pump.queue_line(if l == "DROP" { "1 DROP" } else { l });
        run.drain();
        let work = run.emu.instructions_count() - before - (run.idle_instr - idle_before);
        println!(
            "{name:18} {:>18?}: {:>8} work",
            if l == "DROP" { "1 DROP" } else { l },
            work
        );
        run.emu.clear_uart_output();
        run.pump.queue_line("DEPTH 0 = 0= IF DROP THEN");
        run.drain();
    }
}

#[test]
#[ignore]
fn rebuild_cost() {
    let fff = "../sw-cor24-forth/forth-from-forth";
    let rd = |f: &str| std::fs::read_to_string(format!("{fff}/{f}")).unwrap();
    let core: String = ["runtime", "minimal", "lowlevel", "midlevel", "highlevel"]
        .iter()
        .map(|t| rd(&format!("core/{t}.fth")))
        .collect::<Vec<_>>()
        .join("\n")
        .lines()
        .filter(|l| *l != "' QUIT QUIT-VECTOR !" && *l != "QUIT")
        .collect::<Vec<_>>()
        .join("\n");
    let input = format!(
        "{}\nXC-KERNEL-BEGIN\n{}\n{}\nXC-COMPILE-SOURCE\n{}\nXC-END\n",
        rd("compiler/xcomp.fth"),
        rd("core/prims.fth"),
        rd("core/prims-asm.fth"),
        core
    );
    let mut run = Run::new(FFF_KERNEL_SRC);
    run.drain();
    let before = run.emu.instructions_count();
    run.pump.queue.extend(input.bytes().filter(|&b| b != b'\r'));
    let t0 = std::time::Instant::now();
    run.drain();
    let out = run.emu.get_uart_output().to_string();
    let a = out.find("!!BEGIN-KERNEL!!").unwrap();
    let b = out.find("!!END-KERNEL!!").unwrap();
    let body: Vec<&str> = out[a..b].lines().skip(1).filter(|l| *l != " ok").collect();
    let committed = std::fs::read_to_string(format!("{fff}/kernel.s")).unwrap();
    let committed_body: Vec<&str> = committed.lines().skip(4).collect();
    println!(
        "rebuild: {} input bytes, {} instructions ({} waiting), {:?} native; {} lines out; identical to kernel.s: {}",
        input.len(),
        run.emu.instructions_count() - before,
        run.idle_instr,
        t0.elapsed(),
        body.len(),
        body == committed_body
    );
}

#[test]
#[ignore]
fn line_cost() {
    lines("forth-in-forth", FIF_KERNEL_SRC, FIF_CORE_FILES);
    lines("forth-from-forth", FFF_KERNEL_SRC, FFF_CORE_FILES);
}

#[test]
#[ignore]
fn boot_cost() {
    measure("forth.s", ForthTier::Interpreter.assembly(), &[]);
    measure("forth-in-forth", FIF_KERNEL_SRC, FIF_CORE_FILES);
    measure("forth-on-forthish", FOF_KERNEL_SRC, FOF_CORE_FILES);
    measure("forth-from-forth", FFF_KERNEL_SRC, FFF_CORE_FILES);
}
