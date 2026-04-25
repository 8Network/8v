// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E tests: `8v read file:0-0`, `file:0-5`, `file:5-0` must exit non-zero
//! and emit an error message describing why the range is invalid.
//!
//! BUG-1: `start == 0` was silently clamped to 1, producing an inverted Range
//! JSON with `start:1, end:0` and exit 0.

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
    assert!(out.status.success(), "8v init failed: {:?}", out);
    dir
}

/// `8v read file.txt:0-0` must exit non-zero with an error about start being 0.
#[test]
fn read_range_zero_zero_is_error() {
    let dir = init_temp_workspace();
    std::fs::write(
        dir.path().join("file.txt"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    let out = bin()
        .args(["read", "file.txt:0-0"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read file.txt:0-0");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_ne!(
        out.status.code(),
        Some(0),
        "expected non-zero exit for :0-0\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("0") || stderr.contains("invalid range") || stderr.contains("start"),
        "expected error message about invalid range in stderr\nstderr: {stderr}"
    );
}

/// `8v read file.txt:0-5` must exit non-zero — start 0 is invalid.
#[test]
fn read_range_zero_start_is_error() {
    let dir = init_temp_workspace();
    std::fs::write(
        dir.path().join("file.txt"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    let out = bin()
        .args(["read", "file.txt:0-5"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read file.txt:0-5");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_ne!(
        out.status.code(),
        Some(0),
        "expected non-zero exit for :0-5\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("0") || stderr.contains("invalid range") || stderr.contains("start"),
        "expected error message about invalid range in stderr\nstderr: {stderr}"
    );
}

/// `8v read file.txt:5-0` must exit non-zero — end < start.
#[test]
fn read_range_inverted_is_error() {
    let dir = init_temp_workspace();
    std::fs::write(
        dir.path().join("file.txt"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    let out = bin()
        .args(["read", "file.txt:5-0"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read file.txt:5-0");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_ne!(
        out.status.code(),
        Some(0),
        "expected non-zero exit for :5-0\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("5") || stderr.contains("invalid range") || stderr.contains("start"),
        "expected error message about invalid range in stderr\nstderr: {stderr}"
    );
}
