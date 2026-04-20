// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end failing-first tests for `8v stats --session <id>` (§6 items 5-10).

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock predates UNIX epoch")
        .as_millis() as i64
}

#[allow(clippy::too_many_arguments)]
fn event_pair_at(
    session_id: &str,
    run_id: &str,
    command: &str,
    timestamp_ms: i64,
    duration_ms: u64,
    success: bool,
    argv: &[&str],
    agent_name: Option<&str>,
    output_bytes: u64,
) -> String {
    let argv_owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    let agent_field = match agent_name {
        Some(n) => serde_json::json!({
            "name": n,
            "version": "x.y.z",
            "protocol_version": "2024-11-05"
        }),
        None => serde_json::Value::Null,
    };
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": timestamp_ms,
        "version": "0.1.0",
        "caller": if agent_name.is_some() { "mcp" } else { "cli" },
        "command": command,
        "argv": argv_owned,
        "command_bytes": command.len() as u64,
        "command_token_estimate": (command.len() / 4) as u64,
        "project_path": null,
        "agent_info": agent_field,
        "session_id": session_id,
    });
    let completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": run_id,
        "timestamp_ms": timestamp_ms + duration_ms as i64,
        "output_bytes": output_bytes,
        "token_estimate": 128_u64,
        "duration_ms": duration_ms,
        "success": success,
        "session_id": session_id,
    });
    format!(
        "{}\n{}\n",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&completed).unwrap()
    )
}

/// Unique session IDs for test isolation.
/// ULID body is 26 Crockford-base32 characters (A-Z excluding I/L/O/U, 0-9).
const SES_A: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAAA";
const SES_B: &str = "ses_01HZBBBBBBBBBBBBBBBBBBBBBB";

// ─── Test 5: stats_session_filters_to_session_events_only ───────────────────

/// `--session <id>` must produce only events from that session.
/// Events from other sessions must be absent from the per-command table.
#[test]
fn stats_session_filters_to_session_events_only() {
    let now = now_ms();
    // SES_A: two "read" commands
    let ndjson = format!(
        "{}{}",
        event_pair_at(
            SES_A,
            "run_a1",
            "read",
            now - 60_000,
            50,
            true,
            &["read", "a.rs"],
            None,
            256,
        ),
        // SES_B: one "write" command — must not appear when filtering by SES_A
        event_pair_at(
            SES_B,
            "run_b1",
            "write",
            now - 50_000,
            40,
            true,
            &["write", "b.rs"],
            None,
            384,
        ),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_A, "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session");

    assert!(out.status.success(), "exit 0 for known session");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    let rows = v["rows"].as_array().expect("rows array");
    let labels: Vec<&str> = rows.iter().filter_map(|r| r["label"].as_str()).collect();
    assert!(
        labels.contains(&"read"),
        "expected read row for SES_A, got {labels:?}"
    );
    assert!(
        !labels.contains(&"write"),
        "write (from SES_B) must not appear when filtering SES_A, got {labels:?}"
    );
}

// ─── Test 6: stats_session_window_header_shows_session_id ───────────────────

/// Plain output must have a `session:` header, not `window:`.
#[test]
fn stats_session_window_header_shows_session_id() {
    let now = now_ms();
    let ndjson = event_pair_at(
        SES_A,
        "run_a1",
        "read",
        now - 60_000,
        50,
        true,
        &["read", "a.rs"],
        None,
        128,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_A])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session (plain)");

    assert!(out.status.success(), "exit 0 for known session");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("session:"),
        "plain output must contain 'session:' header, got:\n{stdout}"
    );
    assert!(
        stdout.contains(SES_A),
        "plain output must contain the session id {SES_A}, got:\n{stdout}"
    );
}

// ─── Test 7: stats_session_json_has_session_id_field ────────────────────────

/// `--json` output must have a top-level `session_id` string field.
#[test]
fn stats_session_json_has_session_id_field() {
    let now = now_ms();
    let ndjson = event_pair_at(
        SES_A,
        "run_a1",
        "read",
        now - 60_000,
        50,
        true,
        &["read", "a.rs"],
        None,
        192,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_A, "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session --json");

    assert!(out.status.success(), "exit 0 for known session");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    let session_id = v["session_id"]
        .as_str()
        .expect("top-level session_id string field must be present");
    assert_eq!(
        session_id, SES_A,
        "session_id field must match the requested id"
    );
}

// ─── Test 8: stats_session_since_flag_ignored_with_warning ──────────────────

/// `--session <id> --since 3d` must emit a stderr warning and use full session
/// events, not the time window.
#[test]
fn stats_session_since_flag_ignored_with_warning() {
    let now = now_ms();
    // Place event 10 days ago — outside a 3d window, inside the session
    let ndjson = event_pair_at(
        SES_A,
        "run_a1",
        "read",
        now - 10 * 24 * 3600 * 1000,
        50,
        true,
        &["read", "a.rs"],
        None,
        320,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_A, "--since", "3d", "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session --since");

    assert!(
        out.status.success(),
        "exit 0 — session filter wins over --since"
    );
    // Warning about --since being ignored must appear in stderr
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--since") && stderr.contains("ignored"),
        "stderr must warn that --since is ignored, got:\n{stderr}"
    );
    // The 10-day-old event must appear (session filter applied, not time window)
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    let rows = v["rows"].as_array().expect("rows array");
    assert!(
        !rows.is_empty(),
        "rows must be non-empty — session filter should include the 10d-old event"
    );
}

// ─── Test 9: stats_session_unknown_id_exits_1 ───────────────────────────────

/// Valid session id format but no matching events → stderr `no matching events`, exit 1.
#[test]
fn stats_session_unknown_id_exits_1() {
    let now = now_ms();
    // Events exist, but belong to SES_A — filtering by SES_B yields nothing
    let ndjson = event_pair_at(
        SES_A,
        "run_a1",
        "read",
        now - 60_000,
        50,
        true,
        &["read", "a.rs"],
        None,
        448,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_B])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session unknown");

    assert_eq!(
        out.status.code(),
        Some(1),
        "exit code must be 2 for unknown session"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no matching events"),
        "stdout must say 'no matching events', got:\n{stdout}"
    );
}

// ─── Test 10: legacy_events_not_matched_by_session_filter ───────────────────

/// Events with no `session_id` field (legacy format) must not appear in results
/// when `--session` is set. The lenient parser drops them as malformed; they are
/// never present in the filtered event list.
#[test]
fn legacy_events_not_matched_by_session_filter() {
    let now = now_ms();
    // Legacy event: missing session_id field entirely
    let legacy_started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": "run_legacy1",
        "timestamp_ms": now - 60_000,
        "version": "0.1.0",
        "caller": "cli",
        "command": "read",
        "argv": ["read", "legacy.rs"],
        "command_bytes": 4_u64,
        "command_token_estimate": 1_u64,
        "project_path": null,
        // no session_id field
    });
    let legacy_completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": "run_legacy1",
        "timestamp_ms": now - 59_950,
        "output_bytes": 100_u64,
        "token_estimate": 25_u64,
        "duration_ms": 50_u64,
        "success": true,
        // no session_id field
    });
    // One valid event in SES_A for a different command — to confirm SES_A still works
    let valid = event_pair_at(
        SES_A,
        "run_a1",
        "write",
        now - 50_000,
        40,
        true,
        &["write", "a.rs"],
        None,
        256,
    );
    let ndjson = format!(
        "{}\n{}\n{}",
        serde_json::to_string(&legacy_started).unwrap(),
        serde_json::to_string(&legacy_completed).unwrap(),
        valid
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--session", SES_A, "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --session with legacy events");

    assert!(out.status.success(), "exit 0 for known session");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    let rows = v["rows"].as_array().expect("rows array");
    let labels: Vec<&str> = rows.iter().filter_map(|r| r["label"].as_str()).collect();
    // Only the valid SES_A "write" event should appear
    assert!(
        labels.contains(&"write"),
        "write (valid SES_A event) must appear"
    );
    // Legacy "read" must not appear — it was dropped by the lenient parser
    assert!(
        !labels.contains(&"read"),
        "legacy read event (no session_id) must not appear"
    );
}
