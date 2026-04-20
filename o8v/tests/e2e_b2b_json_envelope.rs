// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E tests for slice B2b: canonical JSON error envelope across commands.
//!
//! Contract verified here (§2.4 + §7 of docs/design/error-contract.md):
//!   - Shape: `{"error":"<message>","code":"<machine-key>"}`
//!   - Emitted to stdout (stderr empty), exit 1 on failure with `--json`.
//!   - No `"error_kind"` field — that was the old wrong shape (deleted).
//!
//! Failing-first methodology: these tests were written before implementation
//! and verified to FAIL on pre-fix code, then PASS after the fix.

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
    assert!(
        out.status.success(),
        "8v init must succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

/// `8v write` to a nonexistent path with `--json` must exit 1 and emit
/// a canonical envelope with both `"error"` and `"code"` fields.
#[test]
fn write_json_error_envelope_has_error_and_code() {
    let dir = init_temp_workspace();
    let out = bin()
        .args([
            "write",
            "/nonexistent_path_b2b/file.txt:1",
            "hello",
            "--json",
        ])
        .current_dir(dir.path())
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "write to nonexistent path must exit non-zero\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(val) => val,
        Err(_) => panic!("stdout must be valid JSON\ngot: {stdout}"),
    };

    assert!(
        v.get("error").is_some(),
        "envelope must have 'error' field\ngot: {v}"
    );
    assert!(
        v.get("code").is_some(),
        "envelope must have 'code' field\ngot: {v}"
    );
    assert!(
        v.get("error_kind").is_none(),
        "envelope must NOT have 'error_kind' (old wrong shape)\ngot: {v}"
    );
    // code must be a non-empty string (one of the approved codes)
    let code = v["code"].as_str().expect("'code' must be a string");
    assert!(!code.is_empty(), "'code' must not be empty");
}

/// `8v search` with an invalid regex and `--json` must exit 1 and emit
/// an envelope with `"code":"invalid_regex"`.
#[test]
fn search_json_error_envelope_on_invalid_regex() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["search", "[invalid", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");

    assert!(
        !out.status.success(),
        "search with invalid regex must exit non-zero\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(val) => val,
        Err(_) => panic!("stdout must be valid JSON\ngot: {stdout}"),
    };

    assert_eq!(
        v["code"].as_str(),
        Some("invalid_regex"),
        "code must be 'invalid_regex'\ngot: {v}"
    );
    assert!(
        v.get("error").is_some(),
        "envelope must have 'error' field\ngot: {v}"
    );
    assert!(
        v.get("error_kind").is_none(),
        "envelope must NOT have 'error_kind'\ngot: {v}"
    );
}

/// `8v read` on a nonexistent path with `--json` must exit 1 and emit
/// an envelope with `"code":"not_found"`.
#[test]
fn read_json_error_envelope_on_missing_path() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["read", "/nonexistent_b2b_path/totally_missing.rs", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read");

    assert!(
        !out.status.success(),
        "read of nonexistent path must exit non-zero\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(val) => val,
        Err(_) => panic!("stdout must be valid JSON\ngot: {stdout}"),
    };

    assert_eq!(
        v["code"].as_str(),
        Some("not_found"),
        "code must be 'not_found'\ngot: {v}"
    );
    assert!(
        v.get("error").is_some(),
        "envelope must have 'error' field\ngot: {v}"
    );
    assert!(
        v.get("error_kind").is_none(),
        "envelope must NOT have 'error_kind'\ngot: {v}"
    );
}

/// `8v upgrade --json` must emit an envelope that matches the canonical shape
/// regardless of whether the network call succeeds or fails.
///
/// The upgrade command may resolve errors inline (exit 0 with error fields) or
/// via CommandError (exit 1). Both paths must produce canonical shape — no
/// `"error_kind"` key in either case.
#[test]
fn upgrade_json_envelope_matches_contract_shape() {
    // Upgrade does not need a workspace, but use a temp dir as cwd.
    let dir = tempfile::tempdir().expect("tempdir");
    let out = bin()
        .args(["upgrade", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v upgrade --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // stdout must be non-empty and valid JSON
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(val) => val,
        Err(_) => panic!("stdout must be valid JSON regardless of exit code\ngot: {stdout}"),
    };

    // Regardless of exit code: `"error_kind"` must never appear (old wrong shape)
    assert!(
        v.get("error_kind").is_none(),
        "envelope must NOT have 'error_kind' (old wrong shape)\ngot: {v}"
    );

    if !out.status.success() {
        // Network failure path: must have canonical `"error"` and `"code":"network"`
        assert!(
            v.get("error").is_some(),
            "on failure, envelope must have 'error' field\ngot: {v}"
        );
        assert_eq!(
            v["code"].as_str(),
            Some("network"),
            "on failure, code must be 'network'\ngot: {v}"
        );
    }
    // exit 0 path: upgrade succeeded or reported an inline error — just verify
    // no wrong-shape field (already asserted above).
}

/// Under `--json`, stderr must be empty on error paths — all error information
/// goes to stdout as a JSON envelope.
#[test]
fn json_mode_stderr_empty_on_error() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["read", "/nonexistent_b2b_stderr_check.rs", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read");

    assert!(
        !out.status.success(),
        "must exit non-zero\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.is_empty(),
        "stderr must be empty under --json; got: {stderr}"
    );

    // stdout must be the envelope
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(val) => val,
        Err(_) => panic!("stdout must be valid JSON\ngot: {stdout}"),
    };
    assert!(
        v.get("error").is_some() && v.get("code").is_some(),
        "stdout envelope must have 'error' and 'code'\ngot: {v}"
    );
}
