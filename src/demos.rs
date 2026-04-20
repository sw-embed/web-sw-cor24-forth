use crate::config::ForthTier;

pub struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub tier: ForthTier,
}

/// Fibonacci for the forth-in-forth tab — variant A: define FIB, then drive
/// it with hand-written calls `0 FIB . 1 FIB . …`. No helper redefinitions
/// (NIP/TUCK/1- are already in core/lowlevel.fth). Inline so the sibling
/// examples/14-fib.fth forth-in-forth regression baseline stays untouched.
const FIB_MANUAL_SRC: &str = "\
: FIB ( n -- f )
  >R 0 1 R>
  BEGIN
    DUP 0=
    IF DROP SWAP DROP EXIT THEN
    >R
    SWAP OVER +
    R> 1 -
    0
  UNTIL
;
0 FIB . 1 FIB . 2 FIB . 3 FIB . 4 FIB . 5 FIB . 6 FIB . 7 FIB . 8 FIB . 9 FIB . 10 FIB .
";

/// Fibonacci for the forth-in-forth tab — variant B: same FIB, but the
/// caller is a colon def using BEGIN/UNTIL. Contrast with FIB_MANUAL_SRC
/// to see how little Forth you need for a loop once BEGIN/UNTIL is
/// available. The loop is wrapped in a : FIBS ; definition because
/// BEGIN/UNTIL are IMMEDIATE words that only work at compile time —
/// running them at the interpret-mode top level corrupts the dictionary.
const FIB_LOOPED_SRC: &str = "\
: FIB ( n -- f )
  >R 0 1 R>
  BEGIN
    DUP 0=
    IF DROP SWAP DROP EXIT THEN
    >R
    SWAP OVER +
    R> 1 -
    0
  UNTIL
;
: FIBS ( -- )
  0
  BEGIN
    DUP FIB .
    1 +
    DUP 21 =
  UNTIL
  DROP
;
FIBS
";

/// Decompile the whole dictionary via DUMP-ALL (defined in forth-in-forth
/// core/highlevel.fth). Defines two small colon words first so there's
/// something to SEE beyond what the kernel already provides.
const DUMP_ALL_SRC: &str = "\
: SQUARE DUP * ;
: CUBE DUP SQUARE * ;
DUMP-ALL
";

/// Tab 1 (forth.s) demo list. These all target the asm kernel; 14-fib.fth
/// defines its own NIP/TUCK/1-/etc. because forth.s doesn't provide them.
pub const FORTH_S_DEMOS: &[Demo] = &[
    Demo {
        title: "ASCII Stars",
        description: "STAR, STARS, NL — EMIT patterns",
        source: include_str!("../../sw-cor24-forth/examples/04-stars.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Colon Definitions",
        description: "TWO, SQUARE, CUBE — compile mode",
        source: include_str!("../../sw-cor24-forth/examples/01-colon.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Comments",
        description: "\\ and ( ) — line and inline comments",
        source: include_str!("../../sw-cor24-forth/examples/06-comments.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Division & Modulo",
        description: "/MOD, /, MOD — unsigned integer division",
        source: include_str!("../../sw-cor24-forth/examples/07-divmod.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Fibonacci",
        description: "FIB with NIP/TUCK/1+/1- helpers; FIB(0)..FIB(10)",
        source: include_str!("../../sw-cor24-forth/examples/14-fib.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "FizzBuzz",
        description: "BEGIN/UNTIL, nested IF/ELSE — classic 1-20",
        source: include_str!("../../sw-cor24-forth/examples/11-fizzbuzz.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "IF ELSE",
        description: "IF ELSE THEN — conditional branching",
        source: include_str!("../../sw-cor24-forth/examples/09-if-else.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "IF THEN",
        description: "IF THEN — conditional execution",
        source: include_str!("../../sw-cor24-forth/examples/08-if-then.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "LED Control",
        description: "ON/OFF words, BLINK — hardware I/O",
        source: include_str!("../../sw-cor24-forth/examples/02-led.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Loop",
        description: "BEGIN UNTIL — counted loops with counter",
        source: include_str!("../../sw-cor24-forth/examples/10-loop.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Loop Switch LED",
        description: "SW? LED! in 10000-iter loop — toggle S2 while running",
        source: include_str!("../../sw-cor24-forth/examples/13-loop-switch-led.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Math Words",
        description: "NEGATE, DOUBLE, TRIPLE, * multiply",
        source: include_str!("../../sw-cor24-forth/examples/03-math.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Self Test",
        description: "Exercise all kernel words — stack empty at end, no '?' errors",
        source: include_str!("../../sw-cor24-forth/examples/12-selftest.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Smoke Test",
        description: ".S, arithmetic, hex, WORDS — interpret-only",
        source: include_str!("../../sw-cor24-forth/examples/00-smoke.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Switch \u{2192} LED",
        description: "SW? LED! — click S2 first, then run",
        source: include_str!("../../sw-cor24-forth/examples/05-switch-led.fth"),
        tier: ForthTier::Interpreter,
    },
];

/// Tab 2 (forth-in-forth) demo list. Includes a DUMP-ALL demo (only works
/// on this tab, because DUMP-ALL is defined in core/highlevel.fth) and a
/// simplified FIB that relies on NIP/TUCK/1-/etc. being already present.
pub const FIF_DEMOS: &[Demo] = &[
    Demo {
        title: "SEE (all words)",
        description: "Define SQUARE, CUBE; DUMP-ALL decompiles every word",
        source: DUMP_ALL_SRC,
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "ASCII Stars",
        description: "STAR, STARS, NL — EMIT patterns",
        source: include_str!("../../sw-cor24-forth/examples/04-stars.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Colon Definitions",
        description: "TWO, SQUARE, CUBE — compile mode",
        source: include_str!("../../sw-cor24-forth/examples/01-colon.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Comments",
        description: "\\ and ( ) — line and inline comments",
        source: include_str!("../../sw-cor24-forth/examples/06-comments.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Division & Modulo",
        description: "/MOD, /, MOD — unsigned integer division",
        source: include_str!("../../sw-cor24-forth/examples/07-divmod.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Fibonacci (manual calls)",
        description: "Define FIB; hand-written 0 FIB . 1 FIB . … 10 FIB .",
        source: FIB_MANUAL_SRC,
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Fibonacci (looped)",
        description: "Same FIB; BEGIN/UNTIL counter prints FIB(0)..FIB(20)",
        source: FIB_LOOPED_SRC,
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "FizzBuzz",
        description: "BEGIN/UNTIL, nested IF/ELSE — classic 1-20",
        source: include_str!("../../sw-cor24-forth/examples/11-fizzbuzz.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "IF ELSE",
        description: "IF ELSE THEN — conditional branching",
        source: include_str!("../../sw-cor24-forth/examples/09-if-else.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "IF THEN",
        description: "IF THEN — conditional execution",
        source: include_str!("../../sw-cor24-forth/examples/08-if-then.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "LED Control",
        description: "ON/OFF words, BLINK — hardware I/O",
        source: include_str!("../../sw-cor24-forth/examples/02-led.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Loop",
        description: "BEGIN UNTIL — counted loops with counter",
        source: include_str!("../../sw-cor24-forth/examples/10-loop.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Loop Switch LED",
        description: "SW? LED! in 10000-iter loop — toggle S2 while running",
        source: include_str!("../../sw-cor24-forth/examples/13-loop-switch-led.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Math Words",
        description: "NEGATE, DOUBLE, TRIPLE, * multiply",
        source: include_str!("../../sw-cor24-forth/examples/03-math.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Self Test",
        description: "Exercise all kernel words — stack empty at end, no '?' errors",
        source: include_str!("../../sw-cor24-forth/examples/12-selftest.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Smoke Test",
        description: ".S, arithmetic, hex, WORDS — interpret-only",
        source: include_str!("../../sw-cor24-forth/examples/00-smoke.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Switch \u{2192} LED",
        description: "SW? LED! — click S2 first, then run",
        source: include_str!("../../sw-cor24-forth/examples/05-switch-led.fth"),
        tier: ForthTier::Interpreter,
    },
];
