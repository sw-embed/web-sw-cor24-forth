//! Forth debugger component — COR24 emulator with Forth-aware inspection.

use crate::config::{ForthTier, StackSize};
use crate::demos::DEMOS;
use cor24_emulator::{AssembledLine, Assembler, EmulatorCore};
use gloo::timers::callback::Timeout;
use std::collections::{HashMap, VecDeque};
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;

/// Execution batch size per tick (instructions).
const BATCH_SIZE: u64 = 50_000;

/// Tick interval in milliseconds.
const TICK_MS: u32 = 25;

/// Cell size in bytes (24-bit words).
const CELL: u32 = 3;

/// Bottom panel tab selection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Dictionary,
    CompileLog,
}

/// Categorized Forth word entry from assembler labels.
#[derive(Clone)]
struct DictEntry {
    name: String,
    addr: u32,
    kind: WordKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordKind {
    Primitive,
    ColonDef,
    Thread,
}

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
    /// Toggle breakpoint at address.
    ToggleBreakpoint(u32),
    /// Select a dictionary word for inspection.
    SelectWord(String),
    /// Load and run a demo by index.
    LoadDemo(usize),
    /// Toggle hardware switch S2.
    ToggleSwitch,
    /// Switch bottom panel tab.
    SetBottomTab(BottomTab),
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
    /// Assembler labels: label name -> address.
    labels: HashMap<String, u32>,
    /// Reverse lookup: address -> label name.
    reverse_labels: HashMap<u32, String>,
    /// Assembled lines for compile log.
    assembled_lines: Vec<AssembledLine>,
    /// Categorized dictionary entries.
    dict_entries: Vec<DictEntry>,
    /// Currently selected word for inspection.
    selected_word: Option<String>,
    /// Bottom panel tab.
    bottom_tab: BottomTab,
    /// Program extent (end of assembled code).
    program_end: u32,
    /// Hardware switch S2 state.
    switch_pressed: bool,
    /// Currently selected demo index.
    selected_demo: Option<usize>,
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

        // Store assembly metadata for panels.
        self.labels = result.labels.clone();
        self.reverse_labels = result
            .labels
            .iter()
            .map(|(name, &addr)| (addr, name.clone()))
            .collect();
        self.assembled_lines = result.lines.clone();
        self.program_end = result.bytes.len() as u32;
        self.dict_entries = self.build_dict_entries();

        self.emulator.hard_reset();
        self.emulator.load_program(0, &result.bytes);
        self.emulator.load_program_extent(result.bytes.len() as u32);
        self.emulator.set_pc(0);
        self.output.clear();
        self.halted = false;
        self.prev_regs = [0; 8];
        self.prev_pc = 0;
        self.uart_rx_queue.clear();
        self.selected_word = None;
        self.switch_pressed = false;
        self.selected_demo = None;

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
            addr -= CELL;
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
        // Only walk when RSP has been initialized to the return stack region
        if rsp == 0 || rsp >= rstack_base {
            return cells;
        }
        let mut addr = rstack_base;
        while addr > rsp && cells.len() < 64 {
            addr -= CELL;
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
        let forward = self.emulator.disassemble(pc, count_after + 1);

        let mut before = Vec::new();
        if count_before > 0 && pc > 0 {
            let scan_start = pc.saturating_sub((count_before as u32) * 4 + 8);
            let all = self.emulator.disassemble(scan_start, 128);
            for &(addr, ref mnemonic, size) in &all {
                if addr < pc {
                    before.push((addr, mnemonic.clone(), size));
                } else {
                    break;
                }
            }
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
        let status = self.emulator.read_byte(0xFF0101);
        if status & 0x01 == 0
            && let Some(byte) = self.uart_rx_queue.pop_front()
        {
            self.emulator.send_uart_byte(byte);
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

    /// Resolve an address to the nearest label at or before it.
    fn resolve_addr(&self, addr: u32) -> String {
        if let Some(name) = self.reverse_labels.get(&addr) {
            return name.clone();
        }
        // Find the closest label before this address.
        let mut best: Option<(&String, u32)> = None;
        for (name, &label_addr) in &self.labels {
            if label_addr <= addr {
                let dist = addr - label_addr;
                if dist < 32 {
                    match best {
                        None => best = Some((name, dist)),
                        Some((_, bd)) if dist < bd => best = Some((name, dist)),
                        _ => {}
                    }
                }
            }
        }
        match best {
            Some((name, 0)) => name.clone(),
            Some((name, offset)) => format!("{}+{}", name, offset),
            None => format!("{:06X}", addr),
        }
    }

    /// Build categorized dictionary entries from assembler labels.
    fn build_dict_entries(&self) -> Vec<DictEntry> {
        let mut entries: Vec<DictEntry> = self
            .labels
            .iter()
            .filter_map(|(name, &addr)| {
                // Skip internal labels (tx1, halt_loop, emit_poll, etc.)
                if name.starts_with("tx")
                    || name.starts_with("halt_")
                    || name.starts_with("emit_")
                    || name.starts_with("key_")
                    || name.starts_with("zbr_")
                    || name.starts_with("eq_")
                    || name.starts_with("lt_")
                    || name.starts_with("zeq_")
                    || name == "_start"
                {
                    return None;
                }

                let kind = if name.ends_with("_word") {
                    WordKind::ColonDef
                } else if name.ends_with("_thread") {
                    WordKind::Thread
                } else if name.starts_with("do_") {
                    WordKind::Primitive
                } else {
                    return None; // skip unrecognized
                };

                Some(DictEntry {
                    name: name.clone(),
                    addr,
                    kind,
                })
            })
            .collect();
        entries.sort_by_key(|e| e.addr);
        entries
    }

    /// Read the thread (parameter field) of a colon word starting at addr+3.
    fn read_word_thread(&self, cfa_addr: u32) -> Vec<(u32, String)> {
        let pfa = cfa_addr + CELL; // PFA = CFA + 3
        let mut thread = Vec::new();
        let mut addr = pfa;
        let mut next_is_literal = false;

        for _ in 0..32 {
            let word_addr = self.emulator.read_word(addr) & 0xFFFFFF;
            if next_is_literal {
                // After do_lit / do_branch / do_zbranch, the next cell is a value, not a CFA
                thread.push((word_addr, format!("{}", word_addr)));
                next_is_literal = false;
            } else {
                let name = self.resolve_addr(word_addr);
                next_is_literal = name == "do_lit" || name == "do_branch" || name == "do_zbranch";
                thread.push((word_addr, name.clone()));

                // Stop after do_exit or do_halt
                if name == "do_exit" || name == "do_halt" {
                    break;
                }
            }
            addr += CELL;
        }
        thread
    }

    /// Build caller chain by walking the return stack and resolving addresses.
    fn build_caller_chain(&self) -> Vec<(u32, String)> {
        let rsp = self.emulator.snapshot().regs[1];
        let rstack_base: u32 = 0x0F0000;
        let mut chain = Vec::new();
        // Only walk when RSP has been initialized to the return stack region
        if rsp == 0 || rsp >= rstack_base {
            return chain;
        }
        let mut addr = rstack_base;

        while addr > rsp && chain.len() < 16 {
            addr -= CELL;
            let ret_addr = self.emulator.read_word(addr) & 0xFFFFFF;
            // Return addresses point into a thread. Try to find which word's PFA this is in.
            let name = self.resolve_addr(ret_addr);
            chain.push((ret_addr, name));
        }
        chain.reverse();
        chain
    }

    /// Compute memory regions for the visualization bar.
    fn memory_regions(&self) -> Vec<(&'static str, &'static str, u32)> {
        let rsp = self.emulator.snapshot().regs[1];
        let sp = self.emulator.snapshot().regs[4];
        let rstack_base: u32 = 0x0F0000;
        let stack_top = self.stack_size.initial_sp();

        let kernel_end = self.program_end;
        let rstack_used = rstack_base.saturating_sub(rsp);
        let dstack_used = stack_top.saturating_sub(sp);

        // Simplified proportional regions (not to scale, but illustrative)
        vec![
            ("Kernel", "region-kernel", kernel_end),
            (
                "Free",
                "region-free",
                rstack_base
                    .saturating_sub(kernel_end)
                    .saturating_sub(rstack_used),
            ),
            ("RStack", "region-rstack", rstack_used),
            ("DStack", "region-dstack", dstack_used),
        ]
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
            labels: HashMap::new(),
            reverse_labels: HashMap::new(),
            assembled_lines: Vec::new(),
            dict_entries: Vec::new(),
            selected_word: None,
            bottom_tab: BottomTab::Dictionary,
            program_end: 0,
            switch_pressed: false,
            selected_demo: None,
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

                self.prev_regs = self.snapshot_regs();
                self.prev_pc = self.emulator.snapshot().pc;
                self.feed_uart_byte();

                let result = self.emulator.run_batch(BATCH_SIZE);
                self.collect_uart_output();

                if self.emulator.is_halted() {
                    self.running = false;
                    self.halted = true;
                } else if matches!(result.reason, cor24_emulator::StopReason::Breakpoint(_)) {
                    self.running = false;
                    self.emulator.pause();
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

            Msg::ToggleBreakpoint(addr) => {
                if self.emulator.has_breakpoint(addr) {
                    self.emulator.remove_breakpoint(addr);
                } else {
                    self.emulator.add_breakpoint(addr);
                }
                true
            }

            Msg::LoadDemo(index) => {
                if let Some(demo) = DEMOS.get(index) {
                    self.selected_demo = Some(index);
                    // Switch tier if needed and reset
                    self.tier = demo.tier;
                    self.load_binary(ctx);
                    // Feed demo source line-by-line into UART
                    for line in demo.source.lines() {
                        let trimmed = line.trim();
                        // Skip empty lines and comments
                        if trimmed.is_empty() || trimmed.starts_with('\\') {
                            continue;
                        }
                        for b in trimmed.bytes() {
                            self.uart_rx_queue.push_back(b);
                        }
                        self.uart_rx_queue.push_back(b'\n');
                    }
                    // Auto-run
                    self.running = true;
                    self.emulator.resume();
                    self.schedule_tick(ctx);
                }
                true
            }

            Msg::ToggleSwitch => {
                self.switch_pressed = !self.switch_pressed;
                self.emulator.set_button_pressed(self.switch_pressed);
                true
            }

            Msg::SelectWord(name) => {
                if self.selected_word.as_ref() == Some(&name) {
                    self.selected_word = None; // toggle off
                } else {
                    self.selected_word = Some(name);
                }
                true
            }

            Msg::SetBottomTab(tab) => {
                self.bottom_tab = tab;
                true
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
        let caller_chain = self.build_caller_chain();
        let regions = self.memory_regions();

        let reg_names = [
            "r0/W", "r1/RSP", "r2/IP", "r3/fp", "sp/DSP", "r5/zc", "r6/iv", "r7/ir",
        ];

        // Compute total for region bar proportions.
        let region_total: u32 = regions.iter().map(|(_, _, sz)| *sz).sum::<u32>().max(1);

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
                            "Interpreter" => ForthTier::Interpreter,
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

                    <select class="demo-select" onchange={ctx.link().callback(|e: Event| {
                        let select: HtmlInputElement = e.target_unchecked_into();
                        let idx: usize = select.value().parse().unwrap_or(usize::MAX);
                        Msg::LoadDemo(idx)
                    })}>
                        <option value="" selected={self.selected_demo.is_none()}>
                            {"Demo..."}
                        </option>
                        { for DEMOS.iter().enumerate().map(|(i, demo)| {
                            let sel = self.selected_demo == Some(i);
                            html! {
                                <option value={i.to_string()} selected={sel}>
                                    { &demo.title }
                                </option>
                            }
                        })}
                    </select>
                </div>

                // Memory map bar
                <div class="memory-map">
                    <span class="memory-map-label">{"Memory"}</span>
                    <div class="region-bar">
                        { for regions.iter().map(|(name, class, size)| {
                            let pct = (*size as f64 / region_total as f64 * 100.0).max(2.0);
                            let style = format!("width: {}%", pct);
                            html! {
                                <div class={classes!("region", *class)} style={style}
                                     title={format!("{}: {} bytes", name, size)}>
                                    { if pct > 8.0 { *name } else { "" } }
                                </div>
                            }
                        })}
                    </div>
                </div>

                // Main panels
                <div class="panels">
                    // Output / terminal
                    <div class="output-panel">
                        // Floating hardware panel (top-right)
                        <div class="hw-float">
                            <div class="hw-row">
                                <span class="hw-label">{"D2"}</span>
                                <div class={if snap.led & 1 != 0 { "led led-on" } else { "led led-off" }} />
                            </div>
                            <div class="hw-row">
                                <span class="hw-label">{"S2"}</span>
                                <div class={if self.switch_pressed { "switch switch-on" } else { "switch switch-off" }}
                                     onclick={ctx.link().callback(|_| Msg::ToggleSwitch)} />
                            </div>
                            <div class="hw-sep" />
                            <div class="hw-stats">
                                <span class="hw-stat-label">{"Cycles"}</span>
                                <span class="hw-stat-value">{ format!("{}", snap.cycles) }</span>
                                <span class="hw-stat-label">{"Instrs"}</span>
                                <span class="hw-stat-value">{ format!("{}", snap.instructions) }</span>
                            </div>
                        </div>
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

                    // Side panel: registers + stacks + disassembly
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

                        // Caller Chain
                        { if !caller_chain.is_empty() {
                            html! {
                                <div class="panel-section">
                                    <h3>{"Caller Chain"}</h3>
                                    <div class="caller-chain">
                                        { for caller_chain.iter().enumerate().map(|(i, (addr, name))| {
                                            html! {
                                                <div class="caller-entry">
                                                    <span class="caller-depth">
                                                        { format!("{}.", i) }
                                                    </span>
                                                    <span class="caller-name">{ name }</span>
                                                    <span class="caller-addr">
                                                        { format!("{:06X}", addr) }
                                                    </span>
                                                </div>
                                            }
                                        })}
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }}

                        // Disassembly with breakpoints
                        <div class="panel-section">
                            <h3>{"Disassembly"}</h3>
                            <div class="disasm-view">
                                { for disasm.iter().map(|(addr, mnemonic, is_current)| {
                                    let has_bp = self.emulator.has_breakpoint(*addr);
                                    let a = *addr;
                                    let label = self.reverse_labels.get(addr).cloned();
                                    html! {
                                        <>
                                            { if let Some(lbl) = label {
                                                html! {
                                                    <div class="disasm-label">{ format!("{}:", lbl) }</div>
                                                }
                                            } else {
                                                html! {}
                                            }}
                                            <div class={classes!(
                                                "disasm-line",
                                                is_current.then_some("disasm-current"),
                                                has_bp.then_some("disasm-breakpoint")
                                            )}
                                            onclick={ctx.link().callback(move |_| Msg::ToggleBreakpoint(a))}
                                            title="Click to toggle breakpoint">
                                                <span class="disasm-bp-gutter">
                                                    { if has_bp { "\u{25cf}" } else { "\u{00a0}" } }
                                                </span>
                                                <span class="disasm-addr">
                                                    { format!("{:06X}", addr) }
                                                </span>
                                                <span class="disasm-instr">
                                                    { mnemonic }
                                                </span>
                                            </div>
                                        </>
                                    }
                                })}
                            </div>
                        </div>

                        // Bottom tabbed panel
                        <div class="panel-section bottom-tabs">
                            <div class="tab-bar">
                                <button
                                    class={classes!(
                                        "tab-btn",
                                        (self.bottom_tab == BottomTab::Dictionary).then_some("active")
                                    )}
                                    onclick={ctx.link().callback(|_| Msg::SetBottomTab(BottomTab::Dictionary))}>
                                    {"Dictionary"}
                                </button>
                                <button
                                    class={classes!(
                                        "tab-btn",
                                        (self.bottom_tab == BottomTab::CompileLog).then_some("active")
                                    )}
                                    onclick={ctx.link().callback(|_| Msg::SetBottomTab(BottomTab::CompileLog))}>
                                    {"Compile Log"}
                                </button>
                            </div>

                            { match self.bottom_tab {
                                BottomTab::Dictionary => self.view_dictionary(ctx),
                                BottomTab::CompileLog => self.view_compile_log(),
                            }}
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
                    { if !self.emulator.breakpoints().is_empty() {
                        html! {
                            <div class="status-item">
                                <span class="status-label">{"BP:"}</span>
                                <span class="status-value">
                                    { format!("{}", self.emulator.breakpoints().len()) }
                                </span>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        }
    }
}

impl Debugger {
    /// Render the dictionary browser + word inspector tab.
    fn view_dictionary(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="dict-panel">
                <div class="dict-list">
                    { for self.dict_entries.iter().map(|entry| {
                        let name = entry.name.clone();
                        let selected = self.selected_word.as_ref() == Some(&entry.name);
                        let kind_class = match entry.kind {
                            WordKind::Primitive => "dict-primitive",
                            WordKind::ColonDef => "dict-colon",
                            WordKind::Thread => "dict-thread",
                        };
                        let kind_label = match entry.kind {
                            WordKind::Primitive => "PRIM",
                            WordKind::ColonDef => "COLON",
                            WordKind::Thread => "THREAD",
                        };
                        html! {
                            <div class={classes!(
                                "dict-entry",
                                kind_class,
                                selected.then_some("dict-selected")
                            )}
                            onclick={ctx.link().callback(move |_| Msg::SelectWord(name.clone()))}>
                                <span class="dict-addr">
                                    { format!("{:06X}", entry.addr) }
                                </span>
                                <span class="dict-name">{ &entry.name }</span>
                                <span class="dict-kind">{ kind_label }</span>
                            </div>
                        }
                    })}
                </div>

                // Word inspector
                { if let Some(ref word_name) = self.selected_word {
                    if let Some(entry) = self.dict_entries.iter().find(|e| &e.name == word_name) {
                        match entry.kind {
                            WordKind::ColonDef => {
                                let thread = self.read_word_thread(entry.addr);
                                html! {
                                    <div class="word-inspector">
                                        <h4>{ format!(": {}", word_name) }</h4>
                                        <div class="word-thread">
                                            { for thread.iter().map(|(addr, name)| {
                                                // Detect literal values: if previous entry was do_lit,
                                                // this is the literal value
                                                html! {
                                                    <div class="thread-entry">
                                                        <span class="thread-addr">
                                                            { format!("{:06X}", addr) }
                                                        </span>
                                                        <span class="thread-name">{ name }</span>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    </div>
                                }
                            }
                            WordKind::Thread => {
                                let thread = self.read_word_thread(entry.addr.wrapping_sub(CELL));
                                html! {
                                    <div class="word-inspector">
                                        <h4>{ format!("thread: {}", word_name) }</h4>
                                        <div class="word-thread">
                                            { for thread.iter().map(|(addr, name)| {
                                                html! {
                                                    <div class="thread-entry">
                                                        <span class="thread-addr">
                                                            { format!("{:06X}", addr) }
                                                        </span>
                                                        <span class="thread-name">{ name }</span>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    </div>
                                }
                            }
                            WordKind::Primitive => {
                                let disasm = self.emulator.disassemble(entry.addr, 16);
                                html! {
                                    <div class="word-inspector">
                                        <h4>{ format!("primitive: {}", word_name) }</h4>
                                        <div class="prim-disasm">
                                            { for disasm.iter().take_while(|(addr, _, _)| {
                                                // Stop if we hit another label (except our own)
                                                *addr == entry.addr
                                                    || !self.reverse_labels.contains_key(addr)
                                            }).map(|(addr, mnemonic, _)| {
                                                html! {
                                                    <div class="disasm-line">
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
                                }
                            }
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {
                        <div class="word-inspector-empty">
                            {"Click a word to inspect"}
                        </div>
                    }
                }}
            </div>
        }
    }

    /// Render the compile log tab.
    fn view_compile_log(&self) -> Html {
        html! {
            <div class="compile-log">
                { for self.assembled_lines.iter().filter(|line| {
                    // Show lines that produced bytes or have labels
                    !line.bytes.is_empty() || line.label.is_some()
                }).map(|line| {
                    let has_label = line.label.is_some();
                    html! {
                        <div class={classes!(
                            "log-line",
                            has_label.then_some("log-label-line")
                        )}>
                            <span class="log-addr">
                                { format!("{:06X}", line.address) }
                            </span>
                            <span class="log-bytes">
                                { line.bytes.iter()
                                    .take(6)
                                    .map(|b| format!("{:02X}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                }
                            </span>
                            <span class="log-source">
                                { if let Some(ref label) = line.label {
                                    format!("{}:  {}", label, line.source.trim())
                                } else {
                                    line.source.trim().to_string()
                                }}
                            </span>
                        </div>
                    }
                })}
            </div>
        }
    }
}
