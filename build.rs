use cor24_emulator::{Assembler, EmulatorCore, StopReason};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// Paths to the forth-in-forth sources. Both build.rs and runtime
// (src/repl.rs) read these — keep the relative paths identical.
const KERNEL_PATH: &str = "../sw-cor24-forth/forth-in-forth/kernel.s";
const CORE_FILES: &[(&str, &str)] = &[
    (
        "minimal",
        "../sw-cor24-forth/forth-in-forth/core/minimal.fth",
    ),
    (
        "lowlevel",
        "../sw-cor24-forth/forth-in-forth/core/lowlevel.fth",
    ),
    (
        "midlevel",
        "../sw-cor24-forth/forth-in-forth/core/midlevel.fth",
    ),
    (
        "highlevel",
        "../sw-cor24-forth/forth-in-forth/core/highlevel.fth",
    ),
];

/// Low-memory bytes captured in the snapshot. Must match `SNAPSHOT_BYTES`
/// in src/snapshot.rs.
const SNAPSHOT_BYTES: u32 = 64 * 1024;

/// UART poll labels used to detect "idle at KEY". Must match src/repl.rs.
const POLL_LABELS: &[&str] = &[
    "key_poll",
    "word_skip_rx",
    "word_skip_rx2",
    "word_read_rx",
    "word_read_rx2",
    "create_skip_rx",
    "create_skip_rx2",
    "create_read_rx",
    "create_read_rx2",
];

fn main() {
    println!("cargo:rerun-if-changed={KERNEL_PATH}");
    for (_, p) in CORE_FILES {
        println!("cargo:rerun-if-changed={p}");
    }
    // forth-on-forthish inputs (consumed via include_str! in src/demos.rs,
    // not by the snapshot path below). Declared here so edits in that tier
    // rerun build.rs and refresh BUILD_SHA / BUILD_TIMESTAMP.
    println!("cargo:rerun-if-changed=../sw-cor24-forth/forth-on-forthish/kernel.s");
    for name in ["runtime", "minimal", "lowlevel", "midlevel", "highlevel"] {
        println!("cargo:rerun-if-changed=../sw-cor24-forth/forth-on-forthish/core/{name}.fth");
    }
    // Rerun on every commit so BUILD_SHA and BUILD_TIMESTAMP in the footer
    // stay in sync with the checked-out commit. `.git/HEAD` changes on
    // commit, branch switch, and detached-HEAD moves.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=build.rs");

    emit_build_env_vars();
    emit_snapshot_blob();
}

fn emit_build_env_vars() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let host = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let timestamp = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=BUILD_SHA={sha}");
    println!("cargo:rustc-env=BUILD_HOST={host}");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={timestamp}");
}

fn emit_snapshot_blob() {
    let out_dir: PathBuf = std::env::var_os("OUT_DIR").expect("OUT_DIR not set").into();
    let blob_path = out_dir.join("fif_snapshot.bin");

    // Best-effort: if anything fails we emit a 0-byte placeholder so
    // include_bytes! still succeeds; the runtime falls back to cold boot.
    match try_build_snapshot() {
        Ok((blob, instrs)) => {
            fs::write(&blob_path, &blob).expect("write snapshot blob");
            println!(
                "cargo:warning=fif-snapshot: {} bytes, {} instructions",
                blob.len(),
                instrs
            );
        }
        Err(e) => {
            println!("cargo:warning=fif-snapshot: build failed ({e}); shipping empty blob");
            fs::write(&blob_path, []).expect("write empty snapshot blob");
        }
    }
}

fn try_build_snapshot() -> Result<(Vec<u8>, u64), String> {
    let kernel_src = fs::read_to_string(KERNEL_PATH).map_err(|e| format!("read kernel: {e}"))?;

    // Surface which kernel variant we're shipping so we can't silently
    // regress to the pre-hashed-FIND version without noticing.
    let hashed = kernel_src.contains("dict_hash_table") && kernel_src.contains("do_find:");
    println!(
        "cargo:warning=fif-kernel: hashed-FIND markers {}",
        if hashed { "present" } else { "ABSENT" }
    );

    let mut core_contents: Vec<(&str, String)> = Vec::new();
    for (name, path) in CORE_FILES {
        let src = fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
        core_contents.push((name, src));
    }

    // Assemble the kernel.
    let mut asm = Assembler::new();
    let result = asm.assemble(&kernel_src);
    if !result.errors.is_empty() {
        return Err(format!("kernel assembly errors: {:?}", result.errors));
    }
    let labels = result.labels.clone();
    let poll_addrs: Vec<u32> = POLL_LABELS
        .iter()
        .filter_map(|name| labels.get(*name).copied())
        .collect();
    if poll_addrs.is_empty() {
        return Err("no UART poll labels found in assembled kernel".into());
    }

    // Build the UART RX queue: core files LF-separated, CR-stripped, one
    // trailing LF. Matches src/repl.rs's cold-bootstrap logic exactly.
    let mut uart_rx: Vec<u8> = Vec::new();
    for (i, (_, contents)) in core_contents.iter().enumerate() {
        if i > 0 && uart_rx.last().is_none_or(|&b| b != b'\n') {
            uart_rx.push(b'\n');
        }
        for b in contents.bytes() {
            if b == b'\r' {
                continue;
            }
            uart_rx.push(b);
        }
        if uart_rx.last().is_none_or(|&b| b != b'\n') {
            uart_rx.push(b'\n');
        }
    }
    let mut queue: std::collections::VecDeque<u8> = uart_rx.into_iter().collect();

    // Run the kernel.
    let mut emu = EmulatorCore::new();
    emu.load_program(0, &result.bytes);
    emu.load_program_extent(result.bytes.len() as u32);
    emu.set_pc(0);
    emu.resume();

    const SUB_BATCH: u64 = 200_000;
    const MAX_INSTRUCTIONS: u64 = 2_000_000_000;
    let mut total_instructions: u64 = 0;

    let is_idle = |pc: u32| poll_addrs.iter().any(|&addr| pc >= addr && pc < addr + 16);
    // `clear_output = true` discards the kernel's echo (unimportant during
    // bootstrap where we'd accumulate tens of KB of "ok" lines). Verify
    // phase turns it off so we can actually see what the kernel produced.
    let run_one = |emu: &mut EmulatorCore,
                   total: &mut u64,
                   clear_output: bool|
     -> Result<(), String> {
        let r = emu.run_batch(SUB_BATCH);
        *total += r.instructions_run;
        if *total > MAX_INSTRUCTIONS {
            return Err(format!(
                "bootstrap exceeded {MAX_INSTRUCTIONS} instructions"
            ));
        }
        match r.reason {
            StopReason::Halted => return Err("kernel halted during bootstrap".into()),
            StopReason::InvalidInstruction(b) => {
                return Err(format!("invalid instruction 0x{b:02X}"));
            }
            StopReason::StackOverflow(sp) => return Err(format!("stack overflow sp={sp:06X}")),
            StopReason::StackUnderflow(sp) => return Err(format!("stack underflow sp={sp:06X}")),
            _ => {}
        }
        if clear_output {
            emu.clear_uart_output();
        }
        Ok(())
    };

    // Phase 1: drain the entire input queue. Transient PC-at-key_poll
    // states during `\` comment scanning can *not* end this phase — we
    // only stop when every byte has been handed to the UART.
    while !queue.is_empty() {
        let status = emu.read_byte(0xFF0101);
        if status & 0x01 == 0
            && let Some(b) = queue.pop_front()
        {
            emu.send_uart_byte(b);
        }
        run_one(&mut emu, &mut total_instructions, true)?;
    }

    // Phase 2: settle. Keep running until the kernel has digested the last
    // line and is parked at a poll address. Require the idle condition to
    // hold across several consecutive sub-batches before accepting it — a
    // single-hit "PC at poll" could be a transient comment-scanner wait
    // for the next char, not true end-of-input.
    const MIN_STABLE_HITS: u32 = 4;
    const MAX_SETTLE_INSTRUCTIONS: u64 = 50_000_000;
    let mut stable_hits: u32 = 0;
    let settle_start = total_instructions;
    loop {
        run_one(&mut emu, &mut total_instructions, true)?;
        let pc = emu.snapshot().pc;
        if is_idle(pc) {
            stable_hits += 1;
            if stable_hits >= MIN_STABLE_HITS {
                break;
            }
        } else {
            stable_hits = 0;
        }
        if total_instructions - settle_start > MAX_SETTLE_INSTRUCTIONS {
            return Err(format!(
                "bootstrap did not settle within {MAX_SETTLE_INSTRUCTIONS} instructions after queue drain"
            ));
        }
    }

    // Phase 3: verify. Send a trivial expression the kernel should be able
    // to evaluate and check the UART output. If this fails, the dictionary
    // wasn't fully built and the snapshot is garbage — better to ship no
    // blob than a broken one.
    emu.clear_uart_output(); // start from a clean slate
    for b in b"1 2 + .\n" {
        // Wait for UART clear, then push one byte.
        loop {
            let status = emu.read_byte(0xFF0101);
            if status & 0x01 == 0 {
                emu.send_uart_byte(*b);
                break;
            }
            run_one(&mut emu, &mut total_instructions, false)?;
        }
        run_one(&mut emu, &mut total_instructions, false)?;
    }
    // Let the kernel process and emit. Give it enough budget for a full
    // line parse + execute + "ok" echo cycle.
    let mut verify_budget: u64 = 10_000_000;
    while verify_budget > 0 {
        let before = total_instructions;
        run_one(&mut emu, &mut total_instructions, false)?;
        verify_budget = verify_budget.saturating_sub(total_instructions - before);
        if emu.get_uart_output().contains('3') && is_idle(emu.snapshot().pc) {
            break;
        }
    }
    let uart_out: String = emu.get_uart_output().chars().collect();
    emu.clear_uart_output();
    if !uart_out.contains('3') {
        return Err(format!(
            "post-bootstrap sanity check failed: `1 2 + .` emitted {uart_out:?}"
        ));
    }

    // We've now dirtied the snapshot with a "1 2 + ." evaluation. Settle
    // again so PC is back at key_poll with a clean data stack.
    emu.clear_uart_output();
    let mut stable_hits: u32 = 0;
    let resettle_start = total_instructions;
    loop {
        run_one(&mut emu, &mut total_instructions, true)?;
        let pc = emu.snapshot().pc;
        if is_idle(pc) {
            stable_hits += 1;
            if stable_hits >= MIN_STABLE_HITS {
                break;
            }
        } else {
            stable_hits = 0;
        }
        if total_instructions - resettle_start > MAX_SETTLE_INSTRUCTIONS {
            return Err("post-verify resettle timeout".into());
        }
    }

    // Capture the snapshot.
    let memory = emu.read_memory(0, SNAPSHOT_BYTES);
    let snap = emu.snapshot();
    let content_hash = djb2(&kernel_src, &core_contents);

    // Binary format (all little-endian):
    //   0..4   magic "FIF1"
    //   4..12  content_hash u64
    //  12..16  memory_len u32
    //  16..N   memory bytes
    //  N..N+32 regs: 8 * u32
    //    +4    pc u32
    //    +1    c flag u8
    //    +8    cycles u64
    //    +8    instructions u64
    let mut blob: Vec<u8> = Vec::with_capacity(memory.len() + 64);
    blob.extend_from_slice(b"FIF1");
    blob.extend_from_slice(&content_hash.to_le_bytes());
    blob.extend_from_slice(&(memory.len() as u32).to_le_bytes());
    blob.extend_from_slice(&memory);
    for &r in &snap.regs {
        blob.extend_from_slice(&r.to_le_bytes());
    }
    blob.extend_from_slice(&snap.pc.to_le_bytes());
    blob.push(snap.c as u8);
    blob.extend_from_slice(&snap.cycles.to_le_bytes());
    blob.extend_from_slice(&snap.instructions.to_le_bytes());
    Ok((blob, total_instructions))
}

/// djb2 hash — must match `snapshot::content_key`'s algorithm so the
/// embedded blob's hash matches what the runtime computes from the
/// include_str!'d sources.
fn djb2(kernel_src: &str, core_files: &[(&str, String)]) -> u64 {
    let mut h: u64 = 5381;
    for b in kernel_src.as_bytes() {
        h = h.wrapping_mul(33).wrapping_add(*b as u64);
    }
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
