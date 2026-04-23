# User Guide

## What is Forth?

Forth is a **stack-based**, **postfix** programming language. Arguments come
before operators — you write `2 3 +` instead of `2 + 3`. Values accumulate
on a **data stack**; words (functions) pop their arguments from the stack
and push their results back.

Forth is interactive: every line you type is either evaluated immediately
or compiled into a new word definition. The language is tiny, extensible,
and self-hosting — this UI runs a Forth implementation that is itself
partially (tabs 2 and 3) or entirely (tab 3) written in Forth.

## What this UI is

A browser-based COR24 emulator running the `sw-cor24-forth` DTC Forth
kernel in WebAssembly. Type Forth at the REPL, step through threaded
code, inspect the VM — all client-side.

## The three tabs

| Tab | Kernel | Purpose |
|-----|--------|---------|
| `forth.s` | full asm (~3000 lines) | **Debugger**. Step/inspect the VM, watch stacks and registers, set breakpoints. Slower because every tick re-renders six panels. |
| `forth-in-forth` | minimized asm + Forth (~2700 lines asm) | **Self-hosting REPL**. Most of the high-level Forth (IF/THEN, NIP/TUCK, WORDS, SEE, …) is written in Forth in `core/*.fth` and loaded at boot. |
| `forth-on-forthish` *(default)* | minimal asm + `QUIT-VECTOR` handoff | **Phase-3 REPL**. Adds `: ; WORD FIND NUMBER INTERPRET QUIT` and the stack ops in Forth. Once `highlevel.fth` finishes loading, control transfers via `QUIT-VECTOR` to a Forth-written interpret/quit loop — every prompt line now runs through Forth code, not asm. |

Tabs 2 and 3 share a REPL component; tab 1 has a richer debugger layout.

## Using the REPL

- **Prompt** — the interactive kernel echoes ` ok` after each successfully
  interpreted line. On a parse failure it echoes `? ` (the word wasn't
  found and wasn't a number in the current `BASE`).
- **Comments** — `\` comments to end of line; `( ... )` comments inline.
- **Command history** — **↑** / **↓** step through recent input.
- **Demo dropdown** — curated `.fth` scripts. Pick one and click **Run**.
- **Upload `.fth`** — send your own file as if you typed it.
- **Run / Stop / Reset** — Reset reboots the kernel (you lose all your
  definitions); Stop pauses the pump loop without rebooting.
- **S2 switch** — click to press; `SW?` returns `-1` when pressed, `0`
  otherwise.
- **D2 LED** — `LED!` (non-zero on, zero off) lights the red dot.
- **Status strip** — cumulative Cycles / Instructions counters (reset
  button clears them along with the kernel).

## Exploring the kernel

| Word | What it does |
|------|--------------|
| `WORDS` | Lists every word in the dictionary, newest first. |
| `SEE <word>` | Decompiles a Forth colon def (tabs 2/3). |
| `.S` | Non-destructively prints the data stack contents. |
| `DUMP-ALL` | Prints every word's name + address (tabs 2/3). |
| `VER` | Kernel version banner. |
| `HEX` / `DECIMAL` | Change the numeric base for input and output. |

## Tips

- Start with the **Tutorial** tab of this Help dialog for a hands-on walk.
- The **Reference** tab lists every word with its stack effect.
- If the REPL seems frozen, the kernel may be compiling a long colon
  definition — wait a moment before hitting **Reset**.
- Boot of tab 2 is near-instant (hashed-FIND optimization); tab 3 is a
  bit slower because more of the bootstrap now runs through Forth.
- When the stack underflows mid-interpret-line, the kernel prints a
  message and returns you to the prompt; no need to reboot.

## Where the source lives

- UI (this app): [github.com/sw-embed/web-sw-cor24-forth](https://github.com/sw-embed/web-sw-cor24-forth)
- Forth kernel: [github.com/sw-embed/sw-cor24-forth](https://github.com/sw-embed/sw-cor24-forth)
- Emulator: [github.com/sw-embed/sw-cor24-emulator](https://github.com/sw-embed/sw-cor24-emulator)
- COR24 target board: [makerlisp.com](https://makerlisp.com)
