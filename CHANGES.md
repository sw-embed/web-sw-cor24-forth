# Changelog

## 2026-04-22 — Refresh README Screenshot

Replaced `images/screenshot.png` with a fresh capture of the current
UI — tab 3 (forth-on-forthish, the default) active, full `WORDS`
output visible, Help button sitting next to About, footer showing
current commit SHA + timestamp. Previous image was a March capture
from before tabs 2/3 + the Help dialog existed.

Captured via Playwright (chromium, 1400×900, headless) pointed at
the local dev server; input filled with `WORDS`, Enter submitted,
screenshot taken after the kernel finished producing output.

Also bumped the `?ts=` cache-bust on the README `<img>` URL
(1776917745967 → 1776918570363) and updated the alt text from
"Stack Ops demo running in the debugger" to "forth-on-forthish tab
with WORDS output" so it reflects what the image actually shows.

## 2026-04-22 — Fix Stale Footer Timestamp (build.rs Rerun Trigger)

Yesterday's fix for the stale footer date (the `.git/HEAD` rerun-trigger
fix in 5295bff) was **itself wrong** — `.git/HEAD` is a *symbolic
pointer* like `ref: refs/heads/main` whose content and mtime only change
on branch switches / detached-HEAD moves. Regular commits *do not* touch
`.git/HEAD`; they update `.git/refs/heads/<branch>` and append to
`.git/logs/HEAD`. So the previous trigger never fired after a commit,
and the footer timestamp was still stale — the deployed wasm had
`BUILD_TIMESTAMP=2026-04-23T02:46Z` baked in even after three later
commits.

Swapped `build.rs` to watch `.git/logs/HEAD` instead: the reflog
appends a line on every ref update (commit, checkout, pull, reset, …),
is branch-agnostic, and its mtime always reflects "last repo state
change". Verified: fresh `build-pages` run baked
`BUILD_TIMESTAMP=2026-04-23T04:16:45Z` into the wasm, matching the
actual build wall-clock.

Same lesson for anyone touching this again: `.git/HEAD` is a *ref
declaration*, not a *ref log*. Want "fires on every commit"? Use
`.git/logs/HEAD`.

## 2026-04-22 — README Screenshot Cache-Bust

Bumped the `?ts=` query param on `images/screenshot.png` in README.md
(1774477179000 → 1776917745967) so viewers of the GitHub page pull a
fresh copy if the file has been updated. File-mtime-based timestamps
would be more honest but arbitrary current-epoch-ms does the job.

## 2026-04-22 — Help Dialog: User Guide / Reference / Tutorial

The UI previously had no in-app documentation beyond the three per-tab
`?` About dialogs. Added a global Help dialog with three inner tabs
covering the needs of a first-time Forth user.

**New files:**

- `docs/user-guide.md` — what Forth is, what the three tabs are for,
  REPL mechanics (`ok` / `?` / comments), UI affordances (demos,
  history, S2/D2, status strip), and exploration tips (`WORDS`,
  `SEE`, `.S`, `DUMP-ALL`).
- `docs/tutorial.md` — 10-step hands-on walk from `2 3 + .` through
  stack manipulation, colon defs, `IF/THEN/ELSE`, `DO/LOOP`,
  `BEGIN/UNTIL`, variables/constants, `SEE` on your own words, base
  conversion, and `LED!` / `SW?` hardware I/O.
- `docs/reference.md` — every word across fif+fof kernels and core
  tiers, grouped by category (Stack, Arithmetic, Logic, Comparison,
  Return stack, Memory, Control, Runtime primitives, Compilation,
  Interpreter, I/O, Hardware, Introspection, Comments, System) with a
  top-of-page A–Z index. Tab-specific words are tagged.
- `src/help.rs` — new `Help` function component. Renders the embedded
  markdown through `pulldown-cmark` (tables + strikethrough enabled)
  and injects the resulting HTML via `Html::from_html_unchecked`. The
  md files are the source of truth and remain browsable on GitHub.

**Help button placement.** Lives in each tab's toolbar next to the
existing `About` button (orange-accented `.help-btn` sibling of
`.about-btn`). The first attempt put it in the header top-right, but
the `github-corner` SVG is `position: absolute; top: 0; right: 0;
z-index: 100` and was intercepting clicks — put Help in the toolbar
and the octocat stays out of the way.

**Dismissal** matches the `?` dialog trio: X button in the corner,
Esc key, and click-outside. Existing `use_effect_with` keydown
listener in `App` extended to close either kind of dialog.

**`.help-md` styles** cover headings, tables, inline code, code
blocks, lists, links, and `<hr>`, scoped so doc HTML doesn't bleed
into the rest of the app.

## 2026-04-22 — Test: Every Kernel Word Documented

New `help::tests::every_kernel_word_is_documented` unit test guards
against "added a word, forgot the docs" drift:

- Parses `entry_XXX:` dict blocks in `../sw-cor24-forth/forth-in-forth/
  kernel.s` and `../sw-cor24-forth/forth-on-forthish/kernel.s`,
  decoding each name via `flags_len & 0x3F` + the following
  comma-separated `.byte` list.
- Parses colon defs (`: WORD ...` at line start) from every
  `core/*.fth` in both tiers.
- Checks that each discovered word appears inside backticks in
  `docs/reference.md` (either opening ``\`WORD \`` or fully bracketed
  ``\`WORD\``). Loose enough to survive format churn, tight enough
  to avoid matching English words like "and".
- Sanity-guards with `all.len() > 50` so a silently-broken extractor
  doesn't pass the test vacuously.

Failure message lists the specific missing word(s), so next time
a primitive is added upstream the fix is just "write its reference
entry".

## 2026-04-22 — Dep: pulldown-cmark for Help rendering

`pulldown-cmark = "0.11"` added (default-features off, `html` feature
only) to render docs/*.md → HTML inside the Help dialog. Pure-Rust
CommonMark parser, no regex or other heavy deps; compiled wasm size
impact is modest. Tables + strikethrough extensions enabled.

## 2026-04-22 — Remove Per-Line Δcycles / Δinstrs Output

Reverted the per-command `[N cycles, M instrs]` markers I'd added in
`src/repl.rs` for tab-2-vs-tab-3 perf comparison. In practice the
output was noisy — at best one extra line per demo line (doubling the
visible output), at worst several spurious lines per real line on
tab 3 before a subsequent gating fix. The cumulative `Cycles:` /
`Instrs:` fields in the status strip already convey the same info
without cluttering the scroll buffer, and a reset-then-run gives a
clean per-demo total.

Removed: `last_ready_cycles` / `last_ready_instructions` /
`last_ready_output_len` fields, their init/reset in the `reboot` and
`create` paths, and the Δ emission block in the tick handler.
`Instrs:` status-strip field kept.

Root cause of the tab-3-only spurious output (intermittent, 4-of-5
runs): the `just_became_ready = !was_waiting && waiting_for_input`
transition detector depended on the PC being *outside* a UART-poll
range on one tick and *inside* it on the next. On tab 2 the asm WORD
sits tight in `key_poll` across char reads, so transitions only fire
at true end-of-line. On tab 3 Forth WORD returns from a byte-read to
do bytecode dispatch (PC outside the poll range) before calling KEY
again (PC back at poll), so every char read produced a transition —
exactly how many fired was race-y relative to the 60Hz tick vs the
emulator's per-tick execution rate, hence the intermittence.

## 2026-04-22 — Footer Build Metadata Freshness

Footer was displaying a stale date (`2026-04-21`) even on builds made
today. Root cause: `build.rs` captures `BUILD_SHA` / `BUILD_TIMESTAMP`
from `git`/`date` at script-execution time, but cargo only re-runs a
build script when one of its declared `cargo:rerun-if-changed` paths
changes. The declared set was limited to the `forth-in-forth` kernel
+ `core/*.fth` inputs (used by the embedded-snapshot pipeline). Edits
to `forth-on-forthish/`, `src/*.rs`, or CSS — none of which were
tracked — recompiled the crate but did *not* re-run `build.rs`, so
the `env!(...)` values in `src/lib.rs` stayed frozen at the last
fif-input change.

Added two more rerun triggers in `build.rs`:

- `.git/HEAD` — touched on every commit / branch switch, so the
  displayed SHA and timestamp now track the checked-out commit.
- `forth-on-forthish/kernel.s` + the five `core/*.fth` files — edits
  to the tab-3 tier now refresh build metadata (and leave the door
  open for a fof snapshot path later).

Tradeoff: build.rs now re-runs more often, which also triggers the
fif snapshot rebuild (~90s at release optimization). Acceptable —
it only happens on commits or fof edits, not on every incremental
`cargo build`.

## 2026-04-22 — Phase 3 Complete: Tab 3 Now Default + Final Sync

**Default tab flipped from forth-in-forth to forth-on-forthish.** Phase 3
is done upstream (subsets 12–21 all landed), so the "current best" tab
convention bumps forward. `src/lib.rs:24` + `CLAUDE.md` updated.

**Final upstream sync (subset 19 → 21 + phase-4 kickoff):**

- Subset 20 (commit 1c44e0d): `INTERPRET` and `QUIT` moved to Forth in
  `core/highlevel.fth`. A new `QUIT-VECTOR` asm primitive exposes the
  address of a cell holding the installed Forth `QUIT` CFA. Once
  `highlevel.fth` finishes loading, it installs Forth `QUIT` and the
  asm bootstrap hands off — every subsequent prompt line flows through
  Forth code, not asm.
- Subset 21 (commit 3970152): reg-rs baseline re-widened
  (`grep -A 100` → `-A 250`) to capture fib output past the ~22 extra
  " ok" lines emitted during Forth-INTERPRET-driven `highlevel.fth`
  load. Web-visible effect: tab 3's boot log is chattier now.
- Phase 4 kickoff (commit f7697b2): `forth-from-forth` agentrail
  saga — out of scope for this tab.

**Observable boot/runtime differences on tab 3:**

- ~22 more " ok" lines during core-tiers load (Forth INTERPRET echoes
  per line where the asm bootstrap was silent).
- Kernel 2630 → 2659 lines (+29, not the −180 originally planned).
  The asm bootstrap still needs STATE/IMMEDIATE/compile-mode to parse
  `runtime.fth`, and the three big asm bodies (`do_word` ~140,
  `do_find` ~250, `do_number` ~190) must stay alive for bootstrap
  address refs. Shrinking further needs a cross-compiled kernel or
  pre-compiled dict image — deferred to phase 4.
- One new user-visible word: `QUIT-VECTOR` (variable, inspectable via
  `SEE` / `@`).
- No new demos — upstream `examples/` unchanged since `19-do-loop.fth`,
  which was already wired into `FOF_DEMOS` via the `FIF_DEMOS` alias.

**Tab-3 `?` dialog rewritten** for phase-3-complete state. New
sections: "Status — phase 3 complete", "Moved asm → Forth (vs
forth-in-forth)", "New asm primitives" (now includes `QUIT-VECTOR`),
"Phase-3 honest note" on why the ≤800 asm-line target is deferred.
Removed the stale "work-in-progress upstream" hint.

**pages/ bundle refreshed** for GitHub Pages deployment.

## 2026-04-22 — forth-on-forthish Sync Through Subset 19 + Dialog UX Fixes

**Tab 3 synced with upstream through subset 19** (upstream HEAD 62daabb,
`feat(forth-on-forthish): subset 19 — NUMBER → Forth, DIGIT-VALUE helper`).
Kernel.s and core/*.fth are `include_str!`'d so they refresh on rebuild;
no source changes needed for kernel content. Status paragraph in the
tab 3 `?` dialog refreshed from "Subset 13 done" to reflect that subsets
13–19 are all in (: ; DUP DROP OVER SWAP R@ INVERT AND OR XOR NEGATE
− * /MOD WORD FIND NUMBER + helpers PICK / DIGIT-VALUE / STR= now in
Forth; asm primitives ,DOCOL SP@ SP! RP@ NAND WORD-BUFFER EOL-FLAG
added; kernel 2758 → 2630 lines, −128).

**New `core/runtime.fth` tier** wired into `FOF_CORE_FILES` as the
first-loaded core file (before minimal). This tier holds the Forth
definitions of `:` / `;` / stack ops that depend on the new `,DOCOL`
/ `SP@` / `SP!` / `RP@` / `NAND` primitives, and must load before
the other tiers can compile. Tab 2 (forth-in-forth) unchanged — its
core tiers remain minimal / lowlevel / midlevel / highlevel.

**Per-command Δcycles / Δinstrs output.** After the first idle-prompt
is reached (boot complete), every subsequent return-to-prompt emits
`[N cycles, M instrs]` — tracking the deltas between prompt-ready
transitions. Makes tab 2 vs tab 3 performance comparisons directly
visible without having to eyeball the running cycle counter. No delta
printed for the boot itself (baseline only).

**New `Instrs:` field in the status strip**, alongside the existing
`Cycles:` readout. Cheap — emulator snapshot already carried it.

**`?` help dialogs — three fixes:**

- **Dismissal.** Three ways now: X button in the top-right corner,
  Esc key, or click outside the dialog (outside was already working;
  X and Esc are new). Esc binds a document-level `keydown` listener
  only while a dialog is open, via `use_effect_with` + `gloo::events::
  EventListener` (RAII cleanup on close).
- **Width.** `max-width: 500px` → `width: min(720px, 92vw)`. Prose
  was wrapping too tightly on the tab-3 Status paragraph.
- **Scrolling.** Dialog is now a non-scrolling shell containing an
  inner `.about-content` scroll region, so the X button stays pinned
  in the corner even when the body scrolls. Previous behavior: tab
  3's dialog grew taller than the viewport, leaving its body cropped
  top and bottom and the Close button unreachable.

## 2026-04-21 — forth-on-forthish Tab + New Convenience-Word Demos

**New Tab 3: forth-on-forthish** — phase 3 of the self-hosting journey.
Pushes the asm kernel down toward the irreducible minimum (~22 primitives,
≤ 800 LOC target) by moving `:` / `;` / `WORD` / `FIND` / `NUMBER` /
`INTERPRET` / `QUIT` / `*` / `/MOD` / stack ops into Forth. At the time of
this commit the CLI side is at subset 13 partial (`,DOCOL` added, Forth
`:`/`;` landing). Tab 3 reads `../sw-cor24-forth/forth-on-forthish/kernel.s`
and `core/*.fth`; trunk will auto-rebuild as subsets land upstream.

**ForthRepl component refactored** to take `ReplProps { label, kernel_src,
core_files, demos }`. Tabs 2 and 3 now share the same component — only the
props differ. `src/repl.rs` no longer hardcodes forth-in-forth paths.

**Default tab is now forth-in-forth** (was forth.s). Convention: default to
the "current best" phase. Bumps to forth-on-forthish when subsets 13–21
land, then to phase 4 when that's ready.

**Five new convenience-word demos** (tab 2 + tab 3 via alias), matching
upstream `examples/15-19*.fth` added by the CLI-side agent:

- **AGAIN** — BEGIN AGAIN with IF EXIT, countdown loop
- **CONSTANT** — bind values to names (ANSWER, YEAR, UART-DATA)
- **DO LOOP** — DO/LOOP, ?DO, I, UNLOOP with counted loops + factorial
- **VARIABLE** — mutable cells via COUNTER / BUMP / RESET / SHOW
- **WHILE REPEAT** — test-in-middle loop via TRIANGLE (1+2+…+n)

Not added to tab 1 (`FORTH_S_DEMOS`) — forth.s's asm kernel lacks these
words; the demos would just print `?`.

**Alphabetical-order invariant** on all three demo lists
(`FORTH_S_DEMOS`, `FIF_DEMOS`, `FOF_DEMOS`), enforced by both a
compile-time `const assert_demos_sorted` that fails `cargo build`, and
per-list unit tests that name which pair is out of order.

**Per-tab `?` help dialogs updated**:

- Tab 2 (forth-in-forth): replaced "Slower boot" trade-off (fixed by
  the hashed FIND + lookaside + pump work) with a "Performance" section
  crediting the kernel and web fixes. Removed the ergonomic-backlog
  section since the CLI agent is actively adding those words. Added a
  "Further work" pointer to tab 3.
- Tab 3 (forth-on-forthish): new dialog describing approach, current
  subset 13 status, target line counts, and a link to the upstream dir.

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
