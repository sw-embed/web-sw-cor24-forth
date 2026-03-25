# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## CRITICAL: AgentRail Session Protocol (MUST follow exactly)

This project uses AgentRail. Every session follows this exact sequence:

### 1. START (do this FIRST, before anything else)
```bash
agentrail next
```
Read the output carefully. It tells you your current step, prompt, skill docs, and past trajectories.

### 2. BEGIN (immediately after reading the next output)
```bash
agentrail begin
```

### 3. WORK (do what the step prompt says)
Do NOT ask the user "want me to proceed?" or "shall I start?". The step prompt IS your instruction. Execute it.

### 4. COMMIT (after the work is done)
Commit your code changes with git.

### 5. COMPLETE (LAST thing, after committing)
```bash
agentrail complete --summary "what you accomplished" \
  --reward 1 \
  --actions "tools and approach used"
```
If the step failed: `--reward -1 --failure-mode "what went wrong"`
If the saga is finished: add `--done`

### 6. STOP (after complete, DO NOT continue working)
Do NOT make any further code changes after running agentrail complete.
Any changes after complete are untracked and invisible to the next session.
If you see more work to do, it belongs in the NEXT step, not this session.

Do NOT skip any of these steps. The next session depends on your trajectory recording.

## Project: web-tf24a -- Forth Debugger on COR24

Browser-based Forth debugger running the tf24a DTC Forth interpreter on the cor24-rs emulator via WASM.

### Build and Serve

```bash
trunk build                    # Build WASM to dist/
./scripts/serve.sh             # Dev server on port 9181
./scripts/build-pages.sh       # Release build to pages/ for GitHub Pages
cargo clippy --all-targets --all-features -- -D warnings  # Lint
cargo fmt --all                # Format
```

### Utilities

- `ep2ms` — returns milliseconds since epoch; use for `?ts=` cache-busting on image URLs in README (e.g. `screenshot.png?ts=$(ep2ms)`)

### Architecture

- **Trunk** builds the WASM binary and serves it
- **cor24-emulator** provides `EmulatorCore` + `Assembler` (path dep to `../../sw-embed/cor24-rs`)
- **Yew 0.21** CSR framework for the UI
- Assembly files in `asm/` are `include_str!`'d and assembled at runtime
- UART I/O bridges user input to the Forth interpreter running in the emulator

### Key Files

- `src/debugger.rs` -- Main debugger component (emulator loop, UI panels)
- `src/config.rs` -- ForthTier enum (multi-tier assembly) + StackSize
- `src/demos.rs` -- Demo registry (embedded .fth files)
- `demos/*.fth` -- Forth demo source files
- `asm/forth-bootstrap.s` -- Phase 1 tf24a Forth kernel (copied from tf24a)
- `asm/forth-interpreter.s` -- Phase 4 full interpreter (copied from tf24a)
- `index.html` -- Entry point with high-contrast dark theme
- `src/debugger.css` -- Debugger panel styling
- `build.rs` -- Build script (BUILD_SHA, BUILD_HOST, BUILD_TIMESTAMP)
- `scripts/build-pages.sh` -- Release build to pages/ for GitHub Pages
- `.github/workflows/pages.yml` -- Deploy pages/ on push to main

### COR24 Register Allocation (from tf24a)

- r0 = W (work/scratch)
- r1 = RSP (return stack pointer, SRAM ~0x0F0000 growing down)
- r2 = IP (instruction pointer for threaded code)
- sp = DSP (data stack, hardware push/pop in EBR)
- fp = available as extra scratch
- Cell size = 3 bytes (24-bit words)
