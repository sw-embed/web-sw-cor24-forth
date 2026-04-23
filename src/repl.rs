//! Shared REPL component for the forth-in-forth and forth-on-forthish tabs.
//!
//! Both tabs boot identically: assemble a kernel, feed core/*.fth tiers over
//! UART. The kernel source, core files, and demo list are passed as
//! `ReplProps` so one component serves both tabs. The forth.s debugger (tab 1)
//! is a separate component (`src/debugger.rs`) — this REPL is intentionally
//! simpler: Run/Stop/Reset + demo dropdown + file upload + LED/switch, no
//! step / breakpoints / register-or-stack panels.

use crate::config::StackSize;
use crate::demos::Demo;
use crate::snapshot;
use cor24_emulator::{Assembler, EmulatorCore};
use gloo::file::File;
use gloo::file::callbacks::FileReader;
use gloo::timers::callback::Timeout;
use std::collections::{HashMap, VecDeque};
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;

/// Snapshot cache toggle. When false, both the build-time embedded blob
/// and the localStorage cache are bypassed — every visit takes the slow
/// UART bootstrap through `QUIT`. Kept off while we benchmark kernel-side
/// perf work (e.g. XMX FIND hash in `sw-cor24-forth`) so the wall-clock
/// we observe is purely the kernel's, not our cache's.
const SNAPSHOT_CACHE_ENABLED: bool = false;

/// Per-tick instruction budget once the REPL is interactive.
const BATCH_SIZE: u64 = 50_000;
/// Per-tick instruction budget while draining the core/*.fth queue. Tuned
/// so each tick completes in ~100–200 ms of UI-thread time so the page
/// stays responsive and the status bar keeps updating.
const BOOTSTRAP_BATCH: u64 = 600_000;
/// Adaptive pump-loop sub-batch sizes.
///   `PUMP_TINY` is used when the CPU is spinning in a UART-poll loop with
///   bytes still waiting in the RX queue — just enough instructions to let
///   the CPU consume the byte we just fed and either want another or start
///   real compile work. Anything larger is busy-waiting at ~19k instr/cheap
///   byte, which is the single biggest source of wasted time during boot.
///   `PUMP_BIG` is used when the CPU is doing real compile work (not at a
///   poll address) — let it make meaningful progress between poll checks.
const PUMP_TINY: u64 = 2_000;
const PUMP_BIG: u64 = 50_000;
/// Tick interval. Shorter during boot (less scheduler overhead per tick)
/// and restored to the interactive value once ready, so steady-state idle
/// doesn't burn CPU.
const TICK_MS_BOOT: u32 = 5;
const TICK_MS_INTERACTIVE: u32 = 25;

/// Per-instance configuration. Each tab passes its own kernel source,
/// tiered core files, curated demo list, and a short label used in the
/// in-REPL About dialog title and status bar.
#[derive(Properties, PartialEq, Clone)]
pub struct ReplProps {
    pub label: &'static str,
    pub kernel_src: &'static str,
    pub core_files: &'static [(&'static str, &'static str)],
    pub demos: &'static [Demo],
    /// Opens the global Help dialog (User Guide / Reference / Tutorial).
    /// Fired by the "Help" button in the REPL toolbar.
    pub on_open_help: Callback<()>,
}

pub enum Msg {
    Init,
    Tick,
    InputChanged(String),
    SendInput,
    InputKeyDown(KeyboardEvent),
    Run,
    Stop,
    Reset,
    LoadDemo(usize),
    ToggleSwitch,
    FileChanged(Event),
    LoadFile(String),
    RunFile,
    CancelFile,
    ToggleAbout,
    ToggleHistory,
}

pub struct ForthRepl {
    emulator: EmulatorCore,
    output: String,
    input: String,
    running: bool,
    halted: bool,
    booted: bool,
    _tick_handle: Option<Timeout>,
    uart_rx_queue: VecDeque<u8>,
    output_ref: NodeRef,
    input_ref: NodeRef,
    file_input_ref: NodeRef,
    waiting_for_input: bool,
    uart_poll_addrs: Vec<u32>,
    show_about: bool,
    show_history: bool,
    pending_preview: Option<(String, String)>,
    history: Vec<String>,
    full_log: Vec<String>,
    history_pos: isize,
    history_saved: String,
    switch_pressed: bool,
    selected_demo: Option<usize>,
    _file_reader: Option<FileReader>,
}

impl ForthRepl {
    fn load_binary(&mut self, ctx: &Context<Self>) {
        let props = ctx.props();
        let mut asm = Assembler::new();
        let result = asm.assemble(props.kernel_src);

        if !result.errors.is_empty() {
            self.output = "Kernel assembly errors:\n".to_string();
            for e in &result.errors {
                self.output.push_str(e);
                self.output.push('\n');
            }
            return;
        }

        let labels: HashMap<String, u32> = result.labels.clone();
        self.uart_poll_addrs = [
            "key_poll",
            "word_skip_rx",
            "word_skip_rx2",
            "word_read_rx",
            "word_read_rx2",
            "create_skip_rx",
            "create_skip_rx2",
            "create_read_rx",
            "create_read_rx2",
        ]
        .iter()
        .filter_map(|name| labels.get(*name).copied())
        .collect();

        let was_switch_pressed = self.switch_pressed;
        self.emulator.hard_reset();
        if was_switch_pressed {
            self.emulator.set_button_pressed(true);
        }
        self.emulator.load_program(0, &result.bytes);
        self.emulator.load_program_extent(result.bytes.len() as u32);
        self.emulator.set_pc(0);
        self.output.clear();
        self.halted = false;
        self.booted = false;
        self.uart_rx_queue.clear();
        self.waiting_for_input = false;
        self.selected_demo = None;
        self.switch_pressed = was_switch_pressed;

        // Fast paths (embedded blob + localStorage) are gated on
        // SNAPSHOT_CACHE_ENABLED so we can A/B-test kernel-side perf work
        // without interference.
        if SNAPSHOT_CACHE_ENABLED {
            let hash = snapshot::content_hash(props.kernel_src, props.core_files);
            if snapshot::restore_from_embedded(&mut self.emulator, hash) {
                if was_switch_pressed {
                    self.emulator.set_button_pressed(true);
                }
                self.booted = true;
                self.waiting_for_input = true;
                self.running = true;
                self.schedule_tick(ctx);
                return;
            }
            let key = snapshot::content_key(props.kernel_src, props.core_files);
            if let Some(snap) = snapshot::load(&key)
                && snapshot::restore(&mut self.emulator, &snap)
            {
                if was_switch_pressed {
                    self.emulator.set_button_pressed(true);
                }
                self.booted = true;
                self.waiting_for_input = true;
                self.running = true;
                self.schedule_tick(ctx);
                return;
            }
        }

        // Slow path: preload core/*.fth into the RX queue (raw bytes,
        // LF-normalized, no trim, one newline between tiers) and let the
        // kernel's QUIT loop compile each line. On completion we'll capture
        // a snapshot so the next load takes the fast path above.
        for (i, (_name, contents)) in props.core_files.iter().enumerate() {
            if i > 0 && self.uart_rx_queue.back().is_none_or(|&b| b != b'\n') {
                self.uart_rx_queue.push_back(b'\n');
            }
            for b in contents.bytes() {
                if b == b'\r' {
                    continue; // strip CR from CRLF
                }
                self.uart_rx_queue.push_back(b);
            }
            if self.uart_rx_queue.back().is_none_or(|&b| b != b'\n') {
                self.uart_rx_queue.push_back(b'\n');
            }
        }

        self.running = true;
        self.emulator.resume();
        self.schedule_tick(ctx);
    }

    fn schedule_tick(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        let ms = if self.booted {
            TICK_MS_INTERACTIVE
        } else {
            TICK_MS_BOOT
        };
        self._tick_handle = Some(Timeout::new(ms, move || {
            link.send_message(Msg::Tick);
        }));
    }

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

    fn collect_uart_output(&mut self) -> bool {
        let uart = self.emulator.get_uart_output();
        if !uart.is_empty() {
            self.output.push_str(uart);
            self.emulator.clear_uart_output();
            true
        } else {
            false
        }
    }

    fn auto_scroll(&self) {
        if let Some(el) = self.output_ref.cast::<HtmlElement>() {
            el.set_scroll_top(el.scroll_height());
        }
    }
}

impl Component for ForthRepl {
    type Message = Msg;
    type Properties = ReplProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_message(Msg::Init);
        Self {
            emulator: EmulatorCore::new(),
            output: String::new(),
            input: String::new(),
            running: false,
            halted: false,
            booted: false,
            _tick_handle: None,
            uart_rx_queue: VecDeque::new(),
            output_ref: NodeRef::default(),
            input_ref: NodeRef::default(),
            file_input_ref: NodeRef::default(),
            waiting_for_input: false,
            uart_poll_addrs: Vec::new(),
            show_about: false,
            show_history: false,
            pending_preview: None,
            history: Vec::new(),
            full_log: Vec::new(),
            history_pos: -1,
            history_saved: String::new(),
            switch_pressed: false,
            selected_demo: None,
            _file_reader: None,
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

                // Adaptive pump-loop. Each iteration:
                //   1. Check if the CPU is busy-waiting at a UART poll.
                //   2. If polling with empty queue → truly done, break.
                //   3. If polling with bytes to feed → push one byte, run
                //      a TINY sub-batch (~2k instr, just enough for the CPU
                //      to consume the byte and either want another or
                //      start real compile work). This is the key win —
                //      previously we ran ~20k between feeds, ~19.5k of
                //      which the CPU burned spinning in key_poll waiting
                //      for the next byte.
                //   4. If not polling → CPU is doing real compile work,
                //      run a BIG sub-batch to make progress.
                let budget = if self.booted {
                    BATCH_SIZE
                } else {
                    BOOTSTRAP_BATCH
                };
                let mut instructions_used: u64 = 0;
                let mut last_reason = cor24_emulator::StopReason::CycleLimit;
                let mut halted = false;
                loop {
                    let pc = self.emulator.snapshot().pc;
                    let idle_polling = self
                        .uart_poll_addrs
                        .iter()
                        .any(|&addr| pc >= addr && pc < addr + 16);

                    if idle_polling && self.uart_rx_queue.is_empty() {
                        break;
                    }

                    let chunk_size = if idle_polling {
                        self.feed_uart_byte();
                        PUMP_TINY
                    } else {
                        PUMP_BIG
                    };

                    let remaining = budget.saturating_sub(instructions_used);
                    if remaining == 0 {
                        break;
                    }
                    let chunk = remaining.min(chunk_size);
                    let result = self.emulator.run_batch(chunk);
                    last_reason = result.reason.clone();
                    // Safety: if the emulator isn't advancing (paused,
                    // halted), bail out rather than spin.
                    if result.instructions_run == 0 {
                        if matches!(last_reason, cor24_emulator::StopReason::Halted) {
                            halted = true;
                        }
                        break;
                    }
                    instructions_used += result.instructions_run;
                    if matches!(last_reason, cor24_emulator::StopReason::Halted) {
                        halted = true;
                        break;
                    }
                }

                // Show boot output too — users want to see the "ok" stream as
                // proof of life. The final `self.output.clear()` below wipes
                // it right before the first interactive prompt.
                let had_output = self.collect_uart_output();

                let pc = self.emulator.snapshot().pc;
                let was_waiting = self.waiting_for_input;
                self.waiting_for_input =
                    matches!(last_reason, cor24_emulator::StopReason::CycleLimit)
                        && self
                            .uart_poll_addrs
                            .iter()
                            .any(|&addr| pc >= addr && pc < addr + 16);

                // First idle with empty queue = bootstrap complete. When
                // the snapshot cache is enabled, also save so subsequent
                // loads take the fast path.
                if !self.booted && self.waiting_for_input && self.uart_rx_queue.is_empty() {
                    self.booted = true;
                    self.output.clear();
                    if SNAPSHOT_CACHE_ENABLED {
                        let props = ctx.props();
                        let key = snapshot::content_key(props.kernel_src, props.core_files);
                        let snap = snapshot::capture(&self.emulator);
                        let _ = snapshot::save(&key, &snap);
                    }
                }

                if halted {
                    self.running = false;
                    self.halted = true;
                } else if self.running {
                    self.schedule_tick(ctx);
                }

                if self.waiting_for_input && was_waiting && !had_output {
                    return false;
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
                if self.running && !self.waiting_for_input {
                    self.output.push_str("[input queued — interpreter busy]\n");
                }
                self.history.push(self.input.clone());
                self.full_log.push(self.input.clone());
                self.history_pos = -1;
                self.history_saved.clear();

                for b in self.input.bytes() {
                    self.uart_rx_queue.push_back(b);
                }
                self.uart_rx_queue.push_back(b'\n');
                self.input.clear();
                self.waiting_for_input = false;

                if !self.running && !self.halted {
                    self.running = true;
                    self.emulator.resume();
                    self.schedule_tick(ctx);
                }
                true
            }

            Msg::InputKeyDown(e) => {
                match e.key().as_str() {
                    "Enter" => {
                        ctx.link().send_message(Msg::SendInput);
                    }
                    "ArrowUp" => {
                        e.prevent_default();
                        if !self.history.is_empty() {
                            if self.history_pos == -1 {
                                self.history_saved = self.input.clone();
                                self.history_pos = self.history.len() as isize - 1;
                            } else if self.history_pos > 0 {
                                self.history_pos -= 1;
                            }
                            self.input = self.history[self.history_pos as usize].clone();
                            return true;
                        }
                    }
                    "ArrowDown" => {
                        e.prevent_default();
                        if self.history_pos >= 0 {
                            self.history_pos += 1;
                            if self.history_pos >= self.history.len() as isize {
                                self.history_pos = -1;
                                self.input = self.history_saved.clone();
                            } else {
                                self.input = self.history[self.history_pos as usize].clone();
                            }
                            return true;
                        }
                    }
                    _ => {}
                }
                false
            }

            Msg::Run => {
                if self.halted || self.running {
                    return false;
                }
                self.running = true;
                self.emulator.resume();
                self.schedule_tick(ctx);
                true
            }

            Msg::Stop => {
                if !self.running {
                    return false;
                }
                self.running = false;
                self.emulator.pause();
                self._tick_handle = None;
                true
            }

            Msg::Reset => {
                self.load_binary(ctx);
                true
            }

            Msg::LoadDemo(index) => {
                if let Some(demo) = ctx.props().demos.get(index) {
                    self.selected_demo = Some(index);
                    self.pending_preview =
                        Some((format!("Demo: {}", demo.title), demo.source.to_string()));
                }
                true
            }

            Msg::ToggleSwitch => {
                self.switch_pressed = !self.switch_pressed;
                self.emulator.set_button_pressed(self.switch_pressed);
                true
            }

            Msg::FileChanged(e) => {
                let input: HtmlInputElement = e.target_unchecked_into();
                if let Some(file_list) = input.files()
                    && let Some(file) = file_list.get(0)
                {
                    let file = File::from(file);
                    let link = ctx.link().clone();
                    let reader = gloo::file::callbacks::read_as_text(&file, move |result| {
                        if let Ok(text) = result {
                            link.send_message(Msg::LoadFile(text));
                        }
                    });
                    self._file_reader = Some(reader);
                }
                input.set_value("");
                false
            }

            Msg::LoadFile(contents) => {
                self.pending_preview = Some(("Uploaded File".to_string(), contents));
                true
            }

            Msg::RunFile => {
                if let Some((title, contents)) = self.pending_preview.take() {
                    self.selected_demo = None;
                    self.full_log.push(format!("\\ --- {} ---", title));
                    for line in contents.lines() {
                        let trimmed = line.trim_end_matches('\r').trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        self.full_log.push(trimmed.to_string());
                        self.history.push(trimmed.to_string());
                        for b in trimmed.bytes() {
                            self.uart_rx_queue.push_back(b);
                        }
                        self.uart_rx_queue.push_back(b'\n');
                    }
                    self.waiting_for_input = false;
                    if !self.running && !self.halted {
                        self.running = true;
                        self.emulator.resume();
                        self.schedule_tick(ctx);
                    }
                }
                true
            }

            Msg::CancelFile => {
                self.pending_preview = None;
                true
            }

            Msg::ToggleAbout => {
                self.show_about = !self.show_about;
                true
            }

            Msg::ToggleHistory => {
                self.show_history = !self.show_history;
                true
            }
        }
    }

    fn rendered(&mut self, _ctx: &Context<Self>, first_render: bool) {
        self.auto_scroll();
        // Focus the input on first render only. Subsequent renders leave
        // focus where the user put it — critical so drag-to-select in the
        // output area isn't cancelled by a mid-drag re-render.
        if first_render
            && !self.show_about
            && let Some(el) = self.input_ref.cast::<HtmlElement>()
        {
            let _ = el.focus();
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let snap = self.emulator.snapshot();
        let status_text;
        let status_label: &str = if self.halted {
            "Halted"
        } else if !self.booted {
            status_text = format!(
                "Booting core/*.fth — {} bytes left, {} cycles",
                self.uart_rx_queue.len(),
                snap.cycles,
            );
            &status_text
        } else if self.waiting_for_input {
            "Ready"
        } else if self.running {
            "Running"
        } else {
            "Stopped"
        };

        // Lock out UI during bootstrap — the dropdown would close on every
        // re-render, buttons would fight the pump loop, and a user typing
        // would land bytes ahead of the core/*.fth stream.
        let ui_locked = !self.booted;

        html! {
            <div class="debugger repl-simple">
                // Toolbar
                <div class="toolbar">
                    <button onclick={ctx.link().callback(|_| Msg::Run)}
                            disabled={ui_locked || self.running || self.halted}
                            class={if self.running { "active" } else { "" }}>
                        {"Run"}
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::Stop)}
                            disabled={ui_locked || !self.running}>
                        {"Stop"}
                    </button>
                    <button onclick={ctx.link().callback(|_| Msg::Reset)}
                            disabled={ui_locked}>
                        {"Reset"}
                    </button>

                    <select class="demo-select"
                            disabled={ui_locked}
                            onchange={ctx.link().callback(|e: Event| {
                                let select: HtmlInputElement = e.target_unchecked_into();
                                let idx: usize = select.value().parse().unwrap_or(usize::MAX);
                                Msg::LoadDemo(idx)
                            })}>
                        <option value="" selected={self.selected_demo.is_none()}>
                            {"Demo..."}
                        </option>
                        { for ctx.props().demos.iter().enumerate().map(|(i, demo)| {
                            let sel = self.selected_demo == Some(i);
                            html! {
                                <option value={i.to_string()} selected={sel}>
                                    { demo.title }
                                </option>
                            }
                        })}
                    </select>

                    <button class="upload-btn"
                            disabled={ui_locked}
                            onclick={
                                let file_ref = self.file_input_ref.clone();
                                Callback::from(move |_: MouseEvent| {
                                    if let Some(input) = file_ref.cast::<HtmlInputElement>() {
                                        input.click();
                                    }
                                })
                            }>
                        {"Load .fth"}
                    </button>
                    <input
                        type="file"
                        accept=".fth,.fs"
                        ref={self.file_input_ref.clone()}
                        style="display:none"
                        onchange={ctx.link().callback(Msg::FileChanged)}
                    />
                    <button class="about-btn"
                            disabled={ui_locked}
                            onclick={ctx.link().callback(|_| Msg::ToggleHistory)}>
                        {"History"}
                    </button>
                    <button class="about-btn"
                            onclick={ctx.link().callback(|_| Msg::ToggleAbout)}>
                        {"About"}
                    </button>
                    <button class="help-btn"
                            onclick={ctx.props().on_open_help.reform(|_| ())}>
                        {"Help"}
                    </button>
                </div>

                // Main: single full-width output/input panel
                <div class="panels">
                    <div class="output-panel repl-output-panel">
                        <div class="hw-float">
                            <div class="hw-row">
                                <span class="hw-label">{"D2"}</span>
                                <div class={if snap.led & 1 == 0 { "led led-on" } else { "led led-off" }} />
                            </div>
                            <div class="hw-row">
                                <span class="hw-label">{"S2"}</span>
                                <div class={classes!(
                                        "switch",
                                        if self.switch_pressed { "switch-on" } else { "switch-off" },
                                        ui_locked.then_some("switch-disabled")
                                    )}
                                     onclick={
                                        let link = ctx.link().clone();
                                        Callback::from(move |_: MouseEvent| {
                                            if !ui_locked {
                                                link.send_message(Msg::ToggleSwitch);
                                            }
                                        })
                                     } />
                            </div>
                        </div>
                        <div class="output" ref={self.output_ref.clone()}>{ &self.output }</div>
                        <div class={if self.waiting_for_input { "input-bar input-ready" }
                                    else if self.running { "input-bar input-busy" }
                                    else { "input-bar" }}>
                            <span class="prompt">
                                { if self.running && !self.waiting_for_input { "\u{25F3} " } else { "> " } }
                            </span>
                            <input
                                type="text"
                                ref={self.input_ref.clone()}
                                value={self.input.clone()}
                                disabled={ui_locked}
                                oninput={ctx.link().callback(|e: InputEvent| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    Msg::InputChanged(input.value())
                                })}
                                onkeydown={ctx.link().callback(Msg::InputKeyDown)}
                                placeholder={if !self.booted {
                                    "Booting core/*.fth — UI locked until ready..."
                                } else if self.running && !self.waiting_for_input {
                                    "Running..."
                                } else {
                                    "Try: WORDS  or  : SQUARE DUP * ;  5 SQUARE ."
                                }}
                            />
                        </div>
                    </div>
                </div>

                // File/Demo preview dialog
                { if let Some((ref title, ref contents)) = self.pending_preview {
                    html! {
                        <div class="about-overlay" onclick={ctx.link().callback(|_| Msg::CancelFile)}>
                            <div class="about-dialog file-preview-dialog"
                                 onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <h2>{ title }</h2>
                                <pre class="file-preview">{ contents }</pre>
                                <div class="dialog-buttons">
                                    <button class="run-btn" onclick={ctx.link().callback(|_| Msg::RunFile)}>
                                        {"Run"}
                                    </button>
                                    <button onclick={ctx.link().callback(|_| Msg::CancelFile)}>
                                        {"Cancel"}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                } else { html! {} }}

                // History dialog
                { if self.show_history {
                    html! {
                        <div class="about-overlay" onclick={ctx.link().callback(|_| Msg::ToggleHistory)}>
                            <div class="about-dialog file-preview-dialog"
                                 onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <h2>{"Command History"}</h2>
                                { if self.full_log.is_empty() {
                                    html! { <p>{"No commands yet."}</p> }
                                } else {
                                    html! {
                                        <pre class="file-preview">
                                            { self.full_log.iter().enumerate().map(|(i, cmd)| {
                                                format!("{:3}  {}\n", i + 1, cmd)
                                            }).collect::<String>() }
                                        </pre>
                                    }
                                }}
                                <p class="about-hint">{"Use "}<strong>{"Up/Down Arrow"}</strong>{" in the input to recall commands."}</p>
                                <button onclick={ctx.link().callback(|_| Msg::ToggleHistory)}>{"Close"}</button>
                            </div>
                        </div>
                    }
                } else { html! {} }}

                // Status bar
                <div class="status-bar">
                    <div class="status-item">
                        <span class="status-label">{"Status:"}</span>
                        <span class="status-value">{ status_label }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Kernel:"}</span>
                        <span class="status-value">{ ctx.props().label }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Stack:"}</span>
                        <span class="status-value">{ StackSize::ThreeKb.label() }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Cycles:"}</span>
                        <span class="status-value">{ format!("{}", snap.cycles) }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Instrs:"}</span>
                        <span class="status-value">{ format!("{}", snap.instructions) }</span>
                    </div>
                </div>

                // About dialog
                { if self.show_about {
                    html! {
                        <div class="about-overlay" onclick={ctx.link().callback(|_| Msg::ToggleAbout)}>
                            <div class="about-dialog" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <h2>{ ctx.props().label }</h2>
                                <p>{"A self-hosting Forth: the asm kernel holds only what needs machine code, and the rest (IF/THEN/ELSE, \\ and ( comments, . CR WORDS .S SEE, ...) is defined in Forth loaded at boot."}</p>
                                <h3>{"Try these:"}</h3>
                                <pre>{concat!(
                                    "WORDS                list every word (asm + Forth)\n",
                                    ": SQUARE DUP * ;\n",
                                    "5 SQUARE .\n",
                                    "SEE SQUARE           decompile the colon def\n",
                                    "SEE IF               see IF is itself in Forth\n",
                                    "DEPTH .S             stack depth + contents\n",
                                    "VER                  print kernel banner\n",
                                )}</pre>
                                <p class="about-hint">{"Boot preloads "}
                                    <strong>{"minimal → lowlevel → midlevel → highlevel"}</strong>
                                    {" core/*.fth. "}<strong>{"Up/Down"}</strong>{" recalls commands."}</p>
                                <button onclick={ctx.link().callback(|_| Msg::ToggleAbout)}>{"Close"}</button>
                            </div>
                        </div>
                    }
                } else { html! {} }}
            </div>
        }
    }
}
