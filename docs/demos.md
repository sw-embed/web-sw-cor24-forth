# Forth Demos

All demos run on the COR24 Forth interpreter (Phase 5: compile mode, `*`, colon defs).

## Alphabetical List

### ASCII Stars (`04-stars.fth`)
`STAR`, `STARS`, `NL` — defines words using `EMIT` to print asterisk patterns. Demonstrates colon definitions that compose other words.

### Colon Definitions (`01-colon.fth`)
`TWO`, `SQUARE`, `CUBE` — compile mode fundamentals. Defines and calls colon definitions, including `DUP`, `*`, and nesting (`CUBE` calls `SQUARE`).

### Comments (`06-comments.fth`)
`\` and `( )` — line and inline comments. Demonstrates both comment styles.

### Division & Modulo (`07-divmod.fth`)
`/MOD`, `/`, `MOD` — unsigned integer division. Tests division, modulo, and the combined `/MOD` word.

### FizzBuzz (`11-fizzbuzz.fth`)
`BEGIN/UNTIL`, nested `IF/ELSE` — classic FizzBuzz 1-20. Defines `MOD`, `FIZZ`, `BUZZ`, `FIZZBUZZ`, `CHECK`, and `RUN` words.

### IF ELSE (`09-if-else.fth`)
`IF ELSE THEN` — conditional branching with else clauses.

### IF THEN (`08-if-then.fth`)
`IF THEN` — conditional execution without else.

### LED Control (`02-led.fth`)
`ON`, `OFF`, `BLINK` — hardware I/O via `LED!`. Defines words that write to the LED register at 0xFF0000.

### Loop (`10-loop.fth`)
`BEGIN UNTIL` — counted loops with counter. `UP` counts 1-5, `DOWN` counts 5-1.

### Loop Switch LED (`13-loop-switch-led.fth`)
`SW?` `LED!` in a 16000-iteration loop with triple-nested delay loops. Click S2 quickly to see the LED D2 change before the loop exits. Demonstrates real-time hardware polling.

### Math Words (`03-math.fth`)
`NEGATE`, `DOUBLE`, `TRIPLE`, `*` — arithmetic and multiplication.

### Self Test (`12-selftest.fth`)
Exercises all kernel words — stack should be empty at end, no `?` errors. Covers arithmetic, stack ops, number base, LED, per-word balance, logic, colon defs, control flow, version, error handling, and WORDS.

### Smoke Test (`00-smoke.fth`)
`.S`, arithmetic, `HEX`, `WORDS` — interpret-only smoke test. Prints stack contents, does math, switches to hex mode, and dumps the dictionary.

### Switch → LED (`05-switch-led.fth`)
`SW? LED!` — reads switch S2 and writes to LED D2 (one-shot). Click S2 first, then run.
