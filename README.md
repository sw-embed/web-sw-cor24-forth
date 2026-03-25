# web-tf24a

Web UI for [Tiny Forth](https://github.com/sw-vibe-coding/tf24a) on COR24. Browser-based Forth debugger running the tf24a DTC Forth interpreter on the COR24 emulator via Rust, Yew, and WebAssembly.

Type Forth commands, step through threaded code, inspect stacks and registers, and toggle hardware I/O — all in the browser.

**[Live Demo](https://sw-vibe-coding.github.io/web-tf24a/)**

![Stack Ops demo running in the debugger](images/screenshot.png)

## Features

- **Interactive REPL** with UART I/O bridge to the Forth interpreter
- **7 embedded demos**: LED Blink, Arithmetic, Stack Ops, Hex Mode, Comparison, Return Stack, Words — selectable from the dropdown
- **Debugger controls**: Step, Step Over, Run/Pause, Reset, breakpoints (click disassembly lines)
- **Hardware I/O panel**: visual LED D2 (glows red when lit) and clickable Switch S2
- **Inspection panels**: CPU registers (with change highlighting), data stack, return stack, caller chain, disassembly, dictionary browser, word inspector, compile log
- **Memory map**: visual bar showing kernel, free, return stack, and data stack regions
- **Multi-tier assembly**: Bootstrap (Phase 1) and Interpreter (Phase 4: LED!, .S, HEX, WORDS, BYE)
- **Configurable stack**: 3 KB (hardware default) or 8 KB (full EBR window)

## Related

- [tf24a](https://github.com/sw-vibe-coding/tf24a) — The Forth implementation (assembly)
- [cor24-rs](https://github.com/sw-embed/cor24-rs) — COR24 assembler and emulator (Rust)
- [web-tml24c](https://github.com/sw-vibe-coding/web-tml24c) — Web UI for Tiny Macro Lisp
- [web-tc24r](https://github.com/sw-vibe-coding/web-tc24r) — Web UI for tc24r C compiler
- [COR24-TB](https://makerlisp.com) — The COR24 target board

## Documentation

- [tf24a docs](https://github.com/sw-vibe-coding/tf24a/tree/main/docs) — Forth word reference, LED control, design notes
- [COR24 ISA](https://github.com/sw-embed/cor24-rs/blob/main/docs/isa.md) — 24-bit RISC instruction set

## Development

```bash
./scripts/serve.sh              # dev server with hot reload on port 9181
./scripts/build-pages.sh        # release build to pages/ for GitHub Pages
cargo clippy -- -D warnings     # lint
cargo fmt --all                 # format
```

## License

MIT &copy; 2026 Michael A. Wright
