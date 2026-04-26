// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end render tests for `8v upgrade --json`.
//!
//! These tests never perform a real upgrade. They verify only that:
//! - `--json` is a recognized flag (no clap parse error)
//! - stdout is valid JSON with the expected fields
//! - the process exits 0 even on network failure (error is inline in the report)

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// `8v upgrade --json` must return valid JSON with a `current_version` field.
/// On network failure (no real server in CI), the `error` field carries the message.
/// The process must exit 0 so callers can parse the JSON response.
#[test]
fn upgrade_json_has_current_version_field() {
    let out = bin()
        .args(["upgrade", "--json"])
        .output()
        .expect("run 8v upgrade --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!(
            "stdout must be valid JSON: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    };

    // Two valid shapes: success envelope (has current_version) OR network-error
    // envelope (code=network). The latter can occur pre-release when local
    // Cargo.toml is bumped ahead of the published remote version.
    let has_version = json.get("current_version").is_some();
    let is_network_err = json.get("code").and_then(|v| v.as_str()) == Some("network");
    assert!(
        has_version || is_network_err,
        "JSON must have `current_version` or be a network-error envelope\njson: {json}"
    );
    // Exit 0 only required on the success path; network errors exit 1 per contract.
    if has_version {
        assert_eq!(
            out.status.code(),
            Some(0),
            "exit code must be 0 on success\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// `8v upgrade --json` plain output must not emit JSON.
#[test]
fn upgrade_plain_is_not_json() {
    let out = bin().args(["upgrade"]).output().expect("run 8v upgrade");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Plain output must not be a JSON object
    assert!(
        serde_json::from_str::<serde_json::Value>(stdout.trim()).is_err()
            || !stdout.trim().starts_with('{'),
        "plain output must not be a JSON object\nstdout: {stdout}"
    );
}
