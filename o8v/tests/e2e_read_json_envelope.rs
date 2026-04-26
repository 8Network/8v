// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E test: multi-batch read --json error entry matches canonical envelope shape.
//!
//! Before fix: `{"status":"Err","message":"..."}`
//! After fix:  `{"code":"...","error":"..."}`

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

#[test]
fn multi_batch_error_entry_uses_canonical_envelope() {
    let dir = init_temp_workspace();
    // Write a valid file so we get one Ok + one Err entry.
    std::fs::write(dir.path().join("hello.txt"), "hello\n").unwrap();

    let out = bin()
        .args(["read", "hello.txt", "/nonexistent_xyz_abc", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\nstdout: {stdout}"),
    };

    let entries = v["Multi"]["entries"]
        .as_array()
        .expect("Multi.entries must be an array");

    let err_entry = entries
        .iter()
        .find(|e| e["label"].as_str().unwrap_or("").contains("nonexistent"))
        .expect("no entry for /nonexistent_xyz_abc");

    let result = &err_entry["result"];

    // Must have canonical envelope fields.
    assert!(
        result.get("code").is_some(),
        "missing 'code' field in result: {result}"
    );
    assert!(
        result.get("error").is_some(),
        "missing 'error' field in result: {result}"
    );

    // Must NOT have the old shape.
    assert!(
        result.get("status").is_none(),
        "unexpected 'status' field in result: {result}"
    );
    assert!(
        result.get("message").is_none(),
        "unexpected 'message' field in result: {result}"
    );
}
