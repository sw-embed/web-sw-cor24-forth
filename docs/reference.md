# Reference

Every word available across the three tabs, grouped by category. Each
entry is `WORD  ( stack-effect )  — short description`.

**Stack notation:** `( before -- after )`. Rightmost item is the top.
`n` = signed number, `u` = unsigned, `c` = byte, `addr` = address,
`xt` = execution token (CFA), `flag` = `0` (false) or non-zero (true),
`-1` = Forth canonical true.

**Tab annotations in parens:**
- *(all)* — available on every tab.
- *(tab 2+3)* — forth-in-forth and forth-on-forthish only.
- *(tab 3)* — forth-on-forthish only.

## Quick index

Symbols: [`!`](#memory) [`"`](#strings) [`#`](#numeric-output)
[`'`](#compilation) [`(`](#comments) [`*`](#arithmetic) [`+`](#arithmetic)
[`,`](#memory) [`-`](#arithmetic) [`.`](#io)
[`/`](#arithmetic) [`/MOD`](#arithmetic) [`0<`](#comparison)
[`0=`](#comparison) [`0BRANCH`](#runtime-threaded-code-primitives)
[`1+`](#arithmetic) [`1-`](#arithmetic)
[`2DROP`](#stack-manipulation) [`2DUP`](#stack-manipulation)
[`2OVER`](#stack-manipulation) [`2SWAP`](#stack-manipulation)
[`:`](#compilation) [`;`](#compilation) [`<`](#comparison)
[`=`](#comparison) [`>NAME`](#introspection) [`>R`](#return-stack)
[`?DO`](#control-flow) [`@`](#memory) [`[`](#compilation)
[`[']`](#compilation) [`\`](#comments) [`]`](#compilation)

A–Z: ABS ALLOT AND AGAIN BASE BEGIN BRANCH BYE C! C@ CONSTANT CR
CREATE DECIMAL DEPTH DIGIT-VALUE DO DROP DUMP-ALL DUP ELSE EMIT EOL!
EOL-FLAG EXECUTE EXIT FIND HERE HEX I IF IMMEDIATE INTERPRET INVERT
KEY LATEST LED! LIT LOOP MOD NAND NEGATE NIP NUMBER OR OVER PICK
PRIM-MARKER PRINT-NAME QUIT QUIT-VECTOR R@ R> REPEAT ROT -ROT SEE
SEE-CFA SP! SP@ SPACE RP@ RP! STATE STR= SW? SWAP THEN TUCK UNLOOP
UNTIL VARIABLE VER WHILE WORD WORDS WORD-BUFFER XOR `,DOCOL` `(DO)`
`(LOOP)` `(?DO)`

---

## Stack manipulation

- `DUP       ( n -- n n )` — duplicate the top.
- `DROP      ( n -- )` — discard the top.
- `SWAP      ( a b -- b a )` — swap the top two.
- `OVER      ( a b -- a b a )` — copy the second-from-top.
- `NIP       ( a b -- b )` — drop the second. *(tab 2+3)*
- `TUCK      ( a b -- b a b )` — copy top under second. *(tab 2+3)*
- `ROT       ( a b c -- b c a )` — rotate top three left. *(tab 2+3)*
- `-ROT      ( a b c -- c a b )` — rotate top three right. *(tab 2+3)*
- `2DUP      ( a b -- a b a b )` — duplicate top pair. *(tab 2+3)*
- `2DROP     ( a b -- )` — drop top pair. *(tab 2+3)*
- `2SWAP     ( a b c d -- c d a b )` — swap top two pairs. *(tab 2+3)*
- `2OVER     ( a b c d -- a b c d a b )` — copy second pair. *(tab 2+3)*
- `PICK      ( xn … x0 u -- xn … x0 xu )` — copy the u-th item
  (0-based). *(tab 2+3)*
- `DEPTH     ( -- n )` — current stack depth (does not include the
  pushed `n`). *(tab 2+3)*
- `.S        ( -- )` — print stack non-destructively: `<depth> a b c`.
  *(tab 2+3)*

## Arithmetic

- `+         ( a b -- a+b )` — add.
- `-         ( a b -- a-b )` — subtract.
- `*         ( a b -- a*b )` — multiply.
- `/         ( a b -- a/b )` — signed division. *(tab 2+3)*
- `MOD       ( a b -- a mod b )` — remainder. *(tab 2+3)*
- `/MOD      ( a b -- rem quot )` — division with remainder.
- `1+        ( n -- n+1 )` — increment. *(tab 2+3)*
- `1-        ( n -- n-1 )` — decrement. *(tab 2+3)*
- `NEGATE    ( n -- -n )` — two's-complement negate. *(tab 2+3)*
- `ABS       ( n -- |n| )` — absolute value. *(tab 2+3)*

## Logic (bitwise)

- `AND       ( a b -- a&b )` — bitwise AND.
- `OR        ( a b -- a|b )` — bitwise OR.
- `XOR       ( a b -- a^b )` — bitwise XOR.
- `INVERT    ( a -- ~a )` — one's complement. *(tab 2+3)*
- `NAND      ( a b -- ~(a&b) )` — primitive used to derive AND/OR/XOR
  in forth-on-forthish. *(tab 3)*

## Comparison

- `=         ( a b -- flag )` — equal? *(tab 2+3)*
- `<         ( a b -- flag )` — less-than?
- `0=        ( n -- flag )` — zero? *(tab 2+3)*
- `0<        ( n -- flag )` — negative? *(tab 2+3)*

Forth flags: canonical true is `-1`, false is `0`.

## Return stack

- `>R        ( n -- ) ( R: -- n )` — push to return stack.
- `R>        ( -- n ) ( R: n -- )` — pop from return stack.
- `R@        ( -- n ) ( R: n -- n )` — copy top of return stack.
- `I         ( -- n )` — inner-loop index (inside `DO ... LOOP`).
- `UNLOOP    ( -- )` — remove loop-control pair from return stack.

## Memory

- `@         ( addr -- n )` — fetch cell.
- `!         ( n addr -- )` — store cell.
- `C@        ( addr -- c )` — fetch byte.
- `C!        ( c addr -- )` — store byte.
- `,         ( n -- )` — append cell to the dictionary; bumps `HERE`.
- `C,        ( c -- )` — append byte.
- `ALLOT     ( n -- )` — reserve n bytes at `HERE`.
- `HERE      ( -- addr )` — current dictionary pointer (next free byte).
- `LATEST    ( -- addr )` — variable holding newest dict entry's address.
- `SP@       ( -- addr )` — data-stack pointer.
- `SP!       ( addr -- )` — set data-stack pointer. *(tab 3)*
- `RP@       ( -- addr )` — return-stack pointer. *(tab 3)*
- `RP!       ( addr -- )` — set return-stack pointer. *(tab 3)*

## Control flow

Compile-time; use inside `:` definitions.

- `IF        ( flag -- )` — begin conditional; runs to `THEN` or `ELSE`
  when flag is non-zero. *(all)*
- `ELSE      ( -- )` — alternate branch.
- `THEN      ( -- )` — end `IF` / `ELSE`.
- `BEGIN     ( -- )` — start indefinite loop.
- `UNTIL     ( flag -- )` — exit `BEGIN` loop when flag is non-zero.
- `AGAIN     ( -- )` — unconditional jump back to `BEGIN`; use `EXIT`
  or throw to leave. *(tab 2+3)*
- `WHILE     ( flag -- )` — mid-loop test. *(tab 2+3)*
- `REPEAT    ( -- )` — close a `BEGIN...WHILE...REPEAT` loop. *(tab 2+3)*
- `DO        ( limit start -- )` — counted-loop opener. *(tab 2+3)*
- `?DO       ( limit start -- )` — like `DO` but skips if limit==start.
  *(tab 2+3)*
- `LOOP      ( -- )` — close `DO`; increments `I` by 1. *(tab 2+3)*
- `EXIT      ( -- )` — return from current colon definition.

## Runtime (threaded-code primitives)

These are emitted by `IF`/`DO`/etc. into compiled Forth. You normally
don't type them at the REPL.

- `LIT         ( -- n )` — followed inline by a cell; pushes it.
- `BRANCH      ( -- )` — unconditional jump (inline target).
- `0BRANCH     ( flag -- )` — jump if flag is zero (inline target).
- `(DO)        ( limit start -- )` — loop-entry primitive. *(tab 2+3)*
- `(?DO)       ( limit start -- )` — skip-if-equal entry. *(tab 2+3)*
- `(LOOP)      ( -- )` — loop-back primitive. *(tab 2+3)*

## Compilation

- `:         ( "name" -- )` — begin a colon definition.
- `;         ( -- )` — end; IMMEDIATE.
- `CREATE    ( "name" -- )` — make a dictionary entry whose runtime
  pushes its PFA.
- `VARIABLE  ( "name" -- )` — `CREATE` + allot a cell. *(tab 2+3)*
- `CONSTANT  ( n "name" -- )` — make a word that pushes n. *(tab 2+3)*
- `IMMEDIATE ( -- )` — mark the last word IMMEDIATE (runs during
  compilation).
- `[         ( -- )` — switch to interpret mode; IMMEDIATE.
- `]         ( -- )` — switch to compile mode.
- `'         ( "name" -- xt )` — tick: look up a word's CFA at
  interpret time. *(tab 2+3)*
- `[']       ( "name" -- xt )` — compile-time tick; IMMEDIATE.
- `,DOCOL    ( -- )` — compile the colon-def runtime header; used by
  forth-on-forthish's Forth `:` implementation.

## Interpreter / parser

- `WORD      ( c -- c-addr )` — parse next whitespace-delimited word.
- `FIND      ( c-addr -- xt flag | c-addr 0 )` — look up a word in the
  dictionary; flag is `1` for IMMEDIATE, `-1` for normal, `0` on
  failure (c-addr left on stack in that case).
- `NUMBER    ( c-addr -- n 0 | 0 -1 )` — parse counted string as a
  number in the current `BASE`.
- `DIGIT-VALUE ( c -- n -1 | c 0 )` — digit-lookup helper used by
  `NUMBER`. *(tab 2+3)*
- `STR=      ( c-addr1 c-addr2 n -- flag )` — byte-wise string compare.
  *(tab 2+3)*
- `INTERPRET ( -- )` — read one line of input; interpret or compile
  each word.
- `QUIT      ( -- )` — the outer REPL: reset return stack,
  `BEGIN INTERPRET ok AGAIN`.
- `EXECUTE   ( xt -- )` — jump to an execution token.
- `STATE     ( -- addr )` — variable: 0 = interpret, non-zero =
  compile.
- `BASE      ( -- addr )` — numeric base variable (10 / 16 / …).
- `EOL!      ( -- )` — mark end-of-line for the parser. *(tab 2)*
- `EOL-FLAG  ( -- addr )` — end-of-line flag; parsers read/write via
  `@` / `!`. *(tab 3)*
- `WORD-BUFFER ( -- addr )` — address of the parser's scratch buffer.
  *(tab 3)*
- `QUIT-VECTOR ( -- addr )` — variable holding the CFA of the Forth
  `QUIT` (set by `highlevel.fth` during subset-20 handoff). *(tab 3)*

## I/O

- `EMIT      ( c -- )` — print one character.
- `KEY       ( -- c )` — block until UART RX has a byte; return it.
- `.         ( n -- )` — print n in the current `BASE` followed by a
  space.
- `CR        ( -- )` — print newline.
- `SPACE     ( -- )` — print one space.
- `HEX       ( -- )` — set `BASE` to 16.
- `DECIMAL   ( -- )` — set `BASE` to 10.

## Hardware (COR24)

- `LED!      ( n -- )` — D2 LED: non-zero on, zero off.
- `SW?       ( -- flag )` — S2 switch: `-1` pressed, `0` released.

## Introspection

- `WORDS     ( -- )` — list all dictionary entries.
- `SEE       ( "name" -- )` — decompile a Forth colon def. *(tab 2+3)*
- `SEE-CFA   ( xt -- )` — decompile by CFA. *(tab 2+3)*
- `DUMP-ALL  ( -- )` — list every word with its address. *(tab 2+3)*
- `PRINT-NAME ( addr -- )` — print a dictionary entry's name.
  *(tab 2+3)*
- `>NAME    ( xt -- addr )` — CFA → name-address. *(tab 2+3)*
- `PRIM-MARKER ( -- addr )` — boundary between asm primitives and
  Forth colon defs; used by `SEE` to format differently. *(tab 2+3)*
- `VER       ( -- )` — print kernel version banner.

## Comments

- `\         — comment to end of line.`
- `(         — comment until matching ')'.`

## System

- `BYE       ( -- )` — halt the emulator.
