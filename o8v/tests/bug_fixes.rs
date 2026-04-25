// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Failing-first regression tests for three confirmed bugs:
//! - SIG-1: First SIGINT must exit 130 (not continue to completion)
//! - FIFO-1: `8v read <fifo>` must not hang; must exit 1 within 2s
//! - BATCH-1: `8v read f1 f2` where all files fail must exit 1, not 0

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_8v"))
}

// ─── SIG-1: First SIGINT must exit 130 ───────────────────────────────────────

/// Send one SIGINT to a running `8v` process and assert it exits with code 130.
///
/// Pre-fix: signal handler sets the flag and prints, then falls through.
/// The process continues to completion and exits 0.
/// This test MUST FAIL before the fix.
///
/// Blocking mechanism: `8v mcp` starts an async MCP server that reads from
/// stdin in a loop. It stays alive indefinitely until stdin closes or a signal
/// arrives. We pipe stdin from the test process so it never closes, giving us
/// a reliable >200ms window to send SIGINT.
#[test]
#[cfg(unix)]
fn sig1_first_sigint_exits_130() {
    use std::os::unix::process::ExitStatusExt;
    use std::time::{Duration, Instant};

    // Spawn `8v mcp` with stdin piped (so it blocks waiting for MCP messages).
    // Signal handler is installed early in main() before the MCP serve call.
    let mut child = Command::new(bin_path())
        .arg("mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn 8v mcp");

    // Give the process time to start and install the signal handler.
    std::thread::sleep(Duration::from_millis(200));

    // Send SIGINT (signal 2) once — this is what Ctrl+C produces in a terminal.
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    // Wait up to 3 seconds for it to exit.
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None => {
                if Instant::now() > deadline {
                    child.kill().expect("kill child");
                    panic!("process did not exit after SIGINT within 3 seconds");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    assert_eq!(
        status.code(),
        Some(130),
        "first SIGINT must produce exit code 130; got {:?} (signal {:?})",
        status.code(),
        status.signal()
    );
}

// ─── FIFO-1: 8v read <fifo> must not hang ────────────────────────────────────

/// `8v read` on a FIFO (named pipe) must return an error quickly.
/// Pre-fix: canonicalize() opens the fd and blocks forever on an unread FIFO.
/// Post-fix: symlink_metadata() runs first, detects non-regular-file, returns error.
/// This test MUST time out (or hang) before the fix.
#[test]
#[cfg(unix)]
fn fifo1_read_fifo_does_not_hang() {
    use std::time::{Duration, Instant};

    let dir = TempDir::new().expect("tmpdir");
    let fifo_path = dir.path().join("test.fifo");

    unsafe {
        let path_cstr = std::ffi::CString::new(fifo_path.to_str().unwrap()).unwrap();
        let rc = libc::mkfifo(path_cstr.as_ptr(), 0o600);
        assert_eq!(rc, 0, "mkfifo failed");
    }

    let init_dir = TempDir::new().expect("init tmpdir");

    let start = Instant::now();

    // Run 8v read on the FIFO with a timeout enforced by the test process.
    // We spawn in a thread with a 2-second limit.
    let fifo_str = fifo_path.to_str().unwrap().to_string();
    let init_str = init_dir.path().to_str().unwrap().to_string();
    let bin = bin_path();

    let handle = std::thread::spawn(move || {
        Command::new(&bin)
            .args(["read", &fifo_str])
            .current_dir(&init_str)
            .output()
    });

    // 2-second deadline
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if handle.is_finished() {
            break;
        }
        if Instant::now() > deadline {
            panic!(
                "8v read on FIFO hung for >2 seconds (FIFO-1 bug not fixed); \
                 elapsed: {:?}",
                start.elapsed()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let output = handle.join().expect("thread join").expect("spawn output");

    // Must exit non-zero (it's not a regular file)
    assert_ne!(
        output.status.code(),
        Some(0),
        "8v read on FIFO must exit non-zero; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "8v read on FIFO must complete in <2s; took {:?}",
        elapsed
    );
}

// ─── BATCH-1: all-fail batch must exit 1 ─────────────────────────────────────

/// `8v read nonexistent1.rs nonexistent2.rs` — both paths fail (files don't exist).
/// Must exit 1, not 0.
/// Pre-fix: `read_to_report` returns `Ok(ReadReport::Multi { entries })` regardless;
/// if mod.rs dispatch checks `.any(Err)` and returns FAILURE, this passes already.
/// This test documents and enforces the contract.
#[test]
fn batch1_all_fail_exits_1() {
    let dir = TempDir::new().expect("tmpdir");

    // Write a tiny Rust file so 8v can init a project.
    fs::write(dir.path().join("lib.rs"), "pub fn f() {}").expect("write lib.rs");

    let out = Command::new(bin_path())
        .args(["read", "nonexistent_alpha.rs", "nonexistent_beta.rs"])
        .current_dir(dir.path())
        .output()
        .expect("spawn 8v read");

    assert_eq!(
        out.status.code(),
        Some(1),
        "batch read where all files fail must exit 1; got {:?}; stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── ORDER-1: batch read output must preserve input order ────────────────────

/// `8v read f1 f2 f3 ... f20` — sections in output must appear in the same
/// order as the paths on the command line.
/// The alleged bug: output sections are non-deterministically reordered.
/// This test creates 20 files with a deliberately shuffled name list, runs
/// `8v read` with that exact order, parses `=== <path> ===` headers, and
/// asserts order matches input. Repeated 5× to catch non-determinism.
#[test]
fn order1_batch_read_preserves_input_order() {
    use std::fs;

    // Shuffled order (not alphabetical, not insertion order) — 20 files.
    let names = [
        "file13.rs",
        "file07.rs",
        "file01.rs",
        "file19.rs",
        "file04.rs",
        "file11.rs",
        "file16.rs",
        "file02.rs",
        "file08.rs",
        "file20.rs",
        "file05.rs",
        "file14.rs",
        "file09.rs",
        "file17.rs",
        "file03.rs",
        "file18.rs",
        "file06.rs",
        "file10.rs",
        "file15.rs",
        "file12.rs",
    ];

    let dir = tempfile::TempDir::new().expect("tmpdir");
    for name in &names {
        let path = dir.path().join(name);
        fs::write(&path, format!("pub fn f_{name}() {{}}\n")).expect("write");
    }

    let full_paths: Vec<String> = names
        .iter()
        .map(|n| dir.path().join(n).to_str().unwrap().to_string())
        .collect();

    // Run 5× to catch non-determinism.
    for round in 0..5 {
        let mut cmd = std::process::Command::new(bin_path());
        cmd.arg("read");
        for p in &full_paths {
            cmd.arg(p);
        }
        cmd.current_dir(dir.path());
        let out = cmd.output().expect("spawn 8v read");
        let stdout = String::from_utf8_lossy(&out.stdout);

        // Parse `=== <path> ===` headers in output order.
        let observed: Vec<&str> = stdout
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("===") && trimmed.ends_with("===") {
                    // strip leading/trailing `===` and whitespace
                    let inner = trimmed.trim_start_matches('=').trim_end_matches('=').trim();
                    Some(inner)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            observed.len(),
            names.len(),
            "round {round}: expected {} sections, got {}; stdout:\n{stdout}",
            names.len(),
            observed.len()
        );

        for (i, (obs, exp_path)) in observed.iter().zip(full_paths.iter()).enumerate() {
            assert!(
                obs.contains(names[i]),
                "round {round}: section {i} expected to contain '{}' but got '{}'; stdout:\n{stdout}",
                names[i],
                obs
            );
            let _ = exp_path; // used via names[i]
        }
    }
}
