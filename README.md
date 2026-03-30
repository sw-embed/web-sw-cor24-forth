# web-sw-cor24-forth

Web UI for [sw-cor24-forth](https://github.com/sw-embed/sw-cor24-forth) on COR24. Browser-based Forth debugger running the DTC Forth interpreter on the COR24 emulator via Rust, Yew, and WebAssembly.

Type Forth commands, step through threaded code, inspect stacks and registers, and toggle hardware I/O — all in the browser.

**[Live Demo](https://sw-embed.github.io/web-sw-cor24-forth/)**

![Stack Ops demo running in the debugger](images/screenshot.png?ts=1774477179000)

## Features

- **Interactive REPL** with UART I/O bridge to the Forth interpreter
- **7 embedded demos**: LED Blink, Arithmetic, Stack Ops, Hex Mode, Comparison, Return Stack, Words — selectable from the dropdown
- **Debugger controls**: Step, Step Over, Run/Pause, Reset, breakpoints (click disassembly lines)
- **Hardware I/O panel**: visual LED D2 (glows red when lit) and clickable Switch S2
- **Inspection panels**: CPU registers (with change highlighting), data stack, return stack, caller chain, disassembly, dictionary browser, word inspector, compile log
- **Memory map**: visual bar showing kernel, free, return stack, and data stack regions
- **Multi-tier assembly**: Bootstrap (Phase 1) and Interpreter (Phase 4: D2_ON!/D2_OFF!, .S, HEX, WORDS, BYE)
- **Configurable stack**: 3 KB (hardware default) or 8 KB (full EBR window)

## Provenance

Forked from [sw-vibe-coding/web-tf24a](https://github.com/sw-vibe-coding/web-tf24a) as part of the COR24 ecosystem consolidation under [sw-embed](https://github.com/sw-embed).

## Related

- [sw-cor24-forth](https://github.com/sw-embed/sw-cor24-forth) — The Forth implementation (assembly)
- [sw-cor24-emulator](https://github.com/sw-embed/sw-cor24-emulator) — COR24 emulator (Rust)
- [sw-cor24-project](https://github.com/sw-embed/sw-cor24-project) — COR24 ecosystem hub
- [COR24-TB](https://makerlisp.com) — The COR24 target board

## Documentation

- [sw-cor24-forth docs](https://github.com/sw-embed/sw-cor24-forth/tree/main/docs) — Forth word reference, LED control, design notes
- [COR24 ISA](https://github.com/sw-embed/sw-cor24-emulator/blob/main/docs/isa.md) — 24-bit RISC instruction set

## Development

```bash
./scripts/serve.sh              # dev server with hot reload on port 9181
./scripts/build-pages.sh        # release build to pages/ for GitHub Pages
cargo clippy -- -D warnings     # lint
cargo fmt --all                 # format
```

## License

MIT &copy; 2026 Michael A. Wright
