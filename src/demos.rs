use crate::config::ForthTier;

pub struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub tier: ForthTier,
}

/// Demos sourced from sw-cor24-forth/examples/*.fth (alphabetized)
pub const DEMOS: &[Demo] = &[
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
        title: "Math Words",
        description: "NEGATE, DOUBLE, TRIPLE, * multiply",
        source: include_str!("../../sw-cor24-forth/examples/03-math.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Smoke Test",
        description: ".S, arithmetic, hex, WORDS — interpret-only",
        source: include_str!("../../sw-cor24-forth/examples/00-smoke.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Self Test",
        description: "Exercise all kernel words — stack empty at end, no '?' errors",
        source: include_str!("../../sw-cor24-forth/examples/12-selftest.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Switch \u{2192} LED",
        description: "S2-D2! — click S2 first, then run",
        source: include_str!("../../sw-cor24-forth/examples/05-switch-led.fth"),
        tier: ForthTier::Interpreter,
    },
];
