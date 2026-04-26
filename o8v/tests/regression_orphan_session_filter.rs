// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Regression test for Bug #3: `8v log --session <id>` must NOT surface
//! orphan-run warnings from *other* sessions.
//!
//! Setup: two sessions in events.ndjson
//!   - Session A: one orphan CommandStarted (no matching CommandCompleted)
//!   - Session B: one clean CommandStarted + CommandCompleted pair
//!
//! When filtering to session B, the output must contain zero orphan warnings.
//! Before the fix, the full-event-set warnings (including session A's orphan)
//! were passed directly to build_drill_report, leaking session A's warning.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn home_with_events(events_ndjson: &str) -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    fs::write(dot_8v.join("events.ndjson"), events_ndjson).expect("write events.ndjson");
    dir
}

fn make_orphan_started(session_id: &str, run_id: &str, command: &str) -> String {
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": 1_700_000_000_000_i64,
        "version": "0.1.0",
        "caller": "cli",
        "command": command,
        "argv": [command, "."],
        "command_bytes": command.len() as u64,
        "command_token_estimate": (command.len() / 4) as u64,
        "project_path": null,
        "session_id": session_id,
    });
    format!("{}\n", serde_json::to_string(&started).unwrap())
}

fn make_event_pair(
    session_id: &str,
    run_id: &str,
    command: &str,
    success: bool,
    output_bytes: u64,
) -> String {
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": 1_700_000_002_000_i64,
        "version": "0.1.0",
        "caller": "cli",
        "command": command,
        "argv": [command, "."],
        "command_bytes": command.len() as u64,
        "command_token_estimate": (command.len() / 4) as u64,
        "project_path": null,
        "session_id": session_id,
    });
    let completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": run_id,
        "timestamp_ms": 1_700_000_003_000_i64,
        "output_bytes": output_bytes,
        "token_estimate": 128_u64,
        "duration_ms": 42_u64,
        "success": success,
        "session_id": session_id,
    });
    format!(
        "{}\n{}\n",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&completed).unwrap()
    )
}

const SESSION_A: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA1";
const SESSION_B: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA2";

/// Bug #3 regression: filtering to session B must not surface session A's orphan warning.
#[test]
fn orphan_warning_does_not_leak_across_sessions() {
    // Session A: orphan CommandStarted (no CommandCompleted)
    let session_a_events = make_orphan_started(SESSION_A, "run_orphan_a", "check");
    // Session B: clean pair
    let session_b_events = make_event_pair(SESSION_B, "run_clean_b", "check", true, 256);

    let all_events = format!("{}{}", session_a_events, session_b_events);
    let home = home_with_events(&all_events);

    let out = bin()
        .args(["log", "--json", "--session", SESSION_B])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session SESSION_B");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "expected exit 0 for session B\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    // Confirm we got session B's drill-in
    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        SESSION_B,
        "expected session B drill-in\ngot: {v}"
    );

    // The warnings array must not contain any orphan_started warning
    let warnings = v["warnings"].as_array().cloned().unwrap_or_default();
    let orphan_warnings: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("orphan_started"))
        .collect();

    assert!(
        orphan_warnings.is_empty(),
        "session B must not surface orphan warnings from session A\n\
         found: {orphan_warnings:?}\nfull output: {v}"
    );
}
