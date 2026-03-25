use crate::config::ForthTier;

pub struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub tier: ForthTier,
}

pub const DEMOS: &[Demo] = &[
    Demo {
        title: "LED Blink",
        description: "Toggle LED D2 on and off",
        source: include_str!("../demos/led-blink.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Arithmetic",
        description: "Basic math: +, -, negative numbers",
        source: include_str!("../demos/arithmetic.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Stack Ops",
        description: "DUP, SWAP, OVER, DROP, DEPTH, .S",
        source: include_str!("../demos/stack-ops.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Hex Mode",
        description: "HEX/DECIMAL base switching",
        source: include_str!("../demos/hex-mode.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Comparison",
        description: "=, <, 0= with true/false results",
        source: include_str!("../demos/comparison.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Return Stack",
        description: ">R, R>, R@ operations",
        source: include_str!("../demos/return-stack.fth"),
        tier: ForthTier::Interpreter,
    },
    Demo {
        title: "Words",
        description: "List all dictionary entries",
        source: include_str!("../demos/words.fth"),
        tier: ForthTier::Interpreter,
    },
];
