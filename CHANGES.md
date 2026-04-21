# Changelog

## 2026-04-21 — forth-in-forth Bootstrap Speedup

Boot of the forth-in-forth tab feels snappy now (was ~10+ seconds, previously
slow enough that the UI visibly flashed through the `ok`-per-line stream).
Speedup came from two independent fixes landing at once:

**Web-side, `src/repl.rs` pump tuning:**

- Pump sub-batch made **adaptive**. Previously a fixed 20k instructions
  between UART-byte feeds, meaning the CPU spent ~19.5k cycles spinning in
  `key_poll` waiting for the next byte on cheap-byte chars (middle-of-word
  input). New loop inspects PC each iteration and uses a `PUMP_TINY` = 2k
  batch when CPU is in a `key_poll` (just enough to consume a byte and
  decide what's next) and `PUMP_BIG` = 50k when CPU is doing real compile
  work. Kills the single biggest source of wasted time during bootstrap.
- `BOOTSTRAP_BATCH`: 500k → 600k instructions per tick (modest bump).
- `TICK_MS` split into `TICK_MS_BOOT` = 5 (was 25) during bootstrap and
  `TICK_MS_INTERACTIVE` = 25 once ready. Cuts browser scheduler overhead
  during boot without burning CPU while idle at the prompt.

**Kernel-side, pulled from sw-cor24-forth** (authored by the CLI-side agent):

- `forth-in-forth/kernel.s` grew a **hashed FIND** via 2-round 24-bit XMX
  over the word name, with a 256-slot bucket table populated at `_start`
  and kept current by `do_create` (see `sw-cor24-forth@fdae7dd`).
- Plus a **1-entry FIND lookaside cache** in front of the hash table — the
  last successful `do_find` result is checked first before touching the
  bucket (see `sw-cor24-forth@4ea2f79`). Most consecutive compile-time
  lookups target the same word (e.g. repeated `,` during IMMEDIATE words
  like `IF`/`THEN`), so the cache-hit rate is high.

**Other fixes:**

- Uppercase-normalized `examples/06-comments.fth` (upstream fix — the demo
  previously used lowercase names which `do_find` doesn't case-fold, so
  every lookup missed and STATE got wedged in compile mode, preventing
  subsequent demos from running).

**Build-time snapshot infrastructure** (present but **disabled** at runtime
pending CLI-side precompute work):

- `build.rs` runs the full bootstrap natively after assembling the kernel,
  captures 64 KB of low memory + registers, writes a binary blob to
  `$OUT_DIR/fif_snapshot.bin`, `include_bytes!`d into the wasm.
- `src/snapshot.rs`: parses the embedded blob, verifies a content-hash of
  kernel + core files matches, restores on first visit. Also serializes a
  cache to `localStorage` for same-browser subsequent visits.
- `SNAPSHOT_CACHE_ENABLED: bool` gate at the top of `src/repl.rs` disables
  both fast paths so benchmarking the kernel is uncontaminated by our cache.
  Flip to `true` once the kernel's precompute plan lands.

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
