//! Headless regression tests for the hardware demos (LED D2 / switch S2).
//!
//! Every kernel the site ships is booted exactly the way the browser boots
//! it — same assembler, same core/*.fth queue, same `Pump` loop — and the
//! shipped demo sources are pushed through it with the emulated button
//! toggled mid-run. No browser, no Yew, no timers.
//!
//! What this catches, in the order it was learned the hard way:
//!   * a demo that leaks the data stack (the MIRROR loop of
//!     `13-loop-switch-led.fth` once leaked two cells per iteration and
//!     silently overflowed the 3 KB stack, halting the CPU with the LED
//!     frozen — "toggling the switch does nothing");
//!   * a kernel whose `SW?` / `LED!` stop reaching the I/O register;
//!   * a kernel that adds a UART busy-wait loop without listing its label
//!     in `pump::POLL_LABELS` (the REPL would starve it of bytes);
//!   * a pump-loop change that stops re-reading the button register.
//!
//! Boot is the expensive part (tens of millions of instructions), so each
//! kernel is booted once and cloned per test.

use cor24_assembler::Assembler;
use cor24_emulator::EmulatorCore;
use std::sync::OnceLock;
use web_sw_cor24_forth::config::ForthTier;
use web_sw_cor24_forth::demos::{
    Demo, FIF_CORE_FILES, FIF_DEMOS, FIF_KERNEL_SRC, FOF_CORE_FILES, FOF_DEMOS, FOF_KERNEL_SRC,
    FORTH_S_DEMOS,
};
use web_sw_cor24_forth::pump::{BATCH_SIZE, Pump};

/// Cell size in bytes (24-bit words).
const CELL: u32 = 3;
/// Data stack must never dip more than this many cells below its idle
/// level while a demo runs. The MIRROR loop legitimately holds ~4 cells
/// (counter, limit, inner counter, flag); a leak of 2 cells/iteration blows
/// through this within a few iterations.
const MAX_STACK_DIP_CELLS: u32 = 8;
/// After S2 changes, the LED must follow within this many ticks. One tick
/// is BATCH_SIZE instructions; the MIRROR loop body is a few thousand.
const LED_FOLLOW_TICKS: usize = 10;

/// Which tab's driving loop to replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loop {
    /// Tab 1 (`src/debugger.rs`): one byte per tick, flat batch.
    Flat,
    /// Tabs 2 & 3 (`src/repl.rs`): adaptive pump.
    Adaptive,
}

/// A booted kernel: emulator + pump, ready for interactive input.
#[derive(Clone)]
struct Booted {
    emu: EmulatorCore,
    pump: Pump,
}

struct Kernel {
    name: &'static str,
    kernel_src: &'static str,
    core_files: &'static [(&'static str, &'static str)],
    demos: &'static [Demo],
    driver: Loop,
    cell: OnceLock<Booted>,
}

static FORTH_S: Kernel = Kernel {
    name: "forth.s",
    kernel_src: ForthTier::Interpreter.assembly(),
    core_files: &[],
    demos: FORTH_S_DEMOS,
    driver: Loop::Flat,
    cell: OnceLock::new(),
};
static FIF: Kernel = Kernel {
    name: "forth-in-forth",
    kernel_src: FIF_KERNEL_SRC,
    core_files: FIF_CORE_FILES,
    demos: FIF_DEMOS,
    driver: Loop::Adaptive,
    cell: OnceLock::new(),
};
static FOF: Kernel = Kernel {
    name: "forth-on-forthish",
    kernel_src: FOF_KERNEL_SRC,
    core_files: FOF_CORE_FILES,
    demos: FOF_DEMOS,
    driver: Loop::Adaptive,
    cell: OnceLock::new(),
};

const ALL: [&Kernel; 3] = [&FORTH_S, &FIF, &FOF];

/// Mirror of `ForthRepl::load_binary` / `Debugger::load_binary`: assemble,
/// load, queue the core files, run until the kernel idles with the queue
/// drained.
fn boot(k: &Kernel) -> Booted {
    let mut asm = Assembler::new();
    let result = asm.assemble(k.kernel_src);
    assert!(
        result.errors.is_empty(),
        "{}: assembly errors: {:?}",
        k.name,
        result.errors
    );
    let mut pump = Pump::new();
    pump.reset(&result.labels);
    assert!(
        !pump.poll_addrs.is_empty(),
        "{}: none of pump::POLL_LABELS resolve — the pump cannot detect idle",
        k.name
    );
    let mut emu = EmulatorCore::new();
    emu.hard_reset();
    emu.load_program(0, &result.bytes);
    emu.load_program_extent(result.bytes.len() as u32);
    emu.set_pc(0);
    pump.queue_core_files(k.core_files);
    emu.resume();
    let mut b = Booted { emu, pump };
    for _ in 0..100_000 {
        let out = tick(&mut b, k.driver);
        assert!(!out.halted, "{}: CPU halted during bootstrap", k.name);
        if b.pump.booted {
            b.emu.clear_uart_output();
            return b;
        }
    }
    panic!("{}: bootstrap did not finish", k.name);
}

fn booted(k: &Kernel) -> Booted {
    k.cell.get_or_init(|| boot(k)).clone()
}

fn tick(b: &mut Booted, driver: Loop) -> web_sw_cor24_forth::pump::TickOutcome {
    match driver {
        Loop::Flat => b.pump.tick_flat(&mut b.emu, BATCH_SIZE),
        Loop::Adaptive => b.pump.tick_adaptive(&mut b.emu),
    }
}

/// Mirror of `Msg::RunFile`: trimmed non-empty lines, each newline-terminated.
fn run_file(b: &mut Booted, src: &str) {
    for line in src.lines() {
        let t = line.trim_end_matches('\r').trim();
        if !t.is_empty() {
            b.pump.queue_line(t);
        }
    }
    b.pump.waiting_for_input = false;
}

fn demo<'a>(k: &'a Kernel, title: &str) -> &'a Demo {
    k.demos
        .iter()
        .find(|d| d.title == title)
        .unwrap_or_else(|| panic!("{}: no demo titled {title:?}", k.name))
}

/// The kernels report an unknown word by emitting a bare `?` token.
/// Match the token, not the character: `WORDS` legitimately prints `SW?`.
fn has_unknown_word_error(out: &str) -> bool {
    out.split_whitespace().any(|tok| tok == "?")
}

fn led_on(b: &Booted) -> bool {
    b.emu.is_led_on()
}

fn dsp(b: &Booted) -> u32 {
    b.emu.snapshot().regs[4]
}

/// Like [`run_until`] but reports a halt instead of panicking, so callers
/// can say which demo died.
fn try_run_until(
    b: &mut Booted,
    driver: Loop,
    max: usize,
    mut pred: impl FnMut(&Booted) -> bool,
) -> Result<Option<usize>, String> {
    for i in 0..max {
        let out = tick(b, driver);
        if out.halted {
            return Err(format!("CPU halted at tick {i}: {:?}", out.reason));
        }
        if pred(b) {
            return Ok(Some(i + 1));
        }
    }
    Ok(None)
}

/// Run ticks until `pred` holds or `max` ticks elapse; returns ticks used.
fn run_until(
    b: &mut Booted,
    driver: Loop,
    max: usize,
    mut pred: impl FnMut(&Booted) -> bool,
) -> Option<usize> {
    for i in 0..max {
        let out = tick(b, driver);
        assert!(!out.halted, "CPU halted at tick {i}: {:?}", out.reason);
        if pred(b) {
            return Some(i + 1);
        }
    }
    None
}

/// Run ticks until the kernel is idle with the queue drained.
fn settle(b: &mut Booted, driver: Loop, max: usize) {
    let done = run_until(b, driver, max, |b| {
        b.pump.waiting_for_input && b.pump.queue.is_empty()
    });
    assert!(
        done.is_some(),
        "kernel did not return to the prompt within {max} ticks"
    );
}

// ---------------------------------------------------------------------------

#[test]
fn every_kernel_boots_to_the_prompt() {
    for k in ALL {
        let b = booted(k);
        assert!(b.pump.booted && b.pump.waiting_for_input, "{}", k.name);
    }
}

/// `02-led.fth`: the `ON` / `OFF` words it defines reach the LED register.
/// Only the wiring is asserted, not the polarity: the register is
/// active-low (0 = lit) while the docs describe `LED!` as non-zero = on,
/// and that disagreement is upstream's to settle.
#[test]
fn led_control_demo_drives_the_led() {
    for k in ALL {
        let mut b = booted(k);
        run_file(&mut b, demo(k, "LED Control").source);
        settle(&mut b, k.driver, 2_000);
        let out = b.emu.get_uart_output().to_string();
        assert!(
            !has_unknown_word_error(&out),
            "{}: kernel reported an unknown word: {out:?}",
            k.name
        );
        b.pump.queue_line("ON");
        b.pump.waiting_for_input = false;
        settle(&mut b, k.driver, 2_000);
        let after_on = b.emu.get_led() & 1;
        b.pump.queue_line("OFF");
        b.pump.waiting_for_input = false;
        settle(&mut b, k.driver, 2_000);
        let after_off = b.emu.get_led() & 1;
        assert_ne!(
            after_on, after_off,
            "{}: `ON` then `OFF` left the LED register unchanged",
            k.name
        );
    }
}

/// `05-switch-led.fth`: press S2 first, then `SW? LED!` turns the LED on.
#[test]
fn switch_to_led_demo_copies_s2_into_d2() {
    for k in ALL {
        let src = demo(k, "Switch \u{2192} LED").source;

        let mut b = booted(k);
        b.emu.set_button_pressed(true);
        run_file(&mut b, src);
        settle(&mut b, k.driver, 2_000);
        assert!(led_on(&b), "{}: S2 pressed → LED should be on", k.name);

        let mut b = booted(k);
        b.emu.set_button_pressed(false);
        run_file(&mut b, src);
        settle(&mut b, k.driver, 2_000);
        assert!(!led_on(&b), "{}: S2 released → LED should be off", k.name);
    }
}

/// `13-loop-switch-led.fth`: while MIRROR runs, the LED tracks S2 and the
/// data stack does not leak.
#[test]
fn loop_switch_led_demo_tracks_s2_without_leaking() {
    for k in ALL {
        let mut b = booted(k);
        let idle_dsp = dsp(&b);
        let src = demo(k, "Loop Switch LED").source;
        run_file(&mut b, src);

        // Let the definitions compile and MIRROR start: the queue drains and
        // the CPU leaves the poll loop.
        let started = run_until(&mut b, k.driver, 2_000, |b| {
            b.pump.queue.is_empty() && !b.pump.waiting_for_input
        });
        assert!(started.is_some(), "{}: MIRROR never started", k.name);
        let out = b.emu.get_uart_output().to_string();
        assert!(
            !has_unknown_word_error(&out),
            "{}: kernel reported an unknown word: {out:?}",
            k.name
        );

        let mut min_dsp = u32::MAX;
        let mut check_stack = |b: &Booted| min_dsp = min_dsp.min(dsp(b));

        // Switch released → LED off.
        b.emu.set_button_pressed(false);
        let t = run_until(&mut b, k.driver, LED_FOLLOW_TICKS, |b| {
            check_stack(b);
            !led_on(b)
        });
        assert!(
            t.is_some(),
            "{}: LED did not go off after S2 released",
            k.name
        );

        // Press → LED on.
        b.emu.set_button_pressed(true);
        let t = run_until(&mut b, k.driver, LED_FOLLOW_TICKS, |b| {
            check_stack(b);
            led_on(b)
        });
        assert!(t.is_some(), "{}: LED did not follow S2 press", k.name);

        // Keep it running a while (hundreds of MIRROR iterations) so a
        // per-iteration stack leak shows up, then release → LED off.
        run_until(&mut b, k.driver, 300, |b| {
            check_stack(b);
            false
        });
        b.emu.set_button_pressed(false);
        let t = run_until(&mut b, k.driver, LED_FOLLOW_TICKS, |b| {
            check_stack(b);
            !led_on(b)
        });
        assert!(
            t.is_some(),
            "{}: LED did not follow S2 release after a long run",
            k.name
        );

        let dip_cells = idle_dsp.saturating_sub(min_dsp) / CELL;
        assert!(
            dip_cells <= MAX_STACK_DIP_CELLS,
            "{}: data stack dipped {dip_cells} cells below idle while MIRROR ran — the demo leaks the stack \
             (idle DSP {idle_dsp:#08X}, min {min_dsp:#08X})",
            k.name
        );
    }
}

/// (kernel, demo) pairs that are known to fail today, with why. The test
/// below requires each of these to *still* fail, so the list cannot go
/// stale: fix the kernel or the demo upstream, then delete the entry.
const KNOWN_BROKEN: &[(&str, &str, &str)] = &[
    // One root cause for all four: forth-on-forthish's Forth-level NUMBER
    // does not accept a leading '-' (`-1 .` prints `?`). forth.s and
    // forth-in-forth run every one of these demos clean.
    (
        "forth-on-forthish",
        "CONSTANT",
        "`-65280 CONSTANT UART-DATA` fails on the negative literal; later \
         `UART-DATA .` prints garbage and `UART-STAT .` halts the CPU",
    ),
    (
        "forth-on-forthish",
        "DO LOOP",
        "the `-1` sentinel line in FIND-5 prints `?`; the rest of the demo runs",
    ),
    (
        "forth-on-forthish",
        "IF ELSE",
        "`: DEMO -1 IF ...` prints `?` on the negative literal",
    ),
    (
        "forth-on-forthish",
        "IF THEN",
        "`: T -1 IF ...` prints `?` on the negative literal",
    ),
    (
        "forth-on-forthish",
        "Self Test",
        "the arithmetic line's negative literal prints `?` and leaves garbage; \
         the CPU halts before the end of the file",
    ),
];

/// Every demo in every tab's list must at least compile and run to the
/// prompt without a halt, and — except for Self Test, whose last section
/// deliberately feeds `FOO BAR` to exercise the unknown-word path — without
/// an unknown-word `?`. Demos that read the switch run with S2 released.
#[test]
fn every_shipped_demo_runs_clean() {
    for k in ALL {
        for d in k.demos {
            if d.title == "Loop Switch LED" {
                continue; // long-running; covered above
            }
            let known_broken = KNOWN_BROKEN
                .iter()
                .find(|(kn, dt, _)| *kn == k.name && *dt == d.title)
                .map(|(_, _, why)| *why);
            let expects_errors = d.title == "Self Test";
            let mut b = booted(k);
            run_file(&mut b, d.source);
            let max = 20_000;
            let done = try_run_until(&mut b, k.driver, max, |b| {
                b.pump.waiting_for_input && b.pump.queue.is_empty()
            });
            let out = b.emu.get_uart_output().to_string();
            if let Some(why) = known_broken {
                let still_broken = done.is_err() || has_unknown_word_error(&out);
                assert!(
                    still_broken,
                    "{}: demo {:?} now runs clean — remove it from KNOWN_BROKEN ({why})",
                    k.name, d.title
                );
                continue;
            }
            let done = match done {
                Ok(d) => d,
                Err(e) => panic!("{}: demo {:?}: {e}; output: {out:?}", k.name, d.title),
            };
            assert!(
                done.is_some(),
                "{}: demo {:?} did not finish within {max} ticks; output: {out:?}",
                k.name,
                d.title
            );
            assert!(
                expects_errors || !has_unknown_word_error(&out),
                "{}: demo {:?} hit an unknown word: {out:?}",
                k.name,
                d.title
            );
        }
    }
}
