// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Regression tests for the blind-spots footer conditional.
//!
//! When the filtered event set contains at least one event with `caller="hook"`,
//! the footer MUST drop the "native Read/Edit/Bash invisible" clause while
//! retaining "write-success ≠ code-correct."
//!
//! When only cli/mcp events are present, the full original message must appear.

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time goes forward")
        .as_millis() as i64
}

fn home_with_events(events_ndjson: &str) -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    fs::write(dot_8v.join("events.ndjson"), events_ndjson).expect("write events.ndjson");
    dir
}

/// Build a CommandStarted + CommandCompleted pair with an explicit caller.
fn make_event_pair_with_caller(
    session_id: &str,
    run_id: &str,
    command: &str,
    caller: &str,
    success: bool,
) -> String {
    let ts_start: i64 = now_ms();
    let ts_end: i64 = ts_start + 1_000;
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": ts_start,
        "version": "0.1.0",
        "caller": caller,
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
        "timestamp_ms": ts_end,
        "output_bytes": 512_u64,
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

// ─── stats: hook events present ─────────────────────────────────────────────

#[test]
fn footer_drops_native_blindspot_when_hook_events_present_stats() {
    let events = make_event_pair_with_caller("sess-hook-1", "run-1", "read", "hook", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["stats"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success() || stdout.contains("write-success"),
        "command failed unexpectedly: stderr={stderr}"
    );

    assert!(
        stdout.contains("write-success"),
        "stats footer must contain 'write-success' when hook events present, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("native Read/Edit/Bash invisible"),
        "stats footer must NOT contain 'native Read/Edit/Bash invisible' when hook events present, got:\n{stdout}"
    );
}

// ─── log: hook events present ───────────────────────────────────────────────

#[test]
fn footer_drops_native_blindspot_when_hook_events_present_log() {
    let events = make_event_pair_with_caller("sess-hook-2", "run-2", "read", "hook", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["log"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success() || stdout.contains("write-success"),
        "command failed unexpectedly: stderr={stderr}"
    );

    assert!(
        stdout.contains("write-success"),
        "log footer must contain 'write-success' when hook events present, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("native Read/Edit/Bash invisible"),
        "log footer must NOT contain 'native Read/Edit/Bash invisible' when hook events present, got:\n{stdout}"
    );
}

// ─── stats: cli-only events ──────────────────────────────────────────────────

#[test]
fn footer_keeps_native_blindspot_when_only_cli_events_stats() {
    let events = make_event_pair_with_caller("sess-cli-1", "run-3", "read", "cli", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["stats"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success() || stdout.contains("native Read"),
        "command failed unexpectedly: stderr={stderr}"
    );

    assert!(
        stdout.contains("native Read/Edit/Bash invisible"),
        "stats footer must contain full blind-spots message when only cli events present, got:\n{stdout}"
    );
}

// ─── log: cli-only events ────────────────────────────────────────────────────

#[test]
fn footer_keeps_native_blindspot_when_only_cli_events_log() {
    let events = make_event_pair_with_caller("sess-cli-2", "run-4", "read", "cli", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["log"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success() || stdout.contains("native Read"),
        "command failed unexpectedly: stderr={stderr}"
    );

    assert!(
        stdout.contains("native Read/Edit/Bash invisible"),
        "log footer must contain full blind-spots message when only cli events present, got:\n{stdout}"
    );
}
