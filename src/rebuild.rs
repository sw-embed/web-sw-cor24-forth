//! Tab 4's "Rebuild kernel": the running forth-from-forth kernel compiles
//! its own kernel.s from its Forth sources, and the result is compared with
//! the kernel it is running.
//!
//! This is the self-hosting fixed point that sw-cor24-forth checks with
//! `forth-from-forth/scripts/build-image.sh --check`, done inside the tab:
//! the same compiler (compiler/xcomp.fth), primitives (core/prims-asm.fth)
//! and core tiers, fed over the UART to the same kernel. No Yew here, so
//! tests/led_switch_demo.rs drives exactly this code headlessly.

use cor24_assembler::Assembler;

/// The Forth sources of forth-from-forth, embedded at compile time.
pub const XCOMP: &str = include_str!("../../sw-cor24-forth/forth-from-forth/compiler/xcomp.fth");
pub const PRIMS: &str = include_str!("../../sw-cor24-forth/forth-from-forth/core/prims.fth");
pub const PRIMS_ASM: &str =
    include_str!("../../sw-cor24-forth/forth-from-forth/core/prims-asm.fth");
pub const CORE: &[&str] = &[
    include_str!("../../sw-cor24-forth/forth-from-forth/core/runtime.fth"),
    include_str!("../../sw-cor24-forth/forth-from-forth/core/minimal.fth"),
    include_str!("../../sw-cor24-forth/forth-from-forth/core/lowlevel.fth"),
    include_str!("../../sw-cor24-forth/forth-from-forth/core/midlevel.fth"),
    include_str!("../../sw-cor24-forth/forth-from-forth/core/highlevel.fth"),
];

const BEGIN: &str = "!!BEGIN-KERNEL!!";
const END: &str = "!!END-KERNEL!!";

/// Number of comment lines build-image.sh puts at the top of kernel.s,
/// which the compiler itself does not print.
const HEADER_LINES: usize = 4;

/// The UART input for a rebuild, exactly as build-image.sh sends it to a
/// host kernel that already holds the core: the compiler, kernel mode, the
/// primitives, then the five tiers as source. The QUIT-VECTOR hand-off at
/// the end of highlevel.fth is left out (only the UART bootstrap needed
/// it). `xc-bye` stays 0, so the kernel keeps running afterwards.
pub fn input() -> String {
    let mut s = String::new();
    let mut push = |text: &str| {
        for line in text.lines() {
            s.push_str(line.trim_end_matches('\r'));
            s.push('\n');
        }
    };
    push(XCOMP);
    push("XC-KERNEL-BEGIN");
    push(PRIMS);
    push(PRIMS_ASM);
    push("XC-COMPILE-SOURCE");
    for tier in CORE {
        let kept: Vec<&str> = tier
            .lines()
            .filter(|l| {
                let l = l.trim_end_matches('\r');
                l != "' QUIT QUIT-VECTOR !" && l != "QUIT"
            })
            .collect();
        push(&kept.join("\n"));
    }
    push("XC-END");
    s
}

/// The assembly between the markers, without the REPL's " ok" lines, or
/// None while the end marker has not arrived.
pub fn extract(output: &str) -> Option<String> {
    // Same rule as build-image.sh: the lines strictly between the marker
    // lines, minus the REPL's " ok" lines. Empty lines are kept.
    let start = output.find(BEGIN)?;
    let end = output[start..].find(END)? + start;
    let body: Vec<&str> = output[start + BEGIN.len()..end]
        .lines()
        .skip(1) // the rest of the BEGIN marker's line
        .filter(|l| *l != " ok")
        .collect();
    Some(body.join("\n") + "\n")
}

/// How the rebuilt kernel compares with the running one.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub lines: usize,
    /// Assembled size, or the assembler's first error.
    pub bytes: Result<usize, String>,
    /// Same text as the running kernel.s (ignoring its comment header).
    pub same_text: bool,
    /// Same bytes once assembled.
    pub same_bytes: bool,
    /// First differing line (1-based, in the rebuilt text), if any.
    pub first_difference: Option<(usize, String, String)>,
}

/// Assemble the rebuilt text and compare it with the running kernel's.
pub fn verdict(rebuilt: &str, running_src: &str) -> Verdict {
    let running_body: Vec<&str> = running_src.lines().skip(HEADER_LINES).collect();
    let rebuilt_lines: Vec<&str> = rebuilt.lines().collect();
    let same_text = rebuilt_lines == running_body;
    let first_difference = (!same_text).then(|| {
        let n = rebuilt_lines
            .iter()
            .zip(running_body.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(rebuilt_lines.len().min(running_body.len()));
        (
            n + 1,
            rebuilt_lines.get(n).unwrap_or(&"<end>").to_string(),
            running_body.get(n).unwrap_or(&"<end>").to_string(),
        )
    });
    let new = Assembler::new().assemble(rebuilt);
    let old = Assembler::new().assemble(running_src);
    let bytes = if new.errors.is_empty() {
        Ok(new.bytes.len())
    } else {
        Err(new.errors[0].clone())
    };
    let same_bytes = new.errors.is_empty() && old.errors.is_empty() && new.bytes == old.bytes;
    Verdict {
        lines: rebuilt_lines.len(),
        bytes,
        same_text,
        same_bytes,
        first_difference,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_waits_for_the_end_marker_and_drops_ok_lines() {
        assert_eq!(extract("x\n!!BEGIN-KERNEL!!\nl:\n ok\n"), None);
        assert_eq!(
            extract("x\n!!BEGIN-KERNEL!!\nl:\n ok\n\n.word 0\n!!END-KERNEL!!\n ok\n"),
            Some("l:\n\n.word 0\n".to_string())
        );
    }

    #[test]
    fn input_leaves_out_the_quit_vector_hand_off_and_ends_with_xc_end() {
        let s = input();
        assert!(!s.lines().any(|l| l == "' QUIT QUIT-VECTOR !"));
        assert!(s.ends_with("XC-END\n"));
        assert!(!s.contains('\r'));
    }
}
