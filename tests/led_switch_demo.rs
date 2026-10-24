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
use std::time::{Duration, Instant};
use web_sw_cor24_forth::config::ForthTier;
use web_sw_cor24_forth::demos::{
    Demo, FFF_CORE_FILES, FFF_DEMOS, FFF_KERNEL_SRC, FIF_CORE_FILES, FIF_DEMOS, FIF_KERNEL_SRC,
    FOF_CORE_FILES, FOF_DEMOS, FOF_KERNEL_SRC, FORTH_S_DEMOS,
};
use web_sw_cor24_forth::pump::Pump;

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
/// A demo must be fully typed in and running within this many ticks of
/// being loaded (25 ms each in the browser, so 5 s). Tab 1 used to need
/// 511 ticks for this demo — one byte per tick — which read as "the
/// switch does nothing" because nobody waits 13 s before clicking.
const MAX_START_TICKS: usize = 200;

/// A booted kernel: emulator + pump, ready for interactive input.
#[derive(Clone)]
struct Booted {
    emu: EmulatorCore,
    pump: Pump,
    /// Ticks the bootstrap took (the browser runs one tick per timer).
    boot_ticks: usize,
}

struct Kernel {
    name: &'static str,
    kernel_src: &'static str,
    core_files: &'static [(&'static str, &'static str)],
    demos: &'static [Demo],
    cell: OnceLock<Booted>,
}

static FORTH_S: Kernel = Kernel {
    name: "forth.s",
    kernel_src: ForthTier::Interpreter.assembly(),
    core_files: &[],
    demos: FORTH_S_DEMOS,
    cell: OnceLock::new(),
};
static FIF: Kernel = Kernel {
    name: "forth-in-forth",
    kernel_src: FIF_KERNEL_SRC,
    core_files: FIF_CORE_FILES,
    demos: FIF_DEMOS,
    cell: OnceLock::new(),
};
static FOF: Kernel = Kernel {
    name: "forth-on-forthish",
    kernel_src: FOF_KERNEL_SRC,
    core_files: FOF_CORE_FILES,
    demos: FOF_DEMOS,
    cell: OnceLock::new(),
};

static FFF: Kernel = Kernel {
    name: "forth-from-forth",
    kernel_src: FFF_KERNEL_SRC,
    core_files: FFF_CORE_FILES,
    demos: FFF_DEMOS,
    cell: OnceLock::new(),
};

const ALL: [&Kernel; 4] = [&FORTH_S, &FIF, &FOF, &FFF];

/// Wall-clock budget for any one emulator loop (a boot, or one demo run).
/// Tick limits alone are not a bound: a tick may run 600k instructions, so a
/// kernel that never goes idle could spin for many minutes. Override with
/// LED_SWITCH_DEMO_BUDGET_SECS.
fn loop_budget() -> Duration {
    let secs = std::env::var("LED_SWITCH_DEMO_BUDGET_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    Duration::from_secs(secs)
}

/// Progress and deadline for one emulator loop. Prints a line every 5 s
/// (visible with --nocapture, and in the failure output) and panics with
/// the kernel state once the budget is spent.
struct Watchdog {
    what: String,
    start: Instant,
    last: Instant,
    budget: Duration,
}

impl Watchdog {
    fn new(what: impl Into<String>) -> Self {
        Self::with_budget(what, loop_budget())
    }

    fn with_budget(what: impl Into<String>, budget: Duration) -> Self {
        let now = Instant::now();
        Watchdog {
            what: what.into(),
            start: now,
            last: now,
            budget,
        }
    }

    fn check(&mut self, b: &Booted, ticks: usize) {
        let now = Instant::now();
        let snap = b.emu.snapshot();
        if now.duration_since(self.last) >= Duration::from_secs(5) {
            self.last = now;
            eprintln!(
                "[{}] {:.0}s: {ticks} ticks, {} instructions, pc={:#08x}, queue={}, idle={}",
                self.what,
                now.duration_since(self.start).as_secs_f64(),
                b.emu.instructions_count(),
                snap.pc,
                b.pump.queue.len(),
                b.pump.is_idle(snap.pc)
            );
        }
        if now.duration_since(self.start) > self.budget {
            panic!(
                "[{}] gave up after {:?}: {ticks} ticks, {} instructions, pc={:#08x}, \
                 queue={}, idle={} -- a kernel that never reaches a POLL_LABELS loop \
                 spins here",
                self.what,
                self.budget,
                b.emu.instructions_count(),
                snap.pc,
                b.pump.queue.len(),
                b.pump.is_idle(snap.pc)
            );
        }
    }
}

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
    let mut b = Booted {
        emu,
        pump,
        boot_ticks: 0,
    };
    let mut dog = Watchdog::new(format!("{} boot", k.name));
    for n in 1..=100_000 {
        dog.check(&b, n);
        let out = tick(&mut b);
        assert!(!out.halted, "{}: CPU halted during bootstrap", k.name);
        if b.pump.booted {
            b.emu.clear_uart_output();
            b.boot_ticks = n;
            return b;
        }
    }
    panic!("{}: bootstrap did not finish", k.name);
}

fn booted(k: &Kernel) -> Booted {
    k.cell.get_or_init(|| boot(k)).clone()
}

fn tick(b: &mut Booted) -> web_sw_cor24_forth::pump::TickOutcome {
    b.pump.tick_adaptive(&mut b.emu)
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
    max: usize,
    mut pred: impl FnMut(&Booted) -> bool,
) -> Result<Option<usize>, String> {
    let mut dog = Watchdog::new("run");
    for i in 0..max {
        dog.check(b, i);
        let out = tick(b);
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
fn run_until(b: &mut Booted, max: usize, mut pred: impl FnMut(&Booted) -> bool) -> Option<usize> {
    for i in 0..max {
        let out = tick(b);
        assert!(!out.halted, "CPU halted at tick {i}: {:?}", out.reason);
        if pred(b) {
            return Some(i + 1);
        }
    }
    None
}

/// Run ticks until the kernel is idle with the queue drained.
fn settle(b: &mut Booted, max: usize) {
    let done = run_until(b, max, |b| {
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

/// forth-from-forth's point: the core is in the image, so it must be ready
/// almost at once, where forth-on-forthish compiles five tiers first.
#[test]
fn image_kernel_boots_far_faster_than_streaming_the_core() {
    let fff = booted(&FFF).boot_ticks;
    let fof = booted(&FOF).boot_ticks;
    eprintln!("boot ticks: forth-from-forth {fff}, forth-on-forthish {fof}");
    assert!(fff <= 3, "forth-from-forth took {fff} ticks to boot");
    assert!(
        fff * 20 <= fof,
        "forth-from-forth {fff} ticks vs forth-on-forthish {fof}"
    );
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
        settle(&mut b, 2_000);
        let out = b.emu.get_uart_output().to_string();
        assert!(
            !has_unknown_word_error(&out),
            "{}: kernel reported an unknown word: {out:?}",
            k.name
        );
        b.pump.queue_line("ON");
        b.pump.waiting_for_input = false;
        settle(&mut b, 2_000);
        let after_on = b.emu.get_led() & 1;
        b.pump.queue_line("OFF");
        b.pump.waiting_for_input = false;
        settle(&mut b, 2_000);
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
        settle(&mut b, 2_000);
        assert!(led_on(&b), "{}: S2 pressed → LED should be on", k.name);

        let mut b = booted(k);
        b.emu.set_button_pressed(false);
        run_file(&mut b, src);
        settle(&mut b, 2_000);
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
        let started = run_until(&mut b, 2_000, |b| {
            b.pump.queue.is_empty() && !b.pump.waiting_for_input
        });
        let started = started.unwrap_or_else(|| panic!("{}: MIRROR never started", k.name));
        assert!(
            started <= MAX_START_TICKS,
            "{}: MIRROR took {started} ticks to start (limit {MAX_START_TICKS}) — the pump is \
             starving the kernel of input",
            k.name
        );
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
        let t = run_until(&mut b, LED_FOLLOW_TICKS, |b| {
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
        let t = run_until(&mut b, LED_FOLLOW_TICKS, |b| {
            check_stack(b);
            led_on(b)
        });
        assert!(t.is_some(), "{}: LED did not follow S2 press", k.name);

        // Keep it running a while (hundreds of MIRROR iterations) so a
        // per-iteration stack leak shows up, then release → LED off.
        run_until(&mut b, 300, |b| {
            check_stack(b);
            false
        });
        b.emu.set_button_pressed(false);
        let t = run_until(&mut b, LED_FOLLOW_TICKS, |b| {
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
    // Empty since sw-cor24-forth fixed forth-on-forthish's Forth NUMBER
    // (negative literals) on 2026-10-23; that one bug broke CONSTANT,
    // DO LOOP, IF ELSE, IF THEN and Self Test on tab 3.
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
            let done = try_run_until(&mut b, max, |b| {
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

/// Tab 4's "Rebuild kernel": the running forth-from-forth kernel compiles
/// its own kernel.s from its Forth sources through the same pump (in turbo
/// mode) and the same `rebuild` module the browser uses, and the result
/// must be the kernel it is running, text and bytes. This is the
/// self-hosting fixed point, checked in the tab's own code path.
#[test]
fn tab4_rebuilds_its_own_kernel() {
    let mut b = booted(&FFF);
    let input = web_sw_cor24_forth::rebuild::input();
    b.pump.queue.extend(input.bytes());
    b.pump.turbo = true;
    b.pump.waiting_for_input = false;
    let mut captured = String::new();
    let mut dog = Watchdog::with_budget("tab 4 rebuild", loop_budget() * 3);
    let mut ticks = 0;
    let text = loop {
        dog.check(&b, ticks);
        let out = tick(&mut b);
        ticks += 1;
        assert!(
            !out.halted,
            "CPU halted during the rebuild: {:?}",
            out.reason
        );
        captured.push_str(b.emu.get_uart_output());
        b.emu.clear_uart_output();
        if let Some(t) = web_sw_cor24_forth::rebuild::extract(&captured) {
            break t;
        }
    };
    let v = web_sw_cor24_forth::rebuild::verdict(&text, FFF_KERNEL_SRC);
    eprintln!(
        "tab 4 rebuild: {ticks} ticks, {} lines, {:?} bytes",
        v.lines, v.bytes
    );
    assert!(
        v.same_text && v.same_bytes,
        "rebuilt kernel differs from the running one: {:?}",
        v.first_difference
    );
}
