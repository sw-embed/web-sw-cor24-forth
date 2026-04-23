# Tutorial

A hands-on walk through Forth in this UI. Type each example at the REPL
and press Enter. Expected output is shown after the `→`.

## 1. Your first expression

    2 3 + .
    → 5  ok

Forth is postfix: `2` pushes 2, `3` pushes 3, `+` pops both and pushes 5,
`.` pops the top and prints it. `ok` is the kernel saying "I interpreted
that line without errors".

## 2. The stack is visible

    1 2 3 .S
    → <3> 1 2 3  ok

`.S` shows what's on the stack **without** consuming it. `<3>` is the
depth; `1 2 3` are the items, bottom-to-top — the rightmost is the
top-of-stack (TOS).

    DROP .S
    → <2> 1 2  ok

`DROP` removes the top. Try `SWAP`, `OVER`, `DUP`:

    DUP .S
    → <3> 1 2 2  ok
    SWAP .S
    → <3> 1 2 2  ok     ( 1 and 2 were in that order already here — try it with different values )
    OVER .S
    → <4> 1 2 2 2  ok

Tidy up before moving on:

    2DROP 2DROP .S
    → <0>  ok

## 3. Your first definition

Colon `:` starts a definition; semicolon `;` ends it.

    : SQUARE  DUP * ;
      ok
    5 SQUARE .
    → 25  ok
    3 SQUARE .
    → 9  ok

Definitions live in the dictionary until you **Reset** the kernel.

## 4. Conditionals

`IF ... ELSE ... THEN` is a **compile-time** construct. You use it
**inside** a colon definition:

    : SIGN  ( n -- -1|0|1 )
        DUP 0< IF DROP -1 EXIT THEN
        0= IF 0 ELSE 1 THEN ;
      ok
    -7 SIGN .
    → -1  ok
    0 SIGN .
    → 0  ok
    42 SIGN .
    → 1  ok

## 5. Counted loops

`DO ... LOOP` iterates between a limit and start (limit first):

    : COUNT  10 0 DO I . LOOP ;
      ok
    COUNT
    → 0 1 2 3 4 5 6 7 8 9  ok

`I` is the loop index. `?DO` skips the loop entirely if limit equals start.

## 6. Indefinite loops

`BEGIN ... UNTIL` loops until TOS is non-zero:

    : COUNTDOWN  ( n -- )
        BEGIN DUP . 1- DUP 0= UNTIL DROP ;
      ok
    5 COUNTDOWN
    → 5 4 3 2 1  ok

`BEGIN ... AGAIN` is an infinite loop; use `EXIT` to break out.

## 7. Variables and constants

    VARIABLE COUNTER   ok
    42 COUNTER !       ok
    COUNTER @ .
    → 42  ok

    3.14159 CONSTANT PI-ISH   ok   ( well, as an integer — Forth is typeless )
    PI-ISH .
    → 3  ok    ( the `.14159` parses as next-line input — try an integer constant instead )

For strictly integer work:

    42 CONSTANT ANSWER   ok
    ANSWER .
    → 42  ok

## 8. Explore the dictionary

    WORDS

Scrolls through every word the kernel knows. Recent words (your own
definitions) come first.

    SEE SQUARE

Decompiles your own colon definition back to readable Forth (tabs 2/3
only — tab 1's asm kernel doesn't include `SEE`).

    SEE IF

Shows that `IF` itself is a Forth-level definition — the kernel is
self-hosting.

## 9. Base conversion

    HEX   ok
    255 .
    → FF  ok
    DECIMAL   ok
    FF .
    → ?

`?` because in decimal base, `FF` isn't a number. Switch back with `HEX`.

## 10. Hardware I/O

    1 LED!   ok
    ( the red D2 dot lights up )
    0 LED!   ok
    ( dot off )

    SW?  .
    → 0  ok   ( or -1 if you're holding S2 )

## Next steps

- Open the **Reference** tab to skim the full vocabulary.
- Try the built-in demos (dropdown, top of the REPL).
- Read `SEE` output for any word you're curious about — the Forth parts
  of the kernel are instructive.
