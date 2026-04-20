# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project: web-sw-cor24-forth -- Forth Debugger on COR24

Browser-based Forth debugger running the sw-cor24-forth DTC Forth interpreter on the COR24 emulator via WASM.

### Build and Serve

```bash
trunk build                    # Build WASM to dist/
./scripts/serve.sh             # Dev server on port 9181
./scripts/build-pages.sh       # Release build to pages/ for GitHub Pages
cargo clippy --all-targets --all-features -- -D warnings  # Lint
cargo fmt --all                # Format
```

### Architecture

- **Trunk** builds the WASM binary and serves it
- **cor24-emulator** provides `EmulatorCore` + `Assembler` (path dep to `../sw-cor24-emulator`)
- **Yew 0.21** CSR framework for the UI
- Assembly files in `asm/` are `include_str!`'d and assembled at runtime
- UART I/O bridges user input to the Forth interpreter running in the emulator

### Key Files

- `src/lib.rs` -- App entry point with top-level tab switcher (forth.s / forth-in-forth) and per-tab `?` help dialogs
- `src/debugger.rs` -- Tab 1: full debugger (emulator loop, inspection panels)
- `src/repl.rs` -- Tab 2: forth-in-forth simple REPL (pump-loop UART feed, core/*.fth preload at boot)
- `src/config.rs` -- ForthTier enum (multi-tier assembly) + StackSize
- `src/demos.rs` -- Per-tab demo lists: `FORTH_S_DEMOS` (tab 1), `FIF_DEMOS` (tab 2). Some inline sources (simplified fib, DUMP-ALL); most via `include_str!` from `../sw-cor24-forth/examples/`
- `asm/forth-bootstrap.s` -- Phase 1 Forth kernel (copied from sw-cor24-forth; used by tab 1's Bootstrap tier)
- Tab 1 Interpreter tier reads `../sw-cor24-forth/forth.s` at compile time
- Tab 2 reads `../sw-cor24-forth/forth-in-forth/kernel.s` + `core/*.fth` at compile time
- `index.html` -- Entry point with high-contrast dark theme
- `src/debugger.css` -- Shared styling (top-tab bar, help bubble, REPL layout, debugger panels)
- `build.rs` -- Build script (BUILD_SHA, BUILD_HOST, BUILD_TIMESTAMP)
- `scripts/build-pages.sh` -- Release build to pages/ for GitHub Pages
- `.github/workflows/pages.yml` -- Deploy pages/ on push to main

### COR24 Register Allocation (from sw-cor24-forth)

- r0 = W (work/scratch)
- r1 = RSP (return stack pointer, SRAM ~0x0F0000 growing down)
- r2 = IP (instruction pointer for threaded code)
- sp = DSP (data stack, hardware push/pop in EBR)
- fp = available as extra scratch
- Cell size = 3 bytes (24-bit words)
