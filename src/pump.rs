//! UART pump: hands queued input bytes to the emulated UART and runs the
//! CPU in batches sized to what the kernel is doing.
//!
//! This is the one piece of emulator-driving logic that both UI components
//! share — the REPL tabs (`src/repl.rs`) use the adaptive loop, the
//! debugger tab (`src/debugger.rs`) the flat one — and it is deliberately
//! free of Yew so the headless tests in `tests/` can drive the exact loop
//! the browser runs. Keep it that way: no `web_sys`, no timers, no DOM.

use cor24_emulator::{EmulatorCore, StopReason};
use std::collections::{HashMap, VecDeque};

/// UART status register: bit 0 = RX data ready (byte not yet consumed).
pub const UART_STATUS_ADDR: u32 = 0xFF0101;

/// Per-tick instruction budget once the REPL is interactive.
pub const BATCH_SIZE: u64 = 50_000;
/// Per-tick instruction budget while draining the core/*.fth queue. Tuned
/// so each tick completes in ~100–200 ms of UI-thread time so the page
/// stays responsive and the status bar keeps updating.
pub const BOOTSTRAP_BATCH: u64 = 600_000;
/// Adaptive pump-loop sub-batch sizes.
///   `PUMP_TINY` is used when the CPU is spinning in a UART-poll loop with
///   bytes still waiting in the RX queue — just enough instructions to let
///   the CPU consume the byte we just fed and either want another or start
///   real compile work. Anything larger is busy-waiting at ~19k instr/cheap
///   byte, which is the single biggest source of wasted time during boot.
///   `PUMP_BIG` is used when the CPU is doing real compile work (not at a
///   poll address) — let it make meaningful progress between poll checks.
pub const PUMP_TINY: u64 = 2_000;
pub const PUMP_BIG: u64 = 50_000;

/// Tick interval. Shorter during boot (less scheduler overhead per tick)
/// and restored to the interactive value once ready, so steady-state idle
/// doesn't burn CPU.
pub const TICK_MS_BOOT: u32 = 5;
pub const TICK_MS_INTERACTIVE: u32 = 25;

/// How many bytes past a poll label still count as "inside the poll loop".
pub const POLL_WINDOW: u32 = 16;

/// Labels of every UART busy-wait loop in the kernels we ship. Labels a
/// given kernel does not define are simply skipped.
///
/// A kernel that gains a new busy-wait loop must have its label added here:
/// otherwise the adaptive pump treats the wait as compute (never feeds the
/// next byte, so the REPL stalls) and the debugger reports "Running"
/// instead of "Ready". `tests/led_switch_demo.rs` boots every kernel
/// through this list, so a missing label fails CI rather than the demo.
pub const POLL_LABELS: &[&str] = &[
    "key_poll",
    "word_skip_rx",
    "word_skip_rx2",
    "word_read_rx",
    "word_read_rx2",
    "create_skip_rx",
    "create_skip_rx2",
    "create_read_rx",
    "create_read_rx2",
    "backslash_rx",
    "paren_rx",
];

/// What one tick did.
#[derive(Debug, Clone)]
pub struct TickOutcome {
    /// Instructions executed this tick.
    pub instructions: u64,
    /// Why the last batch stopped.
    pub reason: StopReason,
    /// CPU is halted (halt sentinel, invalid instruction, stack fault).
    pub halted: bool,
    /// This tick saw the kernel go idle with the queue drained for the
    /// first time: the bootstrap is complete.
    pub just_booted: bool,
}

/// Input queue + poll-loop knowledge + idle/booted state.
#[derive(Debug, Default, Clone)]
pub struct Pump {
    /// Bytes waiting to be handed to the UART, in order.
    pub queue: VecDeque<u8>,
    /// Addresses of the kernel's UART poll loops (resolved from
    /// [`POLL_LABELS`] for the loaded kernel).
    pub poll_addrs: Vec<u32>,
    /// Bootstrap finished: the kernel went idle with the queue drained.
    pub booted: bool,
    /// Last tick ended with the CPU parked in a poll loop (interactive
    /// "Ready" state).
    pub waiting_for_input: bool,
}

impl Pump {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget all state and resolve the poll addresses for a freshly
    /// assembled kernel.
    pub fn reset(&mut self, labels: &HashMap<String, u32>) {
        self.queue.clear();
        self.booted = false;
        self.waiting_for_input = false;
        self.poll_addrs = POLL_LABELS
            .iter()
            .filter_map(|name| labels.get(*name).copied())
            .collect();
    }

    /// Is `pc` inside one of the kernel's UART poll loops?
    pub fn is_idle(&self, pc: u32) -> bool {
        self.poll_addrs
            .iter()
            .any(|&addr| pc >= addr && pc < addr + POLL_WINDOW)
    }

    /// Queue one line of input followed by a newline.
    pub fn queue_line(&mut self, line: &str) {
        self.queue.extend(line.bytes());
        self.queue.push_back(b'\n');
    }

    /// Queue the core/*.fth tiers for a cold bootstrap: raw bytes,
    /// LF-normalized, no trim, one newline between tiers.
    pub fn queue_core_files(&mut self, core_files: &[(&str, &str)]) {
        for (i, (_name, contents)) in core_files.iter().enumerate() {
            if i > 0 && self.queue.back().is_none_or(|&b| b != b'\n') {
                self.queue.push_back(b'\n');
            }
            for b in contents.bytes() {
                if b == b'\r' {
                    continue; // strip CR from CRLF
                }
                self.queue.push_back(b);
            }
            if self.queue.back().is_none_or(|&b| b != b'\n') {
                self.queue.push_back(b'\n');
            }
        }
    }

    /// Hand the next queued byte to the UART if the previous one has been
    /// consumed.
    pub fn feed_uart_byte(&mut self, emu: &mut EmulatorCore) {
        if self.queue.is_empty() {
            return;
        }
        let status = emu.read_byte(UART_STATUS_ADDR);
        if status & 0x01 == 0
            && let Some(byte) = self.queue.pop_front()
        {
            emu.send_uart_byte(byte);
        }
    }

    /// Debugger-style tick: feed at most one byte, then run a flat batch.
    pub fn tick_flat(&mut self, emu: &mut EmulatorCore, budget: u64) -> TickOutcome {
        self.feed_uart_byte(emu);
        let result = emu.run_batch(budget);
        self.finish_tick(emu, result.instructions_run, result.reason)
    }

    /// REPL-style adaptive tick. Each iteration:
    ///   1. Check if the CPU is busy-waiting at a UART poll.
    ///   2. If polling with empty queue → truly done, break.
    ///   3. If polling with bytes to feed → push one byte, run a TINY
    ///      sub-batch (~2k instr, just enough for the CPU to consume the
    ///      byte and either want another or start real compile work).
    ///      Previously we ran ~20k between feeds, ~19.5k of which the CPU
    ///      burned spinning in key_poll waiting for the next byte.
    ///   4. If not polling → CPU is doing real compile work, run a BIG
    ///      sub-batch to make progress.
    ///
    /// The per-tick budget is [`BOOTSTRAP_BATCH`] until booted, then
    /// [`BATCH_SIZE`].
    pub fn tick_adaptive(&mut self, emu: &mut EmulatorCore) -> TickOutcome {
        let budget = if self.booted {
            BATCH_SIZE
        } else {
            BOOTSTRAP_BATCH
        };
        let mut used: u64 = 0;
        let mut last_reason = StopReason::CycleLimit;
        loop {
            let pc = emu.snapshot().pc;
            let idle_polling = self.is_idle(pc);
            if idle_polling && self.queue.is_empty() {
                break;
            }
            let chunk_size = if idle_polling {
                self.feed_uart_byte(emu);
                PUMP_TINY
            } else {
                PUMP_BIG
            };
            let remaining = budget.saturating_sub(used);
            if remaining == 0 {
                break;
            }
            let result = emu.run_batch(remaining.min(chunk_size));
            last_reason = result.reason;
            // Safety: if the emulator isn't advancing (paused, halted),
            // bail out rather than spin.
            if result.instructions_run == 0 {
                break;
            }
            used += result.instructions_run;
            if matches!(last_reason, StopReason::Halted) {
                break;
            }
        }
        self.finish_tick(emu, used, last_reason)
    }

    fn finish_tick(
        &mut self,
        emu: &EmulatorCore,
        instructions: u64,
        reason: StopReason,
    ) -> TickOutcome {
        let pc = emu.snapshot().pc;
        self.waiting_for_input = matches!(reason, StopReason::CycleLimit) && self.is_idle(pc);
        // First idle with empty queue = bootstrap complete.
        let just_booted = !self.booted && self.waiting_for_input && self.queue.is_empty();
        if just_booted {
            self.booted = true;
        }
        TickOutcome {
            instructions,
            reason,
            halted: emu.is_halted(),
            just_booted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_window_is_half_open() {
        let mut p = Pump::new();
        p.poll_addrs = vec![0x100];
        assert!(p.is_idle(0x100));
        assert!(p.is_idle(0x100 + POLL_WINDOW - 1));
        assert!(!p.is_idle(0x100 + POLL_WINDOW));
        assert!(!p.is_idle(0xFF));
    }

    #[test]
    fn core_files_are_lf_separated_and_cr_stripped() {
        let mut p = Pump::new();
        p.queue_core_files(&[("a", "1\r\n2"), ("b", "3\n")]);
        assert_eq!(p.queue.iter().copied().collect::<Vec<u8>>(), b"1\n2\n3\n");
    }

    #[test]
    fn reset_resolves_only_known_labels() {
        let mut p = Pump::new();
        let labels: HashMap<String, u32> =
            [("key_poll".to_string(), 0x40), ("do_lit".to_string(), 0x80)]
                .into_iter()
                .collect();
        p.queue_line("x");
        p.booted = true;
        p.reset(&labels);
        assert_eq!(p.poll_addrs, vec![0x40]);
        assert!(p.queue.is_empty());
        assert!(!p.booted);
    }
}
