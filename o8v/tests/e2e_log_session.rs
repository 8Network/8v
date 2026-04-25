// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v log --session <id>`.

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

fn make_event_pair(session_id: &str, run_id: &str, command: &str, success: bool) -> String {
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
    let completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": run_id,
        "timestamp_ms": 1_700_000_001_000_i64,
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

const KNOWN_SESSION: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAAA";

// ─── Test 1: exact match produces drill-in ────────────────────────────────────

#[test]
fn log_session_exact_match_produces_drill_in() {
    let events = make_event_pair(KNOWN_SESSION, "run_001", "check", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["log", "--json", "--session", KNOWN_SESSION])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "expected exit 0 for known session\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        KNOWN_SESSION,
        "session_id must match the requested session\ngot: {v}"
    );
    assert!(
        v["commands"].is_number(),
        "drill-in commands field must be a number\ngot: {v}"
    );
}

// ─── Test 2: unknown ID (valid format) exits 1 ───────────────────────────────

#[test]
fn log_session_unknown_id_exits_1() {
    let events = make_event_pair(KNOWN_SESSION, "run_001", "check", true);
    let home = home_with_events(&events);

    // Valid ULID-format session ID that does not appear in the events file.
    // B2c contract: valid format + no match = exit 1 (user error).
    const UNKNOWN: &str = "ses_01HZBBBBBBBBBBBBBBBBBBBBBB";

    let out = bin()
        .args(["log", "--session", UNKNOWN])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --session <unknown>");

    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit code 1 for unknown session\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ─── Test 3: invalid format is a parse error (non-zero, not exit 1) ──────────

#[test]
fn log_session_invalid_format_is_parse_error() {
    let home = home_with_events("");

    let out = bin()
        .args(["log", "--session", "notanid"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --session notanid");

    assert!(
        !out.status.success(),
        "expected non-zero exit for invalid session format\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    // clap parse errors exit 1 too, but the key requirement is non-zero
    // and that the rejection happens before any events are read (stderr has
    // clap error content).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("notanid") || stderr.contains("session") || stderr.contains("error"),
        "stderr should describe the parse error\ngot: {stderr}"
    );
}

// ─── Test 4: --limit alongside --session → notice on stderr, drill-in output ─

#[test]
fn log_session_ignores_limit_with_notice() {
    let events = make_event_pair(KNOWN_SESSION, "run_001", "check", true);
    let home = home_with_events(&events);

    let out = bin()
        .args(["log", "--session", KNOWN_SESSION, "--limit", "5"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --session --limit");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "expected exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The --limit-ignored notice must go to stderr (not pollute stdout).
    assert!(
        stderr.contains("ignored"),
        "stderr must mention that --limit is ignored\ngot stderr: {stderr}\nstdout: {stdout}"
    );
}
