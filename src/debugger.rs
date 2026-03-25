//! Forth debugger component — COR24 emulator with Forth-aware inspection.

use crate::config::{ForthTier, StackSize};
use cor24_emulator::{Assembler, EmulatorCore};
use gloo::timers::callback::Timeout;
use std::collections::VecDeque;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;

/// Execution batch size per tick (instructions).
const BATCH_SIZE: u64 = 50_000;

/// Tick interval in milliseconds.
const TICK_MS: u32 = 25;

pub enum Msg {
    /// Load and start the emulator.
    Init,
    /// Run a batch of instructions.
    Tick,
    /// User typed in the input bar.
    InputChanged(String),
    /// User pressed Enter to send input.
    SendInput,
    /// Switch Forth tier.
    SetTier(ForthTier),
    /// Switch stack size.
    SetStack(StackSize),
    /// Reset emulator.
    Reset,
    /// Step one instruction.
    Step,
    /// Step over (run until return from call).
    StepOver,
    /// Toggle run/pause.
    PauseResume,
    /// Handle keydown in input field.
    InputKeyDown(KeyboardEvent),
}

pub struct Debugger {
    emulator: EmulatorCore,
    tier: ForthTier,
    stack_size: StackSize,
    output: String,
    input: String,
    running: bool,
    halted: bool,
    /// Pending timeout handle (kept alive to prevent cancel).
    _tick_handle: Option<Timeout>,
    /// Previous register values for change highlighting.
    prev_regs: [u32; 8],
    prev_pc: u32,
    /// Pending UART RX bytes to feed one-at-a-time.
    uart_rx_queue: VecDeque<u8>,
    /// Ref to output div for auto-scroll.
    output_ref: NodeRef,
}

impl Debugger {
    fn load_binary(&mut self, _ctx: &Context<Self>) {
        let mut asm = Assembler::new();
        let result = asm.assemble(self.tier.assembly());

        if !result.errors.is_empty() {
            self.output = "Assembly errors:\n".to_string();
            for e in &result.errors {
                self.output.push_str(e);
                self.output.push('\n');
            }
            return;
        }

        self.emulator.hard_reset();
        self.emulator.load_program(0, &result.bytes);
        self.emulator.load_program_extent(result.bytes.len() as u32);
        self.emulator.set_pc(0);
        self.output.clear();
        self.halted = false;
        self.prev_regs = [0; 8];
        self.prev_pc = 0;
        self.uart_rx_queue.clear();

        // Start paused — debugger mode.
        self.running = false;
        self.emulator.pause();
    }

    fn schedule_tick(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        self._tick_handle = Some(Timeout::new(TICK_MS, move || {
            link.send_message(Msg::Tick);
        }));
    }

    fn snapshot_regs(&self) -> [u32; 8] {
        let snap = self.emulator.snapshot();
        snap.regs
    }

    fn read_data_stack(&self) -> Vec<u32> {
        let sp = self.emulator.snapshot().regs[4]; // sp = r4
        let stack_top = self.stack_size.initial_sp();
        let mut cells = Vec::new();
        let mut addr = stack_top;
        // Stack grows downward; entries are from stack_top-3 down to sp.
        while addr > sp && cells.len() < 64 {
            addr -= 3; // cell = 3 bytes
            let val = self.emulator.read_word(addr);
            cells.push(val & 0xFFFFFF);
        }
        cells.reverse(); // bottom of stack first
        cells
    }

    fn read_return_stack(&self) -> Vec<u32> {
        let rsp = self.emulator.snapshot().regs[1]; // r1 = RSP
        let rstack_base: u32 = 0x0F0000;
        let mut cells = Vec::new();
        let mut addr = rstack_base;
        while addr > rsp && cells.len() < 64 {
            addr -= 3;
            let val = self.emulator.read_word(addr);
            cells.push(val & 0xFFFFFF);
        }
        cells.reverse();
        cells
    }

    /// Disassemble instructions around the current PC.
    fn disassemble_around_pc(
        &self,
        count_before: usize,
        count_after: usize,
    ) -> Vec<(u32, String, bool)> {
        let pc = self.emulator.snapshot().pc;
        // Disassemble forward from PC to get the current + after
        let forward = self.emulator.disassemble(pc, count_after + 1);

        // For instructions before PC, scan backwards heuristically.
        // COR24 has variable-length instructions (1, 2, or 4 bytes).
        // Walk backwards by trying offsets.
        let mut before = Vec::new();
        if count_before > 0 && pc > 0 {
            // Collect candidate addresses by scanning backwards
            let scan_start = pc.saturating_sub((count_before as u32) * 4 + 8);
            let all = self.emulator.disassemble(scan_start, 128);
            // Find instructions that end at or before PC
            for &(addr, ref mnemonic, size) in &all {
                if addr < pc {
                    before.push((addr, mnemonic.clone(), size));
                } else {
                    break;
                }
            }
            // Take only the last count_before
            let skip = before.len().saturating_sub(count_before);
            before = before.into_iter().skip(skip).collect();
        }

        let mut result: Vec<(u32, String, bool)> = Vec::new();
        for (addr, mnemonic, _size) in before {
            result.push((addr, mnemonic, false));
        }
        for (addr, mnemonic, _size) in forward {
            result.push((addr, mnemonic, addr == pc));
        }
        result
    }

    /// Feed one byte from the UART RX queue if the UART is ready.
    fn feed_uart_byte(&mut self) {
        if self.uart_rx_queue.is_empty() {
            return;
        }
        // Check if UART RX buffer can accept (status register bit 0)
        let status = self.emulator.read_byte(0xFF0101);
        if status & 0x01 == 0 {
            // UART ready to receive — feed one byte
            if let Some(byte) = self.uart_rx_queue.pop_front() {
                self.emulator.send_uart_byte(byte);
            }
        }
    }

    /// Collect UART output and auto-scroll.
    fn collect_uart_output(&mut self) {
        let uart = self.emulator.get_uart_output();
        if !uart.is_empty() {
            self.output.push_str(uart);
            self.emulator.clear_uart_output();
        }
    }

    fn auto_scroll(&self) {
        if let Some(el) = self.output_ref.cast::<HtmlElement>() {
            el.set_scroll_top(el.scroll_height());
        }
    }
}

impl Component for Debugger {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Init);
        Self {
            emulator: EmulatorCore::new(),
            tier: ForthTier::Bootstrap,
            stack_size: StackSize::ThreeKb,
            output: String::new(),
            input: String::new(),
            running: false,
            halted: false,
            _tick_handle: None,
            prev_regs: [0; 8],
            prev_pc: 0,
            uart_rx_queue: VecDeque::new(),
            output_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Init => {
                self.load_binary(ctx);
                true
            }

            Msg::Tick => {
                if !self.running {
                    return false;
                }

                // Save previous state for change highlighting.
                self.prev_regs = self.snapshot_regs();
                self.prev_pc = self.emulator.snapshot().pc;

                // Feed pending UART input bytes (poll-before-feed).
                self.feed_uart_byte();

                let _result = self.emulator.run_batch(BATCH_SIZE);

                // Collect UART output.
                self.collect_uart_output();

                if self.emulator.is_halted() {
                    self.running = false;
                    self.halted = true;
                } else if self.running {
                    self.schedule_tick(ctx);
                }

                true
            }

            Msg::InputChanged(val) => {
                self.input = val;
                false
            }

            Msg::SendInput => {
                if self.input.is_empty() {
                    return false;
                }
                // Queue bytes for poll-before-feed delivery.
                for b in self.input.bytes() {
                    self.uart_rx_queue.push_back(b);
                }
                self.uart_rx_queue.push_back(b'\n');
                self.input.clear();

                if !self.running && !self.halted {
                    self.running = true;
                    self.emulator.resume();
                    self.schedule_tick(ctx);
                }
                true
            }

            Msg::SetTier(tier) => {
                self.tier = tier;
                self.load_binary(ctx);
                true
            }

            Msg::SetStack(size) => {
                self.stack_size = size;
                self.load_binary(ctx);
                true
            }

            Msg::Reset => {
                self.load_binary(ctx);
                true
            }

            Msg::Step => {
                if self.halted {
                    return false;
                }
                self.running = false;
                self.emulator.resume();

                self.prev_regs = self.snapshot_regs();
                self.prev_pc = self.emulator.snapshot().pc;

                self.feed_uart_byte();
                let _result = self.emulator.step();
                self.collect_uart_output();

                self.emulator.pause();
                if self.emulator.is_halted() {
                    self.halted = true;
                }
                true
            }

            Msg::StepOver => {
                if self.halted {
                    return false;
                }
                self.running = false;
                self.emulator.resume();

                self.prev_regs = self.snapshot_regs();
                self.prev_pc = self.emulator.snapshot().pc;

                self.feed_uart_byte();
                let _result = self.emulator.step_over();
                self.collect_uart_output();

                self.emulator.pause();
                if self.emulator.is_halted() {
                    self.halted = true;
                }
                true
            }

            Msg::PauseResume => {
                if self.halted {
                    return false;
                }
                if self.running {
                    self.running = false;
                    self.emulator.pause();
                    self._tick_handle = None;
                } else {
                    self.running = true;
                    self.emulator.resume();
                    self.schedule_tick(ctx);
                }
                true
            }

            Msg::InputKeyDown(e) => {
                if e.key() == "Enter" {
                    ctx.link().send_message(Msg::SendInput);
                }
                false
            }
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        self.auto_scroll();
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let snap = self.emulator.snapshot();
        let data_stack = self.read_data_stack();
        let return_stack = self.read_return_stack();
        let disasm = self.disassemble_around_pc(4, 8);

        let reg_names = [
            "r0/W", "r1/RSP", "r2/IP", "r3/fp", "sp/DSP", "r5/zc", "r6/iv", "r7/ir",
        ];

        html! {
            <div class="debugger">
                // Toolbar
                <div class="toolbar">
                    <button onclick={ctx.link().callback(|_| Msg::PauseResume)}
                            class={if self.running { "active" } else { "" }}>
                        { if self.running { "Pause" } else { "Run" } }
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::Step)}
                            disabled={self.running || self.halted}>
                        {"Step"}
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::StepOver)}
                            disabled={self.running || self.halted}>
                        {"Step Over"}
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::Reset)}>
                        {"Reset"}
                    </button>

                    <select onchange={ctx.link().callback(|e: Event| {
                        let select: HtmlInputElement = e.target_unchecked_into();
                        let tier = match select.value().as_str() {
                            "Bootstrap" => ForthTier::Bootstrap,
                            _ => ForthTier::Bootstrap,
                        };
                        Msg::SetTier(tier)
                    })}>
                        { for ForthTier::ALL.iter().map(|t| {
                            html! {
                                <option value={t.label()} selected={*t == self.tier}>
                                    { t.label() }
                                </option>
                            }
                        })}
                    </select>

                    <select onchange={ctx.link().callback(|e: Event| {
                        let select: HtmlInputElement = e.target_unchecked_into();
                        let size = match select.value().as_str() {
                            "3 KB" => StackSize::ThreeKb,
                            _ => StackSize::EightKb,
                        };
                        Msg::SetStack(size)
                    })}>
                        { for StackSize::ALL.iter().map(|s| {
                            html! {
                                <option value={s.label()} selected={*s == self.stack_size}>
                                    { s.label() }
                                </option>
                            }
                        })}
                    </select>

                    <span class="tier-desc">{ self.tier.description() }</span>
                </div>

                // Main panels
                <div class="panels">
                    // Output / terminal
                    <div class="output-panel">
                        <div class="output" ref={self.output_ref.clone()}>{ &self.output }</div>
                        <div class="input-bar">
                            <span class="prompt">{"> "}</span>
                            <input
                                type="text"
                                value={self.input.clone()}
                                oninput={ctx.link().callback(|e: InputEvent| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    Msg::InputChanged(input.value())
                                })}
                                onkeydown={ctx.link().callback(Msg::InputKeyDown)}
                                placeholder="Type Forth input..."
                            />
                        </div>
                    </div>

                    // Side panel: registers + stacks
                    <div class="side-panel">
                        // CPU Registers
                        <div class="panel-section">
                            <h3>{"Registers"}</h3>
                            <div class="registers">
                                <span class="reg-name">{"PC"}</span>
                                <span class={classes!(
                                    "reg-value",
                                    (snap.pc != self.prev_pc).then_some("changed")
                                )}>
                                    { format!("{:06X}", snap.pc) }
                                </span>
                                { for (0..8).map(|i| {
                                    let changed = snap.regs[i] != self.prev_regs[i];
                                    html! {
                                        <>
                                            <span class="reg-name">{ reg_names[i] }</span>
                                            <span class={classes!(
                                                "reg-value",
                                                changed.then_some("changed")
                                            )}>
                                                { format!("{:06X}", snap.regs[i] & 0xFFFFFF) }
                                            </span>
                                        </>
                                    }
                                })}
                                <span class="reg-name">{"C"}</span>
                                <span class="reg-value">
                                    { if snap.c { "1" } else { "0" } }
                                </span>
                            </div>
                        </div>

                        // Data Stack
                        <div class="panel-section">
                            <h3>{ format!("Data Stack ({})", data_stack.len()) }</h3>
                            <div class="stack-display">
                                { if data_stack.is_empty() {
                                    html! { <span class="stack-empty">{"(empty)"}</span> }
                                } else {
                                    html! {
                                        { for data_stack.iter().enumerate().map(|(i, val)| {
                                            let is_tos = i == data_stack.len() - 1;
                                            html! {
                                                <div class="stack-entry">
                                                    <span class="stack-index">
                                                        { format!("[{}]", i) }
                                                    </span>
                                                    <span class="stack-value">
                                                        { format!("{:06X}  {}", val, val) }
                                                    </span>
                                                    { if is_tos {
                                                        html! { <span class="stack-tos">{"<- TOS"}</span> }
                                                    } else {
                                                        html! {}
                                                    }}
                                                </div>
                                            }
                                        })}
                                    }
                                }}
                            </div>
                        </div>

                        // Return Stack
                        <div class="panel-section">
                            <h3>{ format!("Return Stack ({})", return_stack.len()) }</h3>
                            <div class="stack-display">
                                { if return_stack.is_empty() {
                                    html! { <span class="stack-empty">{"(empty)"}</span> }
                                } else {
                                    html! {
                                        { for return_stack.iter().enumerate().map(|(i, val)| {
                                            let is_top = i == return_stack.len() - 1;
                                            html! {
                                                <div class="stack-entry">
                                                    <span class="stack-index">
                                                        { format!("[{}]", i) }
                                                    </span>
                                                    <span class="stack-value">
                                                        { format!("{:06X}  {}", val, val) }
                                                    </span>
                                                    { if is_top {
                                                        html! { <span class="stack-tos">{"<- TOP"}</span> }
                                                    } else {
                                                        html! {}
                                                    }}
                                                </div>
                                            }
                                        })}
                                    }
                                }}
                            </div>
                        </div>

                        // LED + status
                        <div class="panel-section">
                            <h3>{"Hardware"}</h3>
                            <div class="registers">
                                <span class="reg-name">{"LED"}</span>
                                <span class="reg-value">
                                    <span class={classes!(
                                        "led",
                                        (snap.led != 0).then_some("on")
                                    )}></span>
                                </span>
                                <span class="reg-name">{"Cycles"}</span>
                                <span class="reg-value">{ format!("{}", snap.cycles) }</span>
                                <span class="reg-name">{"Instrs"}</span>
                                <span class="reg-value">{ format!("{}", snap.instructions) }</span>
                            </div>
                        </div>

                        // Disassembly
                        <div class="panel-section">
                            <h3>{"Disassembly"}</h3>
                            <div class="disasm-view">
                                { for disasm.iter().map(|(addr, mnemonic, is_current)| {
                                    html! {
                                        <div class={classes!(
                                            "disasm-line",
                                            is_current.then_some("disasm-current")
                                        )}>
                                            <span class="disasm-addr">
                                                { format!("{:06X}", addr) }
                                            </span>
                                            <span class="disasm-instr">
                                                { mnemonic }
                                            </span>
                                        </div>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                </div>

                // Status bar
                <div class="status-bar">
                    <div class="status-item">
                        <span class="status-label">{"Status:"}</span>
                        <span class="status-value">
                            { if self.halted { "Halted" }
                              else if self.running { "Running" }
                              else { "Paused" }
                            }
                        </span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"PC:"}</span>
                        <span class="status-value">{ format!("0x{:06X}", snap.pc) }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Tier:"}</span>
                        <span class="status-value">{ self.tier.label() }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Stack:"}</span>
                        <span class="status-value">{ self.stack_size.label() }</span>
                    </div>
                </div>
            </div>
        }
    }
}
