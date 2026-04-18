// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end render tests for `8v init` — verifies the typed render pipeline.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// `8v init --yes --json` must return valid JSON with a `success` field.
#[test]
fn init_json_has_success_field() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes", "--json"])
        .output()
        .expect("run 8v init --yes --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!(
            "stdout must be valid JSON: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    };

    assert!(
        json.get("success").is_some(),
        "`success` field must be present in JSON output\njson: {json}"
    );
    assert_eq!(
        json["success"], true,
        "`success` must be true on a successful init\njson: {json}"
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "exit code must be 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `8v init --yes` (plain, default) must not emit JSON.
#[test]
fn init_plain_is_not_json() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Plain output must not be a JSON object
    assert!(
        serde_json::from_str::<serde_json::Value>(stdout.trim()).is_err()
            || !stdout.trim().starts_with('{'),
        "plain output must not be a JSON object\nstdout: {stdout}"
    );
}
