// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `--errors-first` as a bare boolean flag on `8v test` and `8v build`.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    o8v_testkit::fixture_path("o8v", name)
}

/// `8v test --errors-first` (bare, no value) must be accepted — not produce "requires a value".
#[test]
fn test_errors_first_bare_flag_is_accepted() {
    let path = fixture_path("test-rust");

    let out = bin()
        .args(["test", path.to_str().unwrap(), "--errors-first"])
        .output()
        .expect("run 8v test --errors-first");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("requires a value"),
        "`--errors-first` must not require a value\nstderr: {stderr}"
    );
    // The flag itself must be parseable (exit code is about the project, not the flag).
    assert!(
        !stderr.contains("error: unexpected argument"),
        "`--errors-first` must be a valid flag\nstderr: {stderr}"
    );
}

/// `8v build --errors-first` (bare) must be accepted.
#[test]
fn build_errors_first_bare_flag_is_accepted() {
    let path = fixture_path("build-rust");

    let out = bin()
        .args(["build", path.to_str().unwrap(), "--errors-first"])
        .output()
        .expect("run 8v build --errors-first");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("requires a value"),
        "`--errors-first` must not require a value\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("error: unexpected argument"),
        "`--errors-first` must be a valid flag\nstderr: {stderr}"
    );
}

/// `--no-errors-first` must be accepted and disable errors-first mode.
#[test]
fn test_no_errors_first_flag_is_accepted() {
    let path = fixture_path("test-rust");

    let out = bin()
        .args(["test", path.to_str().unwrap(), "--no-errors-first"])
        .output()
        .expect("run 8v test --no-errors-first");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("requires a value"),
        "`--no-errors-first` must not require a value\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("error: unexpected argument"),
        "`--no-errors-first` must be a valid flag\nstderr: {stderr}"
    );
}
