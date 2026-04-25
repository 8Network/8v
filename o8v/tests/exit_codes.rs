// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Failing-first tests locking in the 0/1/2 exit-code contract (B2c).
//!
//! Exit 0 = success, Exit 1 = user/runtime error, Exit 2 = clap parse failure only.
//!
//! These tests MUST FAIL on the pre-fix binary (which returns exit 2 for all four
//! cases). They exist to prove the new contract is enforced, not just documented.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn empty_home() -> TempDir {
    TempDir::new().expect("create temp dir")
}

fn empty_dir() -> TempDir {
    TempDir::new().expect("create temp dir")
}

// ─── check: no projects detected is user error → exit 1, not exit 2 ──────────

/// `8v check` on an empty directory finds no projects.
/// This is a user error (wrong path / uninitialized directory) → exit 1.
/// Pre-fix binary returns exit 2 — this test MUST FAIL before the fix.
#[test]
fn exit_code_check_no_projects() {
    let dir = empty_dir();

    let out = bin()
        .args(["check", dir.path().to_str().expect("valid path")])
        .output()
        .expect("run 8v check on empty dir");

    assert_eq!(
        out.status.code(),
        Some(1),
        "check with no projects must exit 1 (user error), not 2 (clap failure)"
    );
}

// ─── fmt: no projects detected is user error → exit 1, not exit 2 ────────────

/// `8v fmt` on an empty directory finds no projects.
/// This is a user error (wrong path / uninitialized directory) → exit 1.
/// Pre-fix binary returns exit 2 — this test MUST FAIL before the fix.
#[test]
fn exit_code_fmt_no_projects() {
    let dir = empty_dir();

    let out = bin()
        .args(["fmt", dir.path().to_str().expect("valid path")])
        .output()
        .expect("run 8v fmt on empty dir");

    assert_eq!(
        out.status.code(),
        Some(1),
        "fmt with no projects must exit 1 (user error), not 2 (clap failure)"
    );
}

// ─── log: empty event log is valid first-run state → exit 0, not exit 2 ──────

/// `8v log` when no events file exists (first-run / fresh home).
/// Empty event log is a normal first-run state, not an error → exit 0.
/// Pre-fix binary returns exit 2 — this test MUST FAIL before the fix.
#[test]
fn exit_code_log_empty() {
    let home = empty_home();
    // Do NOT create ~/.8v/events.ndjson — simulate fresh install.

    let out = bin()
        .args(["log"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with empty home");

    assert_eq!(
        out.status.code(),
        Some(0),
        "log with no events file must exit 0 (first-run state), not 2 (clap failure)"
    );
}

// ─── init: non-TTY context → init fails → exit 1, not exit 0 ────────────────

/// `8v init <path> --json` in a non-TTY context (as run by tests) should exit 1
/// when init fails (requires interactive terminal).
/// Pre-fix binary returns exit 0 even when the JSON body contains `"success":false`.
#[test]
fn exit_code_init_non_tty_json() {
    let dir = empty_dir();

    let out = bin()
        .args(["init", dir.path().to_str().expect("valid path"), "--json"])
        .output()
        .expect("run 8v init --json");

    // The JSON body must contain "success":false and the process must exit 1
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"success\"") && stdout.contains("false"),
        "init --json output must report success:false; got: {stdout}"
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "init failure must exit 1, not 0; stdout: {stdout}"
    );
}

// ─── stats: filtered-empty / zero-width window is user error → exit 1, not 2 ─

/// `8v stats --since 1d --until 1d` creates a zero-width window (no events can
/// fall in it). A user-supplied filter that yields no results is a user error → exit 1.
/// Pre-fix binary returns exit 2 — this test MUST FAIL before the fix.
#[test]
fn exit_code_stats_filtered_empty() {
    let home = empty_home();
    // Write at least one event so the file exists; it will be outside the window.
    let dot_8v = home.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    // Timestamp from 30 days ago — outside the 1d window.
    let old_ts: i64 = 0; // epoch — guaranteed outside any recent window
    let event = serde_json::json!({
        "event": "CommandStarted",
        "run_id": "run_exit_test",
        "timestamp_ms": old_ts,
        "version": "0.1.0",
        "caller": "cli",
        "command": "read",
        "argv": ["read", "a.rs"],
        "command_bytes": 4_u64,
        "command_token_estimate": 1_u64,
        "project_path": null,
        "agent_info": null,
        "session_id": "ses_01HZEXIT0000000000000000000",
    });
    let completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": "run_exit_test",
        "timestamp_ms": old_ts + 50,
        "output_bytes": 128_u64,
        "token_estimate": 32_u64,
        "duration_ms": 50_u64,
        "success": true,
        "session_id": "ses_01HZEXIT0000000000000000000",
    });
    let ndjson = format!(
        "{}\n{}\n",
        serde_json::to_string(&event).unwrap(),
        serde_json::to_string(&completed).unwrap()
    );
    fs::write(dot_8v.join("events.ndjson"), ndjson).expect("write events.ndjson");

    // --since 1d with no recent events → filtered_empty → exit 1
    let out = bin()
        .args(["stats", "--since", "1d"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --since 1d");

    assert_eq!(
        out.status.code(),
        Some(1),
        "stats with filtered-empty result must exit 1 (user error), not 2 (clap failure)"
    );
}
