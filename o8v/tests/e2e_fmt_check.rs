// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v fmt --check` exit code contract.
//!
//! Contract: exit non-zero when files need formatting, exit 0 when already formatted.

use o8v_testkit::TempProject;
use std::process::Command;

fn init_workspace(dir: &std::path::Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["init", "--yes"])
        .current_dir(dir)
        .status()
        .expect("8v init --yes");
    assert!(
        status.success(),
        "8v init --yes failed in {}",
        dir.display()
    );
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

/// `8v fmt --check` must exit non-zero when a Rust file is unformatted.
///
/// Regression guard: before fix, exit code was 0 on dirty.
#[test]
fn fmt_check_exits_nonzero_on_dirty_rust() {
    let project = fixture("fmt-check-rust");
    init_workspace(project.path());

    let out = bin()
        .args(["fmt", "--check", "."])
        .current_dir(project.path())
        .output()
        .expect("run 8v fmt --check");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v fmt --check must exit non-zero when files need formatting\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// `8v fmt --check` must exit 0 when all files are already formatted.
#[test]
fn fmt_check_exits_zero_on_clean_rust() {
    let project = fixture("fmt-check-rust");
    init_workspace(project.path());

    // First format the project in-place.
    let fmt_out = Command::new("cargo")
        .args(["fmt", "--all"])
        .current_dir(project.path())
        .output()
        .expect("cargo fmt");
    assert!(
        fmt_out.status.success(),
        "cargo fmt must succeed: {}",
        String::from_utf8_lossy(&fmt_out.stderr)
    );

    // Now --check should pass.
    let out = bin()
        .args(["fmt", "--check", "."])
        .current_dir(project.path())
        .output()
        .expect("run 8v fmt --check on clean project");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "8v fmt --check must exit 0 when all files are formatted\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// BUG F2: `8v fmt` on a directory with no recognized stacks must exit 0.
/// "Nothing to format" is a success case, not an error.
/// Failing-first: was exit 1 before fix.
#[test]
fn fmt_no_stacks_exits_zero() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Initialize a workspace so 8v doesn't complain about missing init.
    let init_out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v init");
    assert!(init_out.status.success(), "init must succeed");

    let out = bin()
        .args(["fmt", "."])
        .current_dir(dir.path())
        .output()
        .expect("run 8v fmt on dir with no stacks");

    assert!(
        out.status.success(),
        "fmt with no stacks must exit 0 (BUG F2)
stdout: {}
stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// BUG F2 (JSON): `8v fmt --json` on a directory with no stacks must include
/// `"reason":"no_stacks"` in the output.
#[test]
fn fmt_no_stacks_json_has_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let init_out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v init");
    assert!(init_out.status.success(), "init must succeed");

    let out = bin()
        .args(["fmt", ".", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v fmt --json on dir with no stacks");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON\ngot: {stdout}\nerr: {e}"),
    };

    assert_eq!(
        v["reason"].as_str(),
        Some("no_stacks"),
        "fmt --json with no stacks must have reason=no_stacks (BUG F2)
got: {v}"
    );
}
