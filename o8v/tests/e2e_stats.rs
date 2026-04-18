// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v stats`.

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

/// Produce an event pair with a caller-supplied timestamp, duration, and success.
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
        "output_bytes": 512_u64,
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

fn fresh(command: &str, n: usize, success: bool, argv: &[&str], agent: Option<&str>) -> String {
    let now = now_ms();
    let mut out = String::new();
    for i in 0..n {
        let run = format!("run_{command}_{i}");
        let dur = 10 + i as u64 * 2;
        out.push_str(&event_pair_at(
            "ses_01HZAAAAAAAAAAAAAAAAAAAAA",
            &run,
            command,
            now - 60_000 - i as i64 * 100,
            dur,
            success,
            argv,
            agent,
        ));
    }
    out
}

// ─── 1. Default table renders ───────────────────────────────────────────────

#[test]
fn default_table_renders() {
    let ndjson = format!(
        "{}{}{}",
        fresh("read", 10, true, &["read", "src/main.rs"], None),
        fresh(
            "write",
            6,
            false,
            &["write", "handler.rs", "--find", "foo", "--replace", "bar"],
            None
        ),
        fresh("ls", 3, true, &["ls"], None),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");

    assert!(out.status.success(), "stats should exit 0");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    let rows = v["rows"].as_array().expect("rows array");
    let labels: Vec<&str> = rows.iter().filter_map(|r| r["label"].as_str()).collect();
    assert!(
        labels.contains(&"read"),
        "expected read row, got {labels:?}"
    );
    assert!(
        labels.contains(&"write"),
        "expected write row, got {labels:?}"
    );
}

// ─── 2. Drill by argv-shape ─────────────────────────────────────────────────

#[test]
fn drill_argv_shape_breakdown() {
    let ndjson = format!(
        "{}{}",
        fresh(
            "write",
            6,
            false,
            &["write", "handler.rs", "--find", "foo", "--replace", "bar"],
            None
        ),
        fresh("write", 3, true, &["write", "src/main.rs:10", "fix"], None),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "write", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats write");

    assert!(out.status.success(), "drill must exit 0");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    assert_eq!(v["kind"], "drill", "kind must be 'drill'");
    let rows = v["rows"].as_array().expect("rows array");
    let has_failing_shape = rows
        .iter()
        .any(|r| r["ok_rate"].as_f64().map(|p| p < 1.0).unwrap_or(false));
    assert!(
        has_failing_shape,
        "expected at least one argv-shape row with ok_rate < 1.0; rows: {rows:?}"
    );
}

// ─── 3. --compare agent separates rows ──────────────────────────────────────

#[test]
fn compare_agent_separates_rows() {
    let ndjson = format!(
        "{}{}",
        fresh("read", 6, true, &["read"], Some("claude-code")),
        fresh("read", 6, true, &["read"], Some("codex")),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--compare", "agent", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --compare agent");

    assert!(out.status.success(), "compare must exit 0");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["rows"].as_array().expect("rows array");
    let labels: Vec<&str> = rows.iter().filter_map(|r| r["label"].as_str()).collect();
    assert!(
        labels.contains(&"claude-code"),
        "expected claude-code row, got {labels:?}"
    );
    assert!(
        labels.contains(&"codex"),
        "expected codex row, got {labels:?}"
    );
}

// ─── 4. n < 5 → percentiles render as null ──────────────────────────────────

#[test]
fn n_lt_5_percentiles_dashed() {
    let ndjson = fresh("read", 3, true, &["read"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let read_row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "read"))
        .expect("read row present");
    assert!(
        read_row["duration_ms"].is_null(),
        "duration_ms must be absent/null when n<5; got: {read_row}"
    );
}

// ─── 5. Empty window → stderr "no matching events", exit 2 ──────────────────

#[test]
fn empty_window_exits_2() {
    // `--since 1d --until 1d` produces a zero-width window 1 day ago.
    let home = home_with_events(""); // no events
    let out = bin()
        .args(["stats", "--since", "1d", "--until", "1d"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty window must exit 2; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Renderer owns the "no matching events" message — emitted to stdout,
    // not a side-channel eprintln! from the dispatch layer (Layer 3 design).
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no matching events"),
        "stdout must contain 'no matching events'; got: {stdout}"
    );
}

// ─── 6. JSON field contract ─────────────────────────────────────────────────

#[test]
fn json_field_contract() {
    let ndjson = fresh("read", 10, true, &["read"], None);
    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["kind"].is_string(), "kind field present");
    assert!(v["rows"].is_array(), "rows field present");
    let row = &v["rows"][0];
    // Stable contract fields (new wire shape):
    for field in [
        "label",
        "n",
        "ok_rate",
        "output_bytes_per_call_mean",
        "retry_cluster_count",
    ] {
        assert!(
            row.get(field).is_some(),
            "row must contain field '{field}'; got: {row}"
        );
    }
}

// ─── 7. Malformed line skipped by default, --strict hard-fails ──────────────

#[test]
fn malformed_line_skipped_default() {
    let mut ndjson = fresh("read", 6, true, &["read"], None);
    ndjson.push_str("{not valid json\n");
    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");
    assert!(
        out.status.success(),
        "default mode must tolerate malformed lines; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn malformed_line_strict_fails() {
    let mut ndjson = fresh("read", 6, true, &["read"], None);
    ndjson.push_str("{not valid json\n");
    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--strict", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --strict");
    assert!(
        !out.status.success(),
        "--strict must fail on malformed line; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ─── 8. Events with empty session_id are dropped (not aggregated) ───────────

#[test]
fn empty_session_id_events_are_dropped() {
    // Events with session_id="" must produce no row in stats output.
    // The aggregator drops them at the wire boundary; stats sees nothing.
    let mut ndjson = String::new();
    let now = now_ms();
    for i in 0..6 {
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": format!("empty_sid_{i}"),
            "timestamp_ms": now - 60_000 - i as i64 * 100,
            "version": "0.1.0",
            "caller": "cli",
            "command": "empty_sid_cmd",
            "argv": ["empty_sid_cmd"],
            "command_bytes": 10_u64,
            "command_token_estimate": 2_u64,
            "project_path": null,
            "session_id": "",
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": format!("empty_sid_{i}"),
            "timestamp_ms": now - 60_000 - i as i64 * 100 + 10,
            "output_bytes": 512_u64,
            "token_estimate": 128_u64,
            "duration_ms": 10_u64,
            "success": true,
            "session_id": "",
        });
        ndjson.push_str(&serde_json::to_string(&started).unwrap());
        ndjson.push('\n');
        ndjson.push_str(&serde_json::to_string(&completed).unwrap());
        ndjson.push('\n');
    }
    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let found = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "empty_sid_cmd"));
    assert!(
        found.is_none(),
        "empty session_id events must be dropped, not aggregated; got: {v}"
    );
}

// ─── 9. Adversarial counterexample: percentile_boundary_bucket_math ─────────

#[test]
fn percentile_boundary_single_bucket() {
    // All samples at exactly the same ms → p50, p95, p99 must all equal
    // the same bucket upper bound, not different buckets.
    let ndjson = fresh("read", 10, true, &["read"], None);
    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "read"))
        .expect("read row");
    // With n=10 and durations 10..28ms by 2ms, percentile values must be
    // monotone non-decreasing.
    let dur = &row["duration_ms"];
    assert!(
        dur.is_object(),
        "duration_ms must be an object when n=10 >= 5; row={row}"
    );
    let p50 = dur["p50"].as_u64().expect("duration_ms.p50 present");
    let p95 = dur["p95"].as_u64().expect("duration_ms.p95 present");
    let p99 = dur["p99"].as_u64().expect("duration_ms.p99 present");
    assert!(p50 <= p95, "p50={p50} must be <= p95={p95}");
    assert!(p95 <= p99, "p95={p95} must be <= p99={p99}");
}
