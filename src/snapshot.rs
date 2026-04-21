//! Bootstrap-state cache for the forth-in-forth REPL.
//!
//! After the slow UART-bootstrap of `kernel.s` + `core/*.fth` completes, we
//! dump the emulator's low memory + register file to `localStorage` keyed
//! by a content hash of the source. On subsequent loads we restore the
//! snapshot directly — no UART feed, no FIND scans, no threaded IMMEDIATE
//! words replayed. Load time drops to whatever the WASM write loop costs.
//!
//! Invalidation is automatic: any edit to `kernel.s` or `core/*.fth`
//! changes the hash and the old snapshot is ignored (then overwritten after
//! the next successful cold boot).

use cor24_emulator::EmulatorCore;
use serde::{Deserialize, Serialize};
use web_sys::Storage;

/// Build-time embedded snapshot. Written by `build.rs` after running the
/// cold bootstrap natively. If the build couldn't produce a snapshot
/// (missing sources, bootstrap timeout, assembly error) this is a zero-
/// length slice and the runtime falls through to the slow UART path.
pub const EMBEDDED_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fif_snapshot.bin"));

/// Bytes of low memory captured. 64 KB covers the kernel (~3 KB), the
/// growing dictionary (~2 KB after full core load), and leaves wide headroom
/// for any user definitions typed into the REPL before we'd overwrite on
/// reset. The data stack (EBR @ 0xFEEC00) and return stack (0xF0000) live
/// outside this range but are empty at the idle-at-KEY-poll moment we
/// snapshot, so we don't need to capture them.
const SNAPSHOT_BYTES: u32 = 64 * 1024;

/// Storage key prefix. Bump the suffix if the snapshot layout ever changes.
const KEY_PREFIX: &str = "fif-snapshot-v1-";

/// Serialized form. `memory_b64` keeps the blob small vs. a JSON number
/// array. Registers + PC + c + cycles give the CPU enough to resume at
/// the saved point — the kernel restarts in the middle of its KEY poll.
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub memory_b64: String,
    pub regs: [u32; 8],
    pub pc: u32,
    pub c: bool,
    pub cycles: u64,
    pub instructions: u64,
}

/// Compute a stable content hash of the kernel + all core files. Plain
/// djb2 — not cryptographic, but with 64 bits of output and only a handful
/// of possible inputs, collisions are astronomically unlikely for our
/// cache-key purpose. `build.rs` runs the same algorithm so the embedded
/// blob's hash and the runtime hash match bit-for-bit.
pub fn content_hash(kernel_src: &str, core_files: &[(&str, &str)]) -> u64 {
    let mut h: u64 = 5381;
    for b in kernel_src.as_bytes() {
        h = h.wrapping_mul(33).wrapping_add(*b as u64);
    }
    // Separator byte so concatenation ambiguity can't cause collisions.
    h = h.wrapping_mul(33).wrapping_add(0xFF);
    for (name, src) in core_files {
        for b in name.as_bytes() {
            h = h.wrapping_mul(33).wrapping_add(*b as u64);
        }
        for b in src.as_bytes() {
            h = h.wrapping_mul(33).wrapping_add(*b as u64);
        }
        h = h.wrapping_mul(33).wrapping_add(0xFF);
    }
    h
}

pub fn content_key(kernel_src: &str, core_files: &[(&str, &str)]) -> String {
    format!(
        "{}{:016x}",
        KEY_PREFIX,
        content_hash(kernel_src, core_files)
    )
}

fn local_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Capture emulator state. Called once the REPL transitions from
/// "bootstrapping" to "ready".
pub fn capture(emulator: &EmulatorCore) -> Snapshot {
    let snap = emulator.snapshot();
    let bytes = emulator.read_memory(0, SNAPSHOT_BYTES);
    Snapshot {
        memory_b64: b64_encode(&bytes),
        regs: snap.regs,
        pc: snap.pc,
        c: snap.c,
        cycles: snap.cycles,
        instructions: snap.instructions,
    }
}

/// Persist a snapshot under `key`. Returns true if stored. Silent failures
/// (quota exceeded, private-mode blocks, serde error) just skip the cache;
/// the REPL still works, just without the fast path on next load.
pub fn save(key: &str, snap: &Snapshot) -> bool {
    let Some(storage) = local_storage() else {
        return false;
    };
    let Ok(json) = serde_json::to_string(snap) else {
        return false;
    };
    storage.set_item(key, &json).is_ok()
}

/// Try to load a snapshot under `key`.
pub fn load(key: &str) -> Option<Snapshot> {
    let storage = local_storage()?;
    let json = storage.get_item(key).ok().flatten()?;
    serde_json::from_str(&json).ok()
}

struct Parsed {
    content_hash: u64,
    memory: Vec<u8>,
    regs: [u32; 8],
    pc: u32,
    #[allow(dead_code)]
    c: bool,
}

/// Try to restore from the build-time embedded blob. Returns true only if
/// the blob is non-empty, parses cleanly, AND its content hash matches
/// `expected_hash` — so a stale blob from a previous build doesn't get
/// applied against a later runtime with edited sources.
pub fn restore_from_embedded(emulator: &mut EmulatorCore, expected_hash: u64) -> bool {
    let Some(p) = parse_embedded(EMBEDDED_BLOB) else {
        return false;
    };
    if p.content_hash != expected_hash {
        return false;
    }
    emulator.hard_reset();
    emulator.load_program(0, &p.memory);
    emulator.load_program_extent(p.memory.len() as u32);
    emulator.set_pc(p.pc);
    for (i, &v) in p.regs.iter().enumerate() {
        emulator.set_reg(i as u8, v);
    }
    emulator.resume();
    true
}

/// Parse the on-disk blob layout emitted by build.rs. Returns None if the
/// blob is empty, truncated, or has the wrong magic.
fn parse_embedded(blob: &[u8]) -> Option<Parsed> {
    if blob.len() < 16 || &blob[0..4] != b"FIF1" {
        return None;
    }
    let content_hash = u64::from_le_bytes(blob[4..12].try_into().ok()?);
    let memory_len = u32::from_le_bytes(blob[12..16].try_into().ok()?) as usize;
    let mem_end = 16 + memory_len;
    // memory + 8 regs (32) + pc (4) + c (1) + cycles (8) + instructions (8) = 53
    if blob.len() < mem_end + 53 {
        return None;
    }
    let memory = blob[16..mem_end].to_vec();
    let mut regs = [0u32; 8];
    for (i, reg) in regs.iter_mut().enumerate() {
        let start = mem_end + i * 4;
        *reg = u32::from_le_bytes(blob[start..start + 4].try_into().ok()?);
    }
    let pc_start = mem_end + 32;
    let pc = u32::from_le_bytes(blob[pc_start..pc_start + 4].try_into().ok()?);
    let c = blob[pc_start + 4] != 0;
    Some(Parsed {
        content_hash,
        memory,
        regs,
        pc,
        c,
    })
}

/// Whether the build-time blob is present and matches the current sources.
/// Useful for UI hints ("fast boot available").
#[allow(dead_code)]
pub fn has_valid_embedded(expected_hash: u64) -> bool {
    parse_embedded(EMBEDDED_BLOB).is_some_and(|p| p.content_hash == expected_hash)
}

/// Restore emulator state from a snapshot. Returns true on success.
pub fn restore(emulator: &mut EmulatorCore, snap: &Snapshot) -> bool {
    let Some(bytes) = b64_decode(&snap.memory_b64) else {
        return false;
    };
    if bytes.len() as u32 != SNAPSHOT_BYTES {
        return false;
    }
    emulator.hard_reset();
    emulator.load_program(0, &bytes);
    emulator.load_program_extent(SNAPSHOT_BYTES);
    emulator.set_pc(snap.pc);
    for (i, &v) in snap.regs.iter().enumerate() {
        emulator.set_reg(i as u8, v);
    }
    emulator.resume();
    true
}

// ===== Minimal base64 encoder/decoder =====
//
// Inline so we don't pull in a crate for ~40 lines of code.

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(src: &[u8]) -> String {
    let mut out = String::with_capacity(src.len().div_ceil(3) * 4);
    for chunk in src.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(B64[(b0 >> 2) as usize] as char);
        out.push(B64[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn b64_decode(src: &str) -> Option<Vec<u8>> {
    let bytes = src.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let v0 = b64_val(chunk[0])?;
        let v1 = b64_val(chunk[1])?;
        let v2 = if chunk[2] == b'=' {
            0
        } else {
            b64_val(chunk[2])?
        };
        let v3 = if chunk[3] == b'=' {
            0
        } else {
            b64_val(chunk[3])?
        };
        out.push((v0 << 2) | (v1 >> 4));
        if chunk[2] != b'=' {
            out.push((v1 << 4) | (v2 >> 2));
        }
        if chunk[3] != b'=' {
            out.push((v2 << 6) | v3);
        }
    }
    Some(out)
}

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64_roundtrip() {
        let samples: &[&[u8]] = &[
            b"",
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            &[0, 255, 128, 1, 2, 3, 4],
        ];
        for s in samples {
            let enc = b64_encode(s);
            let dec = b64_decode(&enc).unwrap();
            assert_eq!(&dec[..], *s, "roundtrip failed for {:?}", s);
        }
    }

    #[test]
    fn content_key_deterministic() {
        let k = "kernel source";
        let c: &[(&str, &str)] = &[("a", "body a"), ("b", "body b")];
        assert_eq!(content_key(k, c), content_key(k, c));
    }

    #[test]
    fn content_key_changes_with_content() {
        let k = "kernel";
        let c1: &[(&str, &str)] = &[("a", "x")];
        let c2: &[(&str, &str)] = &[("a", "y")];
        assert_ne!(content_key(k, c1), content_key(k, c2));
    }
}
