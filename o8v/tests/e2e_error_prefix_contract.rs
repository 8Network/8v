// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E contract tests for the error-prefix normalization in `main.rs`.
//!
//! Before the fix, commands internally formatted errors as `"Error: ..."` or
//! `"8v: ..."`, then `main.rs` prepended `"error: "`, producing double prefixes
//! like `"error: Error: ..."` or `"error: 8v: ..."`.
//!
//! After the fix, `main.rs` strips one leading `"error: "`, `"Error: "`, or
//! `"8v: "` before emitting the outer `"error: "`, so every fatal error is
//! exactly `"error: <message>"` with no double prefix.
//!
//! Covered commands:
//! - `read`  — strips `"8v: "` (e.g. not-found)
//! - `write` — strips `"Error: "` (e.g. failed-to-read-file)
//! - `write` — strips `"Error: "` (e.g. line-does-not-exist)

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

fn init_temp_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = bin()
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v init --yes");
    assert!(
        out.status.success(),
        "8v init must succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

/// `read` on a nonexistent path must emit exactly `"error: not found: ..."`.
/// Without the fix it emits `"error: 8v: not found: ..."`.
#[test]
fn read_nonexistent_emits_single_prefix() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["read", "/nonexistent/__8v_test_file__.rs"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read");

    assert_eq!(
        out.status.code(),
        Some(1),
        "read on missing file must exit 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);

    // Must start with single "error: " — not double-prefix like "error: 8v: "
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with 'error: '; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: 8v:"),
        "stderr must not contain double prefix 'error: 8v:'; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain double prefix 'error: error:'; got: {stderr:?}"
    );
}

/// `write` on a nonexistent file must emit exactly `"error: failed to read file: ..."`.
/// Without the fix it emits `"error: Error: failed to read file: ..."`.
#[test]
fn write_nonexistent_emits_single_prefix() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["write", "/nonexistent/__8v_test_file__.rs:1", "hello"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v write");

    assert_eq!(
        out.status.code(),
        Some(1),
        "write on missing file must exit 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);

    // Must start with single "error: " — not double-prefix like "error: Error: "
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with 'error: '; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: Error:"),
        "stderr must not contain double prefix 'error: Error:'; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain double prefix 'error: error:'; got: {stderr:?}"
    );
}

/// `write` with a line number beyond EOF must emit `"error: line N does not exist ..."`.
/// Without the fix it emits `"error: Error: line N does not exist ..."`.
#[test]
fn write_bad_line_number_emits_single_prefix() {
    let dir = init_temp_workspace();

    // Create a real 1-line file in the workspace.
    let target = dir.path().join("single_line.txt");
    {
        let mut f = std::fs::File::create(&target).expect("create single_line.txt");
        writeln!(f, "only one line").unwrap();
    }

    // Request line 999 — well beyond EOF.
    let out = bin()
        .args(["write", "single_line.txt:999", "replacement"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v write");

    assert_eq!(
        out.status.code(),
        Some(1),
        "write beyond EOF must exit 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);

    // Must start with single "error: " — not double-prefix like "error: Error: "
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with 'error: '; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: Error:"),
        "stderr must not contain double prefix 'error: Error:'; got: {stderr:?}"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain double prefix 'error: error:'; got: {stderr:?}"
    );
    // Also confirm the message identifies the line problem.
    assert!(
        stderr.contains("line") || stderr.contains("999"),
        "stderr must mention line number; got: {stderr:?}"
    );
}
