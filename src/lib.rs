pub mod config;
pub mod debugger;
pub mod demos;
pub mod repl;

use debugger::Debugger;
use repl::ForthRepl;
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    ForthS,
    ForthInForth,
}

#[function_component(App)]
pub fn app() -> Html {
    let tab = use_state(|| Tab::ForthS);
    let help_open = use_state(|| None::<Tab>);

    let on_forth_s = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::ForthS))
    };
    let on_fif = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::ForthInForth))
    };
    let open_forth_s_help = {
        let help_open = help_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            help_open.set(Some(Tab::ForthS));
        })
    };
    let open_fif_help = {
        let help_open = help_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            help_open.set(Some(Tab::ForthInForth));
        })
    };
    let close_help = {
        let help_open = help_open.clone();
        Callback::from(move |_| help_open.set(None))
    };
    let stop_click = Callback::from(|e: MouseEvent| e.stop_propagation());

    let active = *tab;
    let open = *help_open;

    html! {
        <>
            // GitHub corner
            <a href="https://github.com/sw-embed/web-sw-cor24-forth" class="github-corner"
               aria-label="View source on GitHub" target="_blank">
                <svg width="80" height="80" viewBox="0 0 250 250" aria-hidden="true">
                    <path d="M0,0 L115,115 L130,115 L142,142 L250,250 L250,0 Z" />
                    <path d="M128.3,109.0 C113.8,99.7 119.0,89.6 119.0,89.6 C122.0,82.7 120.5,78.6 \
                        120.5,78.6 C119.2,72.0 123.4,76.3 123.4,76.3 C127.3,80.9 125.5,87.3 125.5,87.3 \
                        C122.9,97.6 130.6,101.9 134.4,103.2" fill="currentColor"
                        style="transform-origin:130px 106px;" class="octo-arm" />
                    <path d="M115.0,115.0 C114.9,115.1 118.7,116.5 119.8,115.4 L133.7,101.6 C136.9,99.2 \
                        139.9,98.4 142.2,98.6 C133.8,88.0 127.5,74.4 143.8,58.0 C148.5,53.4 154.0,51.2 \
                        159.7,51.0 C160.3,49.4 163.2,43.6 171.4,40.1 C171.4,40.1 176.1,42.5 178.8,56.2 \
                        C183.1,58.6 187.2,61.8 190.9,65.4 C194.5,69.0 197.7,73.2 200.1,77.6 C213.8,80.2 \
                        216.3,84.9 216.3,84.9 C212.7,93.1 206.9,96.0 205.4,96.6 C205.1,102.4 203.0,107.8 \
                        198.3,112.5 C181.9,128.9 168.3,122.5 157.7,114.1 C157.9,116.9 156.7,120.9 \
                        152.7,124.9 L141.0,136.5 C139.8,137.7 141.6,141.9 141.8,141.8 Z"
                        fill="currentColor" />
                </svg>
            </a>
            // Header
            <header>
                <h1>{"Tiny Forth"}</h1>
                <span>{"COR24 Debugger"}</span>
            </header>
            // Top-level tab bar
            <div class="top-tabs">
                <button class={classes!("top-tab", (active == Tab::ForthS).then_some("active"))}
                        onclick={on_forth_s}>
                    {"forth.s"}
                    <span class="tab-help" title="What's this tab?"
                          onclick={open_forth_s_help}>{"?"}</span>
                </button>
                <button class={classes!("top-tab", (active == Tab::ForthInForth).then_some("active"))}
                        onclick={on_fif}>
                    {"forth-in-forth"}
                    <span class="tab-help" title="What's this tab?"
                          onclick={open_fif_help}>{"?"}</span>
                </button>
            </div>
            // Active tab content
            { match active {
                Tab::ForthS => html! { <Debugger /> },
                Tab::ForthInForth => html! { <ForthRepl /> },
            }}
            // Help dialog (click-outside to close)
            { match open {
                Some(Tab::ForthS) => html! {
                    <div class="about-overlay" onclick={close_help.clone()}>
                        <div class="about-dialog" onclick={stop_click.clone()}>
                            <h2>{"forth.s"}</h2>
                            <p>{"Full hand-written asm Forth kernel (~3000 lines). Loaded as a single binary. Rich debugger UI: VM registers, data/return stacks, disassembly, dictionary inspector, step/step-over, breakpoints, compile log."}</p>
                            <h3>{"Core words (asm primitives)"}</h3>
                            <p>{"+ − ∗ /MOD AND OR XOR = < 0= DUP DROP SWAP OVER >R R> R@ @ ! C@ C! EXECUTE IF THEN ELSE BEGIN UNTIL : ; IMMEDIATE [ ] CREATE , C, ALLOT HERE LATEST STATE BASE . CR SPACE HEX DECIMAL WORDS .S DEPTH VER EMIT KEY LED! SW? \\ ( FIND WORD NUMBER INTERPRET QUIT"}</p>
                            <h3>{"Missing (compared to forth-in-forth)"}</h3>
                            <p>{"NIP TUCK ROT 2DUP 2DROP 2SWAP 2OVER 1+ 1− NEGATE ABS / MOD 0< ′ (tick) SEE DUMP-ALL PRINT-NAME >NAME SP@ [′] EOL!"}</p>
                            <p class="about-hint">{"Demos that need these words (e.g. Fibonacci) define them inline."}</p>
                            <button onclick={close_help.clone()}>{"Close"}</button>
                        </div>
                    </div>
                },
                Some(Tab::ForthInForth) => html! {
                    <div class="about-overlay" onclick={close_help.clone()}>
                        <div class="about-dialog" onclick={stop_click.clone()}>
                            <h2>{"forth-in-forth"}</h2>
                            <p>{"Minimal asm kernel (~2200 lines) with the rest of Forth written in Forth and loaded at boot from core/minimal.fth + lowlevel.fth + midlevel.fth + highlevel.fth. Self-hosting demonstration. Simple REPL — no debugger."}</p>
                            <h3>{"Added in Forth (over forth.s)"}</h3>
                            <p>{"NIP TUCK ROT −ROT 2DUP 2DROP 2SWAP 2OVER 1+ 1− NEGATE ABS / MOD 0< ′ SEE DUMP-ALL PRINT-NAME >NAME. Plus three new asm primitives [′] EOL! SP@ needed to bootstrap the above."}</p>
                            <h3>{"Moved asm → Forth"}</h3>
                            <p>{"IF THEN ELSE BEGIN UNTIL \\ ( = 0= CR SPACE HEX DECIMAL . DEPTH .S WORDS VER. Try SEE IF to see the Forth definition."}</p>
                            <h3>{"Still missing"}</h3>
                            <p>{"DO LOOP ?DO I J LEAVE UNLOOP WHILE REPEAT AGAIN CONSTANT VARIABLE DOES> RECURSE MIN MAX. Tracked: "}
                                <a href="https://github.com/sw-embed/sw-cor24-forth/issues/2" target="_blank">{"issue #2"}</a>
                                {"."}</p>
                            <h3>{"Trade-offs"}</h3>
                            <p>{"Slower boot (kernel text-bootstraps the core tier); slower compile-time (IMMEDIATE words run as threaded Forth, not inline asm). Binary-compatible with all examples that forth.s runs."}</p>
                            <button onclick={close_help.clone()}>{"Close"}</button>
                        </div>
                    </div>
                },
                None => html! {},
            }}
            // Footer
            <footer>
                <span>{"MIT License"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{"\u{00a9} 2026 Michael A Wright"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://makerlisp.com" target="_blank">{"COR24-TB"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://software-wrighter-lab.github.io/" target="_blank">{"Blog"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://discord.com/invite/Ctzk5uHggZ" target="_blank">{"Discord"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://www.youtube.com/@SoftwareWrighter" target="_blank">{"YouTube"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{ format!("{} \u{00b7} {} \u{00b7} {}",
                    env!("BUILD_HOST"),
                    env!("BUILD_SHA"),
                    env!("BUILD_TIMESTAMP"),
                ) }</span>
            </footer>
        </>
    }
}
