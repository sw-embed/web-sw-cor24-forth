# Changelog

## 2026-04-20 — forth-in-forth Tab

- Added a second top-level tab, **forth-in-forth**, exposing the self-hosting
  kernel from `../sw-cor24-forth/forth-in-forth/`. First tab is the existing
  full-featured `forth.s` debugger (unchanged).
- New simplified REPL component (`src/repl.rs`): Run / Stop / Reset buttons,
  demo dropdown, `.fth` upload, S2 switch + D2 LED, command history, About.
  Deliberately omits debugger-only features (step, step-over, breakpoints,
  register/stack/disassembly panels, memory map, compile log).
- Boot sequence: assembles `forth-in-forth/kernel.s`, then feeds
  `core/{minimal,lowlevel,midlevel,highlevel}.fth` into the UART RX queue as
  raw bytes (`include_str!`, CR-stripped, LF between tiers) so the user
  lands at a prompt with `SEE`, `DUMP-ALL`, `.S`, `DEPTH` etc. already defined.
- UART feed rewritten as a **pump loop**: during bootstrap each tick runs
  up to 500k instructions in 20k-instruction sub-batches, feeding a new
  UART byte between each. Replaces the original 1-byte-per-25ms-tick cap
  that would otherwise make bootstrap take ~75s just on feed latency.
- UI lock during boot: all toolbar controls, demo dropdown, file upload,
  S2 switch, and input field disabled until the kernel idles at the KEY
  poll with an empty RX queue. Status bar shows `bytes left` + `cycles`
  as proof of life.
- **Per-tab demo lists**: `FORTH_S_DEMOS` keeps the original `examples/*.fth`
  set (incl. the messy hand-unrolled `14-fib.fth`); `FIF_DEMOS` adds
  `SEE (all words)` (defines SQUARE/CUBE then calls DUMP-ALL), two
  Fibonacci variants (manual calls vs. looped — same FIB, different driver),
  and drops the messy fib. Both share the rest of the example files.
- **Per-tab `?` help button** next to each tab name: concise dialog
  describing approach, words present/missing/added, and trade-offs. Links
  out to `sw-cor24-forth` issues for follow-ups.
- **Text selection fix**: auto-focus on the input field now happens only
  on first render. Previously every re-render (demo load, tick update)
  stole focus mid-drag, cancelling selections in the output panel.
- **File picker**: narrowed `accept` to `.fth,.fs` (dropped `.4th`,`.f`).
- Filed follow-ups against sw-cor24-forth:
  [#1 hashed dictionary](https://github.com/sw-embed/sw-cor24-forth/issues/1)
  and
  [#2 DO/LOOP, ?DO, CONSTANT/VARIABLE, RECURSE, etc.](https://github.com/sw-embed/sw-cor24-forth/issues/2).
  Both are ergonomic/perf work, not correctness blockers.

## 2026-04-05 — /MOD Primitive & Demo

- Added `/MOD ( n1 n2 -- rem quot )` assembly primitive to `asm/forth-interpreter.s`
  - Unsigned division via repeated subtraction (COR24 has no hardware divide)
  - Inserted in dictionary chain between `-` and `AND`
- Added "Division & Modulo" demo (`07-divmod.fth`) covering `/MOD`, `/`, `MOD`, and fizzbuzz-style divisibility checks
- Applied `cargo fmt` to `src/debugger.rs` (pre-existing formatting drift)
- Rebuilt pages/ for deployment

## 2026-03-30 — Fork Migration

- Forked from [sw-vibe-coding/web-tf24a](https://github.com/sw-vibe-coding/web-tf24a)
- Renamed package to `web-sw-cor24-forth`
- Updated path deps to `../sw-cor24-emulator` (was `../../sw-embed/cor24-rs`)
- Updated demo include paths to `../sw-cor24-forth/examples/` (was `../tf24a/examples/`)
- Updated GitHub links to `sw-embed/web-sw-cor24-forth`
- Updated `build-pages.sh` public URL to `/web-sw-cor24-forth/`
- Removed `.agentrail/` and `.claude/` directories
- Updated README with ecosystem links and provenance
