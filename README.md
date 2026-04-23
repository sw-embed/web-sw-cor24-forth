# web-sw-cor24-forth

Web UI for [sw-cor24-forth](https://github.com/sw-embed/sw-cor24-forth) on COR24. Browser-based Forth debugger running the DTC Forth interpreter on the COR24 emulator via Rust, Yew, and WebAssembly.

Type Forth commands, step through threaded code, inspect stacks and registers, and toggle hardware I/O — all in the browser.

**[Live Demo](https://sw-embed.github.io/web-sw-cor24-forth/)**

Part of the [Software Wrighter COR24 Tools Project](https://sw-embed.github.io/web-sw-cor24-demos/#/).

![forth-on-forthish tab with WORDS output](images/screenshot.png?ts=1776918570363)

## Features

Three top-level tabs, each with its own demo list and a `?` help button
describing what it provides. Default is `forth-on-forthish` (phase 3, the
"current best" self-hosting tier). A **Help** button in every tab's
toolbar opens a global dialog with **User Guide**, **Reference**, and
**Tutorial** sections — all sourced from `docs/*.md`.

### Tab 1: `forth.s` — full debugger

- **Interactive REPL** with UART I/O bridge to the Forth interpreter
- **Debugger controls**: Step, Step Over, Run/Pause, Reset, breakpoints (click disassembly lines)
- **Hardware I/O panel**: visual LED D2 (glows red when lit) and clickable Switch S2
- **Inspection panels**: CPU registers (with change highlighting), data stack, return stack, caller chain, disassembly, dictionary browser, word inspector, compile log
- **Memory map**: visual bar showing kernel, free, return stack, and data stack regions
- **Multi-tier assembly**: Bootstrap (Phase 1) and Interpreter (Phase 4: D2_ON!/D2_OFF!, .S, HEX, WORDS, BYE)
- **Configurable stack**: 3 KB (hardware default) or 8 KB (full EBR window)
- **Demo set**: smoke, colon, LED, math, stars, switch→LED, comments, /MOD, if/then, if/else, loop, fizzbuzz, self-test, switch LED loop, messy-fibonacci

### Tab 2: `forth-in-forth` — self-hosting REPL

- **Minimal asm kernel** from `../sw-cor24-forth/forth-in-forth/kernel.s`
  with the rest of Forth (IF/THEN/ELSE/BEGIN/UNTIL, `\`/`(`, `.`, CR, SPACE,
  HEX, DECIMAL, DEPTH, .S, WORDS, SEE, DUMP-ALL, NIP, TUCK, ROT, 2DUP, …)
  defined in Forth and bootstrapped from `core/{minimal,lowlevel,midlevel,highlevel}.fth`
  at boot
- **Simple REPL** — Run / Stop / Reset, demo dropdown, `.fth` upload, S2+D2,
  command history. Deliberately no step/breakpoints/registers/stacks/disasm
- **Demo set** adds a `SEE (all words)` dictionary dump demo and two
  side-by-side Fibonacci drivers (manual calls vs. BEGIN/UNTIL loop)
- **UI locked during boot** with a progress indicator until the kernel
  idles at the KEY poll

### Tab 3: `forth-on-forthish` — phase 3 self-hosting (default)

- **Same REPL shell as tab 2**, but running a minimized kernel from
  `../sw-cor24-forth/forth-on-forthish/` where `:` `;` `WORD` `FIND`
  `NUMBER` `INTERPRET` `QUIT` plus all stack ops and `*` `/MOD` `AND`
  `OR` `XOR` have been moved out of asm and into Forth
- **Core load order**: `runtime → minimal → lowlevel → midlevel →
  highlevel`. `runtime.fth` supplies the Forth `:` `;` + stack ops
  that later tiers compile against; `highlevel.fth` ends by installing
  Forth `QUIT` via a `QUIT-VECTOR` handoff
- **Forth-driven REPL**: after boot, every prompt line runs through
  Forth `INTERPRET`/`QUIT`, not asm. The asm bootstrap never resumes
- **Chattier boot log** (~22 extra `ok` lines during `highlevel.fth`
  load) is expected — Forth `INTERPRET` echoes per line
- **Per-command Δcycles / Δinstrs** reported after each prompt for
  easy tab-2-vs-tab-3 performance comparison

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

## Links

- Blog: [Software Wrighter Lab](https://software-wrighter-lab.github.io/)
- Discord: [Join the community](https://discord.com/invite/Ctzk5uHggZ)
- YouTube: [Software Wrighter](https://www.youtube.com/@SoftwareWrighter)

## Copyright

Copyright (c) 2026 Michael A. Wright

## License

MIT License. See [LICENSE](LICENSE) for the full text.
