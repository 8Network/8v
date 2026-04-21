// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for general CLI behavior

use std::process::Command;

/// Path to the compiled binary. Set by Cargo for integration tests.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn bin_in(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.current_dir(dir);
    cmd
}

/// Create a minimal project root so `WorkspaceRoot` resolves from CWD.
fn setup_project(tmp: &tempfile::TempDir) {
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
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

// ─── F1 regression: absolute path must not leak in read output headers ────────

/// Regression for F1: when an absolute path is passed (as MCP does after
/// resolve_mcp_paths), the rendered output header must show the relative path,
/// not the absolute path.
#[test]
fn read_absolute_path_renders_relative_in_header() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    // Create a file inside the workspace.
    std::fs::write(
        tmp.path().join("example.txt"),
        "fn hello() {}\nfn world() {}\n",
    )
    .unwrap();

    // Simulate what MCP does: pass the absolute path directly.
    let abs_path = tmp.path().join("example.txt");
    let abs_path_str = abs_path.to_str().unwrap();
    let out = bin_in(tmp.path())
        .args(["read", "--full", abs_path_str])
        .output()
        .expect("run 8v read");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The absolute path prefix of the temp dir must NOT appear in any header.
    let abs_prefix = tmp.path().to_str().unwrap();
    assert!(
        !stdout.contains(abs_prefix),
        "absolute path leaked in read output header:\n{stdout}"
    );
    // The relative filename must appear instead.
    assert!(
        stdout.contains("example.txt"),
        "relative filename missing from read output header:\n{stdout}"
    );
}

/// Regression for F1 (range variant): line-range read with an absolute path
/// must render only the relative path in its header.
#[test]
fn read_range_absolute_path_renders_relative_in_header() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("example.txt"), "line1\nline2\nline3\n").unwrap();

    let abs_path = tmp.path().join("example.txt");
    let abs_range = format!("{}:1-2", abs_path.to_str().unwrap());
    let out = bin_in(tmp.path())
        .args(["read", &abs_range])
        .output()
        .expect("run 8v read");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let abs_prefix = tmp.path().to_str().unwrap();
    assert!(
        !stdout.contains(abs_prefix),
        "absolute path leaked in range read output header:\n{stdout}"
    );
    assert!(
        stdout.contains("example.txt"),
        "relative filename missing from range read output header:\n{stdout}"
    );
}

// ─── Multi --full flag acceptance ─────────────────────────────────────────────

#[test]
fn read_double_full_flag_accepted() {
    // Clap's default SetTrue rejects `--full --full` with
    // "the argument '--full' cannot be used multiple times".
    // This test ensures the flag is accepted (exit 0) when passed twice.
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("fixture.txt"), "hello\nworld\n").unwrap();

    let abs_path = tmp.path().join("fixture.txt");
    let abs_path_str = abs_path.to_str().unwrap();
    let out = bin_in(tmp.path())
        .args(["read", "--full", "--full", abs_path_str])
        .output()
        .expect("run 8v read");

    assert_eq!(
        out.status.code(),
        Some(0),
        "read --full --full must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hello"),
        "read --full --full must output file content; got:\n{stdout}"
    );
}

#[test]
fn read_triple_full_flag_accepted_matches_single() {
    // TG-1 (review R1): triple --full must produce byte-for-byte identical
    // output to single --full. This is a byte-level diff, not a substring check.
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("fixture.txt"), "hello\nworld\n").unwrap();

    let abs_path = tmp.path().join("fixture.txt");
    let abs_path_str = abs_path.to_str().unwrap();

    let single = bin_in(tmp.path())
        .args(["read", "--full", abs_path_str])
        .output()
        .expect("run 8v read single --full");

    let triple = bin_in(tmp.path())
        .args(["read", "--full", "--full", "--full", abs_path_str])
        .output()
        .expect("run 8v read triple --full");

    assert_eq!(
        single.status.code(),
        Some(0),
        "single --full must exit 0; stderr: {}",
        String::from_utf8_lossy(&single.stderr)
    );
    assert_eq!(
        triple.status.code(),
        Some(0),
        "triple --full must exit 0; stderr: {}",
        String::from_utf8_lossy(&triple.stderr)
    );
    // Byte-level diff: stdout must be identical, not merely contain the same substrings.
    assert_eq!(
        single.stdout,
        triple.stdout,
        "stdout must be byte-for-byte identical between single and triple --full;\n\
         single ({} bytes): {}\n\
         triple ({} bytes): {}",
        single.stdout.len(),
        String::from_utf8_lossy(&single.stdout),
        triple.stdout.len(),
        String::from_utf8_lossy(&triple.stdout),
    );
}

#[test]
fn read_full_returns_all_lines() {
    // M4 gap: a mutation that truncates output to the first line (.take(1))
    // survives both `read_double_full_flag_accepted` (checks only "hello") and
    // `read_triple_full_flag_accepted_matches_single` (equality between two
    // equally-truncated outputs). This test closes that gap by asserting that
    // BOTH lines of the two-line fixture appear in the output.
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("fixture.txt"), "hello\nworld\n").unwrap();

    let abs_path = tmp.path().join("fixture.txt");
    let abs_path_str = abs_path.to_str().unwrap();
    let out = bin_in(tmp.path())
        .args(["read", "--full", abs_path_str])
        .output()
        .expect("run 8v read --full");

    assert_eq!(
        out.status.code(),
        Some(0),
        "read --full must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hello"),
        "read --full must output first line; got:\n{stdout}"
    );
    assert!(
        stdout.contains("world"),
        "read --full must output second line (catches truncation mutations); got:\n{stdout}"
    );
}
