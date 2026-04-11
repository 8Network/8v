// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for general CLI behavior

use std::process::Command;

/// Path to the compiled binary. Set by Cargo for integration tests.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── Help and version ──────────────────────────────────────────────────────

#[test]
fn help_exits_0() {
    let out = bin()
        .args(["check", "--help"])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Check a project directory"));
}

#[test]
fn version_exits_0() {
    let out = bin()
        .arg("--version")
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("8v"));
}

#[test]
fn no_subcommand_shows_help() {
    let out = bin().output().expect("failed to run binary");

    // clap exits non-zero when no subcommand given
    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Usage") || stderr.contains("check"),
        "expected usage info, got: {stderr}"
    );
}
