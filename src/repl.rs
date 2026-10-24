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
use crate::pump::{Pump, TICK_MS_BOOT, TICK_MS_INTERACTIVE};
use crate::snapshot;
use cor24_assembler::Assembler;
use cor24_emulator::EmulatorCore;
use gloo::file::File;
use gloo::file::callbacks::FileReader;
use gloo::timers::callback::Timeout;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;

/// Snapshot cache toggle. When false, both the build-time embedded blob
/// and the localStorage cache are bypassed — every visit takes the slow
/// UART bootstrap through `QUIT`. Kept off while we benchmark kernel-side
/// perf work (e.g. XMX FIND hash in `sw-cor24-forth`) so the wall-clock
/// we observe is purely the kernel's, not our cache's.
const SNAPSHOT_CACHE_ENABLED: bool = false;

// Batch sizes, tick intervals, and the pump loop itself live in
// `crate::pump` so the headless tests drive the same code the browser runs.

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
    /// Show "Rebuild kernel" (tab 4): the running kernel compiles its own
    /// kernel.s from its Forth sources; see `crate::rebuild`.
    #[prop_or(false)]
    pub rebuild: bool,
}

/// A kernel rebuild in progress (tab 4).
struct RebuildRun {
    /// UART output since the rebuild started (not shown; it is ~4,000
    /// lines of assembly).
    captured: String,
    total_bytes: usize,
    start_ms: f64,
    start_instructions: u64,
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// 1234567 -> "1,234,567"
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
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
    Rebuild,
}

pub struct ForthRepl {
    emulator: EmulatorCore,
    output: String,
    input: String,
    running: bool,
    halted: bool,
    /// UART input queue, poll-loop addresses, booted / waiting state.
    pump: Pump,
    _tick_handle: Option<Timeout>,
    output_ref: NodeRef,
    input_ref: NodeRef,
    file_input_ref: NodeRef,
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
    rebuild: Option<RebuildRun>,
}

impl ForthRepl {
    /// Finish a rebuild once the end marker has arrived: compare the
    /// rebuilt kernel.s with the running kernel and print the verdict.
    fn finish_rebuild(&mut self, kernel_src: &str) {
        let Some(run) = self.rebuild.as_ref() else {
            return;
        };
        let Some(text) = crate::rebuild::extract(&run.captured) else {
            return;
        };
        let run = self.rebuild.take().unwrap();
        self.pump.turbo = false;
        let secs = (now_ms() - run.start_ms) / 1000.0;
        let instr = self.emulator.instructions_count() - run.start_instructions;
        let v = crate::rebuild::verdict(&text, kernel_src);
        self.output.push_str(&format!(
            "[Rebuild done: {:.1} s, {} instructions]\n",
            secs,
            grouped(instr)
        ));
        match &v.bytes {
            Ok(n) => self.output.push_str(&format!(
                "  kernel.s rebuilt by this kernel: {} lines, assembled to {} bytes\n",
                grouped(v.lines as u64),
                grouped(*n as u64)
            )),
            Err(e) => self.output.push_str(&format!(
                "  kernel.s rebuilt: {} lines, but it does not assemble: {e}\n",
                grouped(v.lines as u64)
            )),
        }
        if v.same_text && v.same_bytes {
            self.output.push_str(
                "  identical to the kernel you are running: same text, same bytes. \
                 A fixed point: this kernel rebuilt itself.\n",
            );
        } else {
            self.output.push_str(&format!(
                "  differs from the running kernel (text {}, bytes {})\n",
                if v.same_text { "same" } else { "different" },
                if v.same_bytes { "same" } else { "different" }
            ));
            if let Some((n, got, want)) = &v.first_difference {
                self.output.push_str(&format!(
                    "  first difference, line {n}:\n    rebuilt: {got}\n    running: {want}\n"
                ));
            }
        }
        let leftover = run.captured[run.captured.find("!!END-KERNEL!!").unwrap_or(0)..]
            .split_once('\n')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_default();
        self.output.push_str(&leftover);
    }

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

        self.pump.reset(&result.labels);

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
                self.pump.booted = true;
                self.pump.waiting_for_input = true;
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
                self.pump.booted = true;
                self.pump.waiting_for_input = true;
                self.running = true;
                self.schedule_tick(ctx);
                return;
            }
        }

        // Slow path: preload core/*.fth into the RX queue and let the
        // kernel's QUIT loop compile each line. On completion we'll capture
        // a snapshot so the next load takes the fast path above.
        self.pump.queue_core_files(props.core_files);

        self.running = true;
        self.emulator.resume();
        self.schedule_tick(ctx);
    }

    fn schedule_tick(&mut self, ctx: &Context<Self>) {
        let link = ctx.link().clone();
        let ms = if self.pump.booted && !self.pump.turbo {
            TICK_MS_INTERACTIVE
        } else {
            TICK_MS_BOOT
        };
        self._tick_handle = Some(Timeout::new(ms, move || {
            link.send_message(Msg::Tick);
        }));
    }

    fn collect_uart_output(&mut self) -> bool {
        let uart = self.emulator.get_uart_output();
        if let Some(run) = self.rebuild.as_mut() {
            if !uart.is_empty() {
                run.captured.push_str(uart);
                self.emulator.clear_uart_output();
            }
            return true; // progress in the status line
        }
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
            pump: Pump::new(),
            _tick_handle: None,
            output_ref: NodeRef::default(),
            input_ref: NodeRef::default(),
            file_input_ref: NodeRef::default(),
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
            rebuild: None,
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

                // Adaptive pump-loop; see `Pump::tick_adaptive`.
                let was_waiting = self.pump.waiting_for_input;
                let outcome = self.pump.tick_adaptive(&mut self.emulator);

                // Show boot output too — users want to see the "ok" stream as
                // proof of life. The `self.output.clear()` below wipes
                // it right before the first interactive prompt.
                let had_output = self.collect_uart_output();
                if self.rebuild.is_some() {
                    self.finish_rebuild(ctx.props().kernel_src);
                }

                // First idle with empty queue = bootstrap complete. When
                // the snapshot cache is enabled, also save so subsequent
                // loads take the fast path.
                if outcome.just_booted {
                    self.output.clear();
                    if SNAPSHOT_CACHE_ENABLED {
                        let props = ctx.props();
                        let key = snapshot::content_key(props.kernel_src, props.core_files);
                        let snap = snapshot::capture(&self.emulator);
                        let _ = snapshot::save(&key, &snap);
                    }
                }
                let halted = outcome.halted;

                if halted {
                    self.running = false;
                    self.halted = true;
                } else if self.running {
                    self.schedule_tick(ctx);
                }

                if self.pump.waiting_for_input && was_waiting && !had_output {
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
                if self.rebuild.is_some() {
                    self.output
                        .push_str("[a kernel rebuild is running; wait for it to finish]\n");
                    return true;
                }
                if self.running && !self.pump.waiting_for_input {
                    self.output.push_str("[input queued — interpreter busy]\n");
                }
                self.history.push(self.input.clone());
                self.full_log.push(self.input.clone());
                self.history_pos = -1;
                self.history_saved.clear();

                self.pump.queue_line(&self.input);
                self.input.clear();
                self.pump.waiting_for_input = false;

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
                self.rebuild = None;
                self.load_binary(ctx);
                true
            }

            Msg::Rebuild => {
                if !self.pump.booted || self.rebuild.is_some() || self.halted {
                    return false;
                }
                let input = crate::rebuild::input();
                self.output.push_str(&format!(
                    "\n[Rebuild: this kernel now compiles its own kernel.s from its Forth \
                     sources: the cross-compiler (compiler/xcomp.fth), the primitives \
                     (core/prims-asm.fth) and the five core tiers, {} bytes in all. \
                     The ~4,000 lines of assembly it prints are captured, not shown; \
                     the result is compared with the kernel you are running. About a \
                     billion and a quarter instructions: expect a minute or so.]\n",
                    grouped(input.len() as u64)
                ));
                self.rebuild = Some(RebuildRun {
                    captured: String::new(),
                    total_bytes: input.len(),
                    start_ms: now_ms(),
                    start_instructions: self.emulator.instructions_count(),
                });
                self.pump.queue.extend(input.bytes());
                self.pump.turbo = true;
                self.pump.waiting_for_input = false;
                if !self.running {
                    self.running = true;
                    self.emulator.resume();
                }
                self.schedule_tick(ctx);
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
                        self.pump.queue_line(trimmed);
                    }
                    self.pump.waiting_for_input = false;
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
        } else if let Some(run) = &self.rebuild {
            let fed = run.total_bytes - self.pump.queue.len();
            status_text = format!(
                "Rebuilding kernel.s: {}% of the source fed, {} M instructions, {:.0} s",
                fed * 100 / run.total_bytes.max(1),
                (self.emulator.instructions_count() - run.start_instructions) / 1_000_000,
                (now_ms() - run.start_ms) / 1000.0
            );
            &status_text
        } else if !self.pump.booted {
            status_text = format!(
                "Booting core/*.fth — {} bytes left, {} cycles",
                self.pump.queue.len(),
                snap.cycles,
            );
            &status_text
        } else if self.pump.waiting_for_input {
            "Ready"
        } else if self.running {
            "Running"
        } else {
            "Stopped"
        };

        // Lock out UI during bootstrap — the dropdown would close on every
        // re-render, buttons would fight the pump loop, and a user typing
        // would land bytes ahead of the core/*.fth stream.
        let ui_locked = !self.pump.booted || self.rebuild.is_some();

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
                    { if ctx.props().rebuild {
                        html! {
                            <button class="upload-btn"
                                    title="This kernel compiles its own kernel.s from its Forth sources and compares the result with itself"
                                    disabled={ui_locked || self.halted}
                                    onclick={ctx.link().callback(|_| Msg::Rebuild)}>
                                {"Rebuild kernel"}
                            </button>
                        }
                    } else {
                        html! {}
                    }}
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
                        <div class={if self.pump.waiting_for_input { "input-bar input-ready" }
                                    else if self.running { "input-bar input-busy" }
                                    else { "input-bar" }}>
                            <span class="prompt">
                                { if self.running && !self.pump.waiting_for_input { "\u{25F3} " } else { "> " } }
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
                                placeholder={if !self.pump.booted {
                                    "Booting core/*.fth — UI locked until ready..."
                                } else if self.running && !self.pump.waiting_for_input {
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
