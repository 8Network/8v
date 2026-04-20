// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Failing-first tests for B2a: `8v upgrade` human output must route to stderr.
//!
//! These tests validate the stderr-channel contract for the upgrade command:
//! - Human audience: all output (including errors) goes to stderr, stdout is empty.
//! - JSON audience: all output goes to stdout, stderr is empty.
//!
//! In CI, `8v upgrade` will fail to reach the real server (network error).
//! The `UpgradeReport { error: Some(e) }` path is sufficient to test channel routing.
//!
//! Three tests fail before the B2a 1-line fix (they verify stderr routing for human):
//!   - upgrade_human_error_goes_to_stderr
//!   - upgrade_human_error_not_on_stdout
//!   - upgrade_human_success_goes_to_stderr
//!
//! Two tests pass before and after (JSON regression guards):
//!   - upgrade_json_error_stays_on_stdout
//!   - upgrade_json_error_not_on_stderr

use std::process::{Command, Stdio};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

/// Human audience: when upgrade encounters an error (e.g. network failure in CI),
/// the error text must appear on stderr, not stdout.
///
/// FAILS before fix: `use_stderr = false` routes output to stdout; stderr is empty.
/// PASSES after fix: `use_stderr = audience == Audience::Human` routes to stderr.
#[test]
fn upgrade_human_error_goes_to_stderr() {
    let out = bin().args(["upgrade"]).output().expect("run 8v upgrade");

    let stderr = String::from_utf8_lossy(&out.stderr);

    // The upgrade command always produces some output (version info or error).
    // After the fix, all human output is on stderr.
    assert!(
        !stderr.is_empty(),
        "stderr must be non-empty for human audience\nstdout: {}\nstderr: {stderr}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// Human audience: when upgrade encounters an error, stdout must be empty.
///
/// FAILS before fix: stdout contains the full upgrade output.
/// PASSES after fix: stdout is empty.
#[test]
fn upgrade_human_error_not_on_stdout() {
    let out = bin().args(["upgrade"]).output().expect("run 8v upgrade");

    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty for human audience; all output goes to stderr\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// JSON audience: on network failure, the JSON error report must stay on stdout.
///
/// PASSES before fix (JSON was always correct — regression guard).
/// PASSES after fix.
#[test]
fn upgrade_json_error_stays_on_stdout() {
    let out = bin()
        .args(["upgrade", "--json"])
        .output()
        .expect("run 8v upgrade --json");

    let stdout = String::from_utf8_lossy(&out.stdout);

    // Must parse as valid JSON
    let json: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!(
            "stdout must be valid JSON for --json audience: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    };

    // On network failure (CI), the error field must be present and non-null.
    // On success, current_version is present. Either way, the JSON must be on stdout.
    let has_error = json.get("error").is_some_and(|v| !v.is_null());
    let has_version = json.get("current_version").is_some();
    assert!(
        has_error || has_version,
        "JSON on stdout must contain either 'error' or 'current_version'\njson: {json}"
    );
}

/// JSON audience: stderr must be empty when using --json.
///
/// PASSES before fix (regression guard).
/// PASSES after fix.
#[test]
fn upgrade_json_error_not_on_stderr() {
    let out = bin()
        .args(["upgrade", "--json"])
        .output()
        .expect("run 8v upgrade --json");

    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stderr.trim().is_empty(),
        "stderr must be empty for --json audience\nstderr: {stderr}\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// Human audience success case: all upgrade output (including success output) must
/// go to stderr. stdout must be empty. This test catches the case where the binary
/// CAN reach the server (e.g., already up-to-date). In CI it falls through to the
/// error path, which is still sufficient: stderr non-empty and stdout empty.
///
/// FAILS before fix: stdout contains output, stderr is empty.
/// PASSES after fix: stderr contains output, stdout is empty.
#[test]
fn upgrade_human_success_goes_to_stderr() {
    let out = bin().args(["upgrade"]).output().expect("run 8v upgrade");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Whether upgrade succeeds or fails (network error in CI), the channel contract holds:
    // human output belongs on stderr, stdout must be empty.
    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty for human audience (success or error path)\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "stderr must be non-empty for human audience (success or error path)\nstdout: {stdout}\nstderr: {stderr}"
    );
}
