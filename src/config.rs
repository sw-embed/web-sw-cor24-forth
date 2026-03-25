/// Forth tier determines which pre-compiled assembly to load.
///
/// As tf24a grows through development phases, new tiers can be added
/// with progressively richer word sets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ForthTier {
    /// Phase 1: Bootstrap, UART I/O, stack tests
    Bootstrap,
}

impl ForthTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bootstrap => "Bootstrap",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Bootstrap => "Phase 1: UART I/O, data/return stack tests, EMIT, KEY",
        }
    }

    pub fn assembly(self) -> &'static str {
        match self {
            Self::Bootstrap => include_str!("../asm/forth-bootstrap.s"),
        }
    }

    pub const ALL: [ForthTier; 1] = [Self::Bootstrap];
}

/// Stack size configuration for the COR24 EBR region.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StackSize {
    /// 3 KB - matches MachXO hardware default
    ThreeKb,
    /// 8 KB - full EBR window, needed for deep recursion
    EightKb,
}

impl StackSize {
    pub fn initial_sp(self) -> u32 {
        match self {
            Self::ThreeKb => 0xFEEC00,
            Self::EightKb => 0xFF0000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ThreeKb => "3 KB",
            Self::EightKb => "8 KB",
        }
    }

    pub fn bytes(self) -> u32 {
        match self {
            Self::ThreeKb => 3072,
            Self::EightKb => 8192,
        }
    }

    pub const ALL: [StackSize; 2] = [Self::ThreeKb, Self::EightKb];
}
