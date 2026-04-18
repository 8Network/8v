// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v fmt --check` exit code contract.
//!
//! Contract: exit non-zero when files need formatting, exit 0 when already formatted.

use o8v_testkit::TempProject;
use std::process::Command;

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

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
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
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on clean project");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "8v fmt --check must exit 0 when all files are formatted\nstdout: {stdout}\nstderr: {stderr}"
    );
}
