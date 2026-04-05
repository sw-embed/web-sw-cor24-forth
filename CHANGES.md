# Changelog

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
