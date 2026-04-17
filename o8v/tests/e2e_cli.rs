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
fn long_version_includes_provenance_fields() {
    // `--version` must print commit, commit_date, branch, describe, built,
    // profile, target, rustc, binary_path — the fields that answer
    // "what binary is this and how was it built?".
    let out = bin()
        .arg("--version")
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    for label in [
        "commit:",
        "commit_date:",
        "branch:",
        "describe:",
        "built:",
        "profile:",
        "target:",
        "rustc:",
        "binary_path:",
    ] {
        assert!(
            stdout.contains(label),
            "--version output missing {label:?}; got:\n{stdout}"
        );
    }
    // Sanity: the private/host identity field we removed must not come back.
    assert!(
        !stdout.contains("built_by"),
        "--version must not include built_by (user@host), got:\n{stdout}"
    );
}

#[test]
fn short_version_is_short_only() {
    // `-V` is the short form — just `8v <version>`, no provenance block.
    let out = bin().arg("-V").output().expect("failed to run binary");

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("8v "),
        "unexpected short version: {stdout}"
    );
    // Short form must not carry the long provenance fields.
    for label in ["commit:", "built:", "binary_path:"] {
        assert!(
            !stdout.contains(label),
            "-V leaked long field {label:?}; got: {stdout}"
        );
    }
}

// ─── Batch-mode exit-code hygiene ──────────────────────────────────────────

#[test]
fn read_batch_all_errors_exits_nonzero() {
    // Bug #20: in batch mode (>1 path), errors are rendered inline in the
    // Multi report but the dispatch arm hard-codes ExitCode::SUCCESS. An
    // agent has no way to tell the command failed. Rule: ANY per-file error
    // in batch mode must yield a non-zero exit so failure is visible.
    let out = bin()
        .args([
            "read",
            "/nonexistent/a.rs",
            "/nonexistent/b.rs",
            "/nonexistent/c.rs",
        ])
        .output()
        .expect("failed to run binary");

    assert_ne!(
        out.status.code(),
        Some(0),
        "batch read with all-errors must exit non-zero; got exit=0. \
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn read_batch_partial_errors_exits_nonzero() {
    // If one path errors and others succeed, exit must still be non-zero —
    // partial success silently discards the error otherwise.
    let out = bin()
        .args(["read", "Cargo.toml", "/nonexistent/missing.rs"])
        .output()
        .expect("failed to run binary");

    assert_ne!(
        out.status.code(),
        Some(0),
        "batch read with any error must exit non-zero; got exit=0. \
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
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
