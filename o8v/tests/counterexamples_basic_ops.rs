// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Counterexample tests for basic operation invariants in log / stats / event-store.
//!
//! Every test in this file was written BEFORE the corresponding bug is fixed.
//! All tests must FAIL on pre-fix code.  They document invariants that the
//! existing test suite does not assert.
//!
//! Gap inventory:
//!   Gap 1 — output_bytes=0 survives round-trip (log show --json reports 0)
//!   Gap 2 — output_bytes exact value propagates to output_bytes_total in log
//!   Gap 3 — output_bytes exact value propagates to output_bytes_per_call_mean in stats
//!   Gap 4 — ok_rate is computed from actual success counts, not hardcoded
//!   Gap 5 — retry cluster detection fires on same-command events within 30 s
//!   Gap 6 — Unknown event type does not corrupt the session record

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

fn parse_json(s: &str) -> serde_json::Value {
    match serde_json::from_str(s) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\nstdout: {s}"),
    }
}

fn as_u64(v: &serde_json::Value, field: &str) -> u64 {
    match v[field].as_u64() {
        Some(n) => n,
        None => panic!("{field} missing or not u64 in: {v}"),
    }
}

fn as_f64(v: &serde_json::Value, field: &str) -> f64 {
    match v[field].as_f64() {
        Some(n) => n,
        None => panic!("{field} missing or not f64 in: {v}"),
    }
}

fn as_array<'a>(v: &'a serde_json::Value, field: &str) -> &'a Vec<serde_json::Value> {
    match v[field].as_array() {
        Some(a) => a,
        None => panic!("missing '{field}' array in: {v}"),
    }
}

/// Build a CommandStarted + CommandCompleted pair with an explicit output_bytes value.
fn pair_with_output_bytes(
    session_id: &str,
    run_id: &str,
    command: &str,
    timestamp_ms: i64,
    output_bytes: u64,
    success: bool,
) -> String {
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": timestamp_ms,
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
        "timestamp_ms": timestamp_ms + 42_i64,
        "output_bytes": output_bytes,
        "token_estimate": output_bytes / 4,
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

// ─── Gap 1: output_bytes=0 survives round-trip ───────────────────────────────

#[test]
fn gap1_output_bytes_zero_survives_round_trip() {
    let session_id = "ses_gap1_zero_round_trip_00000";
    let ndjson =
        pair_with_output_bytes(session_id, "run_g1_01", "check", 1_700_000_000_000, 0, true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "show", session_id])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log show");

    assert!(
        out.status.success(),
        "8v log show should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);

    let got = as_u64(&v, "output_bytes_total");
    assert_eq!(
        got, 0,
        "output_bytes_total should be 0 when event has output_bytes=0, got {got}"
    );
}

// ─── Gap 2: output_bytes exact value propagates to output_bytes_total ────────

#[test]
fn gap2_output_bytes_exact_value_propagates_to_total() {
    let session_id = "ses_gap2_exact_total_00000000";
    let unusual_bytes: u64 = 777;
    let ndjson = pair_with_output_bytes(
        session_id,
        "run_g2_01",
        "read",
        1_700_000_000_000,
        unusual_bytes,
        true,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "show", session_id])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log show");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);
    let got = as_u64(&v, "output_bytes_total");

    assert_eq!(
        got, unusual_bytes,
        "output_bytes_total should equal the event's output_bytes ({unusual_bytes}), got {got}"
    );
}

// ─── Gap 3: output_bytes exact value propagates to stats per-call mean ───────

#[test]
fn gap3_output_bytes_per_call_mean_reflects_actual_events() {
    let session_id = "ses_gap3_stats_mean_000000000";
    let unusual_bytes: u64 = 333;
    let base_ts = now_ms() - 60_000;
    let ndjson = format!(
        "{}{}",
        pair_with_output_bytes(session_id, "run_g3_01", "ls", base_ts, unusual_bytes, true),
        pair_with_output_bytes(
            session_id,
            "run_g3_02",
            "ls",
            base_ts + 1_000,
            unusual_bytes,
            true
        ),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "8v stats should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);
    let rows = as_array(&v, "rows");

    let ls_row = match rows.iter().find(|r| r["label"].as_str() == Some("ls")) {
        Some(r) => r,
        None => panic!("no 'ls' row in stats output: {v}"),
    };

    let mean = as_f64(ls_row, "output_bytes_per_call_mean");

    assert!(
        (mean - unusual_bytes as f64).abs() < 0.01,
        "output_bytes_per_call_mean should be {unusual_bytes}, got {mean}"
    );
}

// ─── Gap 4: ok_rate is computed from actual success counts ───────────────────

#[test]
fn gap4_ok_rate_reflects_actual_success_ratio() {
    let session_id = "ses_gap4_ok_rate_000000000000";
    let base_ts = now_ms() - 60_000;
    let ndjson = format!(
        "{}{}{}",
        pair_with_output_bytes(session_id, "run_g4_01", "write", base_ts, 512, true),
        pair_with_output_bytes(session_id, "run_g4_02", "write", base_ts + 1000, 512, true),
        pair_with_output_bytes(session_id, "run_g4_03", "write", base_ts + 2000, 512, false),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "8v stats should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);
    let rows = as_array(&v, "rows");

    let write_row = match rows.iter().find(|r| r["label"].as_str() == Some("write")) {
        Some(r) => r,
        None => panic!("no 'write' row in stats output: {v}"),
    };

    let ok_rate = as_f64(write_row, "ok_rate");

    let expected = 2.0_f64 / 3.0_f64;
    assert!(
        (ok_rate - expected).abs() < 0.01,
        "ok_rate should be ~{expected:.4} (2 successes / 3 total), got {ok_rate}"
    );
}

// ─── Gap 5: retry cluster detection fires on same-command events within 30 s ─

#[test]
fn gap5_retry_cluster_detected_for_rapid_repeated_commands() {
    let session_id = "ses_gap5_retry_cluster_0000000";
    let base_ts = now_ms();
    let ndjson = format!(
        "{}{}{}",
        pair_with_output_bytes(session_id, "run_g5_01", "check", base_ts, 512, true),
        pair_with_output_bytes(session_id, "run_g5_02", "check", base_ts + 700, 512, true),
        pair_with_output_bytes(session_id, "run_g5_03", "check", base_ts + 1400, 512, true),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "show", session_id])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log show");

    assert!(
        out.status.success(),
        "8v log show should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);
    let clusters = as_array(&v, "clusters");

    assert!(
        !clusters.is_empty(),
        "expected at least one retry cluster for 3 rapid 'check' commands, but clusters array is empty\nfull output: {v}"
    );

    let has_retry = clusters.iter().any(|c| c["kind"].as_str() == Some("retry"));

    assert!(
        has_retry,
        "expected a cluster with kind='retry', got: {clusters:?}"
    );
}

// ─── Gap 6: Unknown event type does not corrupt the session record ────────────

#[test]
fn gap6_unknown_event_type_skipped_valid_commands_survive() {
    let session_id = "ses_gap6_unknown_event_0000000";
    let unusual_bytes: u64 = 999;
    let good_pair = pair_with_output_bytes(
        session_id,
        "run_g6_01",
        "fmt",
        1_700_000_000_000,
        unusual_bytes,
        true,
    );

    let unknown_event = serde_json::json!({
        "event": "SomeFutureEventTypeWeDoNotKnow",
        "session_id": session_id,
        "run_id": "run_g6_unknown",
        "timestamp_ms": 1_700_000_000_500_i64,
        "arbitrary_field": "arbitrary_value",
    });
    let ndjson = format!(
        "{}{}\n",
        good_pair,
        serde_json::to_string(&unknown_event).unwrap(),
    );
    let home = home_with_events(&ndjson);

    let list_out = bin()
        .args(["log", "--json"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json");

    assert!(
        list_out.status.success(),
        "8v log --json should exit 0\nstderr: {}",
        String::from_utf8_lossy(&list_out.stderr)
    );

    let list_stdout = String::from_utf8_lossy(&list_out.stdout);
    let list_v = parse_json(&list_stdout);
    let sessions = as_array(&list_v, "sessions");

    let found = sessions
        .iter()
        .any(|s| s["session_id"].as_str() == Some(session_id));

    assert!(
        found,
        "session {session_id} should appear in log list even after unknown event; got: {list_v}"
    );

    let drill_out = bin()
        .args(["log", "--json", "show", session_id])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log show");

    let drill_stdout = String::from_utf8_lossy(&drill_out.stdout);
    let drill_v = parse_json(&drill_stdout);
    let got_bytes = as_u64(&drill_v, "output_bytes_total");

    assert_eq!(
        got_bytes, unusual_bytes,
        "output_bytes_total should be {unusual_bytes} (from the valid event), got {got_bytes}"
    );
}
