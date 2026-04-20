//! forth-in-forth REPL — self-hosted kernel with core/*.fth preloaded.
//!
//! Boots the `forth-in-forth/kernel.s` assembly, then feeds the four core
//! tier files into the UART RX queue so the user lands at a prompt with
//! the full vocabulary (including `SEE`, `WORDS`, `.S`) already defined.

use crate::config::StackSize;
use crate::demos::FIF_DEMOS;
use cor24_emulator::{Assembler, EmulatorCore};
use gloo::file::File;
use gloo::file::callbacks::FileReader;
use gloo::timers::callback::Timeout;
use std::collections::{HashMap, VecDeque};
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;

/// Per-tick instruction budget once the REPL is interactive.
const BATCH_SIZE: u64 = 50_000;
/// Per-tick instruction budget while draining the core/*.fth queue. Tuned
/// so each tick completes in ~100–200 ms of UI-thread time so the page
/// stays responsive and the status bar keeps updating.
const BOOTSTRAP_BATCH: u64 = 500_000;
/// Sub-batch size for the bootstrap pump-loop. After each sub-batch we feed
/// another UART byte (if the RX buffer is empty) so the interpreter can
/// consume bytes as fast as it wants instead of 1 per tick.
const PUMP_SUB_BATCH: u64 = 20_000;
const TICK_MS: u32 = 25;

/// forth-in-forth kernel source.
const KERNEL_SRC: &str = include_str!("../../sw-cor24-forth/forth-in-forth/kernel.s");

/// Ordered core tier files, loaded at boot.
const CORE_FILES: &[(&str, &str)] = &[
    (
        "minimal",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/minimal.fth"),
    ),
    (
        "lowlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/lowlevel.fth"),
    ),
    (
        "midlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/midlevel.fth"),
    ),
    (
        "highlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/highlevel.fth"),
    ),
];

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
        let mut asm = Assembler::new();
        let result = asm.assemble(KERNEL_SRC);

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

        // Preload core/*.fth into the RX queue — raw bytes, LF-normalized, no trim.
        // One newline between tiers (mirrors `cat file1 file2 | …`).
        for (i, (_name, contents)) in CORE_FILES.iter().enumerate() {
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

        self.selected_demo = None;
        self.switch_pressed = was_switch_pressed;

        // Auto-run so bootstrap drains the queue.
        self.running = true;
        self.emulator.resume();
        self.schedule_tick(ctx);
    }

    fn schedule_tick(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        self._tick_handle = Some(Timeout::new(TICK_MS, move || {
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
    type Properties = ();

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

                // Pump-loop: feed a UART byte whenever the RX buffer is clear,
                // then run a sub-batch, repeat until the instruction budget is
                // exhausted or the CPU is idle in a KEY poll with an empty
                // queue. This lets bootstrap drain ~3KB of core/*.fth at
                // interpreter speed instead of 1 byte / 25 ms.
                let budget = if self.booted {
                    BATCH_SIZE
                } else {
                    BOOTSTRAP_BATCH
                };
                let mut instructions_used: u64 = 0;
                let mut last_reason = cor24_emulator::StopReason::CycleLimit;
                let mut halted = false;
                loop {
                    self.feed_uart_byte();
                    let remaining = budget.saturating_sub(instructions_used);
                    if remaining == 0 {
                        break;
                    }
                    let chunk = remaining.min(PUMP_SUB_BATCH);
                    let result = self.emulator.run_batch(chunk);
                    // Safety: if the emulator is paused or otherwise not
                    // advancing, bail out — without this we'd spin forever
                    // and hang the UI thread.
                    if result.instructions_run == 0 {
                        last_reason = result.reason.clone();
                        if matches!(last_reason, cor24_emulator::StopReason::Halted) {
                            halted = true;
                        }
                        break;
                    }
                    instructions_used += result.instructions_run;
                    last_reason = result.reason.clone();
                    if matches!(last_reason, cor24_emulator::StopReason::Halted) {
                        halted = true;
                        break;
                    }
                    // If the CPU is idle in a KEY poll and we have nothing
                    // more to feed, stop early to save UI-thread cycles.
                    let pc = self.emulator.snapshot().pc;
                    let idle_polling = self
                        .uart_poll_addrs
                        .iter()
                        .any(|&addr| pc >= addr && pc < addr + 16);
                    if idle_polling && self.uart_rx_queue.is_empty() {
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

                // First idle with empty queue = bootstrap complete.
                if !self.booted && self.waiting_for_input && self.uart_rx_queue.is_empty() {
                    self.booted = true;
                    self.output.clear();
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
                if let Some(demo) = FIF_DEMOS.get(index) {
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
                        { for FIF_DEMOS.iter().enumerate().map(|(i, demo)| {
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
                        <span class="status-value">{"forth-in-forth"}</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Stack:"}</span>
                        <span class="status-value">{ StackSize::ThreeKb.label() }</span>
                    </div>
                    <div class="status-item">
                        <span class="status-label">{"Cycles:"}</span>
                        <span class="status-value">{ format!("{}", snap.cycles) }</span>
                    </div>
                </div>

                // About dialog
                { if self.show_about {
                    html! {
                        <div class="about-overlay" onclick={ctx.link().callback(|_| Msg::ToggleAbout)}>
                            <div class="about-dialog" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <h2>{"forth-in-forth"}</h2>
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
