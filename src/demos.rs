use crate::config::ForthTier;

#[derive(PartialEq)]
pub struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub tier: ForthTier,
}

// ===== Per-tab kernel + core source constants =====
//
// Tab 2 (forth-in-forth) and Tab 3 (forth-on-forthish) both follow the
// same REPL pattern: assemble a kernel, feed core/*.fth tiers over UART
// at boot. The source files differ per tab; everything else is shared.

pub const FIF_KERNEL_SRC: &str = include_str!("../../sw-cor24-forth/forth-in-forth/kernel.s");

pub const FIF_CORE_FILES: &[(&str, &str)] = &[
    (
        "minimal",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/minimal.fth"),
    ),
    (
        "lowlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/lowlevel.fth"),
    ),
    (
        "midlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/midlevel.fth"),
    ),
    (
        "highlevel",
        include_str!("../../sw-cor24-forth/forth-in-forth/core/highlevel.fth"),
    ),
];

pub const FOF_KERNEL_SRC: &str = include_str!("../../sw-cor24-forth/forth-on-forthish/kernel.s");

pub const FOF_CORE_FILES: &[(&str, &str)] = &[
    (
        "minimal",
        include_str!("../../sw-cor24-forth/forth-on-forthish/core/minimal.fth"),
    ),
    (
        "lowlevel",
        include_str!("../../sw-cor24-forth/forth-on-forthish/core/lowlevel.fth"),
    ),
    (
        "midlevel",
        include_str!("../../sw-cor24-forth/forth-on-forthish/core/midlevel.fth"),
    ),
    (
        "highlevel",
        include_str!("../../sw-cor24-forth/forth-on-forthish/core/highlevel.fth"),
    ),
];

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

// ===== Adding a new demo =====
//
// When sw-cor24-forth lands new example files (e.g. a `15-do-loop.fth`
// demonstrating newly-added convenience words), adding them here is:
//
//   1. Pick which list(s): FORTH_S_DEMOS (tab 1, asm kernel) and/or
//      FIF_DEMOS (tab 2, self-hosting). FOF_DEMOS aliases FIF_DEMOS —
//      if a demo should appear on tab 3 but NOT tab 2 (or vice versa),
//      break the alias: `pub const FOF_DEMOS: &[Demo] = &[ … ];` with
//      its own curated entries.
//   2. Insert a `Demo { title, description, source, tier }` stanza in
//      the alphabetically-correct position by `title`. Do NOT append —
//      the compile-time `assert_demos_sorted` check below will fail the
//      build if lists drift out of order (and the tests at the bottom
//      report which pair is misplaced).
//   3. `source` is usually `include_str!("../../sw-cor24-forth/examples/NN-*.fth")`;
//      inline Rust string literals work too for tab-specific demos that
//      shouldn't live in the shared sibling repo.
//   4. `tier: ForthTier::Interpreter` for essentially every Forth demo.

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
        title: "AGAIN",
        description: "BEGIN AGAIN with IF EXIT — countdown loop",
        source: include_str!("../../sw-cor24-forth/examples/15-again.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "ASCII Stars",
        description: "STAR, STARS, NL — EMIT patterns",
        source: include_str!("../../sw-cor24-forth/examples/04-stars.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "CONSTANT",
        description: "Bind values to names — ANSWER, YEAR, UART-DATA",
        source: include_str!("../../sw-cor24-forth/examples/17-constant.fth"),
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
        title: "DO LOOP",
        description: "DO/LOOP, ?DO, I, UNLOOP — counted loops + factorial",
        source: include_str!("../../sw-cor24-forth/examples/19-do-loop.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Division & Modulo",
        description: "/MOD, /, MOD — unsigned integer division",
        source: include_str!("../../sw-cor24-forth/examples/07-divmod.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Fibonacci (looped)",
        description: "Same FIB; BEGIN/UNTIL counter prints FIB(0)..FIB(20)",
        source: FIB_LOOPED_SRC,
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Fibonacci (manual calls)",
        description: "Define FIB; hand-written 0 FIB . 1 FIB . … 10 FIB .",
        source: FIB_MANUAL_SRC,
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
        title: "SEE (all words)",
        description: "Define SQUARE, CUBE; DUMP-ALL decompiles every word",
        source: DUMP_ALL_SRC,
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
    Demo {
        title: "VARIABLE",
        description: "Mutable cells — COUNTER with BUMP/RESET/SHOW",
        source: include_str!("../../sw-cor24-forth/examples/18-variable.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "WHILE REPEAT",
        description: "BEGIN WHILE REPEAT — test-in-middle loop, TRIANGLE sum",
        source: include_str!("../../sw-cor24-forth/examples/16-while.fth"),
        tier: ForthTier::Interpreter,
    },
];

/// Tab 3 (forth-on-forthish) demo list. At scaffold phase (subset 12) the
/// kernel is a byte-for-byte copy of forth-in-forth's, so the same demos
/// apply. Will diverge as the CLI-side agent lands subsets 13+ that move
/// `:` / `;` / `WORD` / `FIND` / etc. into Forth — at which point we may
/// want tab-specific demos that exercise the new layered implementation.
pub const FOF_DEMOS: &[Demo] = FIF_DEMOS;

// ===== Alphabetical-order enforcement =====
//
// All demo dropdowns must be alphabetical by `title`. The const assertions
// below cause `cargo build` to fail at compile time if any list slips out
// of order. The unit test runs with `cargo test` and reports which pair
// is out of order — handy when the compile error is too terse.

/// Compile-time sorted-by-title check. Panics at build time with a static
/// message if any adjacent pair is out of order. Byte-wise comparison of
/// titles (titles are ASCII in practice, so this matches lexicographic
/// Unicode order for the characters we use).
const fn assert_demos_sorted(demos: &[Demo]) {
    let mut i = 1;
    while i < demos.len() {
        let prev = demos[i - 1].title.as_bytes();
        let cur = demos[i].title.as_bytes();
        let min_len = if prev.len() < cur.len() {
            prev.len()
        } else {
            cur.len()
        };
        let mut j = 0;
        while j < min_len && prev[j] == cur[j] {
            j += 1;
        }
        if j == min_len {
            if prev.len() > cur.len() {
                panic!("demo list not alphabetical by title — fix src/demos.rs");
            }
        } else if prev[j] > cur[j] {
            panic!("demo list not alphabetical by title — fix src/demos.rs");
        }
        i += 1;
    }
}

const _: () = assert_demos_sorted(FORTH_S_DEMOS);
const _: () = assert_demos_sorted(FIF_DEMOS);
const _: () = assert_demos_sorted(FOF_DEMOS);

#[cfg(test)]
mod tests {
    use super::*;

    fn check_sorted(name: &str, demos: &[Demo]) {
        for pair in demos.windows(2) {
            assert!(
                pair[0].title <= pair[1].title,
                "{name}: titles out of alphabetical order: {:?} before {:?}",
                pair[0].title,
                pair[1].title,
            );
        }
    }

    #[test]
    fn forth_s_demos_alphabetical() {
        check_sorted("FORTH_S_DEMOS", FORTH_S_DEMOS);
    }

    #[test]
    fn fif_demos_alphabetical() {
        check_sorted("FIF_DEMOS", FIF_DEMOS);
    }

    #[test]
    fn fof_demos_alphabetical() {
        check_sorted("FOF_DEMOS", FOF_DEMOS);
    }
}
