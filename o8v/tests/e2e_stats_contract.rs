// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end contract tests for `8v stats --json`.
//!
//! These tests pin the exact JSON field names and value types emitted by the
//! stats renderer.  Each test maps 1-to-1 to a contract clause from
//! `docs/design/log_stats_implementation_v2.md` §3.4 + §10.
//!
//! Confirmed field names (from `o8v-core/src/render/stats_report.rs`):
//!   top-level:  kind  label_key  rows  warnings  failure_hotspots  [shape — drill only]
//!   per-row:    label  n  duration_ms{p50,p95,p99}  ok_rate  output_bytes_per_call_mean  retry_cluster_count
//!   label_key values: "command" (table), "argv_shape" (drill), "agent" (by_agent)

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── helpers ──────────────────────────────────────────────────────────────────

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

/// Build N completed event-pairs for `command` using distinct run-ids.
/// durations cycle 10, 12, 14, … ms so percentile values differ.
fn fresh(command: &str, n: usize, success: bool, argv: &[&str], agent: Option<&str>) -> String {
    let now = now_ms();
    let mut out = String::new();
    for i in 0..n {
        let run_id = format!("contract_run_{command}_{i}");
        let dur: u64 = 10 + i as u64 * 2;
        let timestamp_ms = now - 60_000 - i as i64 * 100;

        let argv_owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        let agent_field = match agent {
            Some(name) => serde_json::json!({
                "name": name,
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
            "caller": if agent.is_some() { "mcp" } else { "cli" },
            "command": command,
            "argv": argv_owned,
            "command_bytes": command.len() as u64,
            "command_token_estimate": (command.len() / 4) as u64,
            "project_path": serde_json::Value::Null,
            "agent_info": agent_field,
            "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": run_id,
            "timestamp_ms": timestamp_ms + dur as i64,
            "output_bytes": 512_u64,
            "token_estimate": 128_u64,
            "duration_ms": dur,
            "success": success,
            "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
        });
        out.push_str(&serde_json::to_string(&started).unwrap());
        out.push('\n');
        out.push_str(&serde_json::to_string(&completed).unwrap());
        out.push('\n');
    }
    out
}

// ── §3.4 contract test 1 ─────────────────────────────────────────────────────
// Top-level shape: kind="table", label_key="command", rows:[…], warnings:[], failure_hotspots:[]
// Each row contains exactly the required fields.

#[test]
fn contract_1_default_table_top_level_shape() {
    let ndjson = fresh("read", 10, true, &["read"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "stats --json must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    // top-level kind
    assert_eq!(
        v["kind"].as_str(),
        Some("table"),
        "kind must be \"table\" for default mode; got {:?}",
        v["kind"]
    );

    // top-level label_key — design §10: table mode emits "command"
    assert_eq!(
        v["label_key"].as_str(),
        Some("command"),
        "label_key must be \"command\" for default table; got {:?}",
        v["label_key"]
    );

    // rows present and non-empty
    let rows = v["rows"].as_array().expect("rows must be a JSON array");
    assert!(!rows.is_empty(), "rows must contain at least one entry");

    // shape must NOT be present in table mode
    assert!(
        v.get("shape").is_none() || v["shape"].is_null(),
        "shape must be absent/null in table mode; got {:?}",
        v["shape"]
    );

    // warnings and failure_hotspots must be present as arrays
    assert!(
        v["warnings"].is_array(),
        "warnings must be a JSON array; got {:?}",
        v["warnings"]
    );
    assert!(
        v["failure_hotspots"].is_array(),
        "failure_hotspots must be a JSON array; got {:?}",
        v["failure_hotspots"]
    );

    // every row has exactly the required fields
    for (i, row) in rows.iter().enumerate() {
        for field in ["label", "n", "retry_cluster_count"] {
            assert!(
                row.get(field).is_some(),
                "row[{i}] missing required field '{field}'; row = {row}"
            );
        }
    }
}

// ── §3.4 contract test 2 ─────────────────────────────────────────────────────
// n < 5 → duration_ms is absent/null; n ≥ 5 → duration_ms is an object with p50/p95/p99.

#[test]
fn contract_2_percentiles_null_below_threshold() {
    // 4 events — below the MIN_SAMPLES_FOR_PERCENTILE=5 threshold
    let ndjson = fresh("probe", 4, true, &["probe"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "probe"))
        .expect("probe row must be present");

    // duration_ms must be absent (skipped via serde) or null when n < 5
    let duration_ms = &row["duration_ms"];
    assert!(
        duration_ms.is_null(),
        "duration_ms must be absent/null when n=4 < 5; row={row}"
    );
}

#[test]
fn contract_2_percentiles_numeric_at_threshold() {
    // exactly 5 events — percentiles must be present
    let ndjson = fresh("probe5", 5, true, &["probe5"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "probe5"))
        .expect("probe5 row must be present");

    // duration_ms must be an object with p50, p95, p99
    let dur = &row["duration_ms"];
    assert!(
        dur.is_object(),
        "duration_ms must be an object when n=5 >= 5; row={row}"
    );

    let p50 = dur["p50"]
        .as_u64()
        .expect("duration_ms.p50 must be u64 when n=5 >= 5");
    let p95 = dur["p95"]
        .as_u64()
        .expect("duration_ms.p95 must be u64 when n=5 >= 5");
    let p99 = dur["p99"]
        .as_u64()
        .expect("duration_ms.p99 must be u64 when n=5 >= 5");

    // sanity: percentiles must be monotone non-decreasing
    assert!(p50 <= p95, "p50={p50} must be <= p95={p95}");
    assert!(p95 <= p99, "p95={p95} must be <= p99={p99}");
}

// ── §3.4 contract test 3 ─────────────────────────────────────────────────────
// ok_rate is a JSON number in [0.0, 1.0] when there are completed records;
// absent/null only when there are zero completed records for that command.

#[test]
fn contract_3_ok_rate_range_all_success() {
    let ndjson = fresh("ping", 6, true, &["ping"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "ping"))
        .expect("ping row must be present");

    let ok_rate = row["ok_rate"]
        .as_f64()
        .expect("ok_rate must be a number when n >= 1");
    assert!(
        (0.0..=1.0).contains(&ok_rate),
        "ok_rate={ok_rate} must be in [0.0, 1.0]"
    );
    // all 6 succeeded → must be 1.0
    assert!(
        (ok_rate - 1.0).abs() < 1e-9,
        "all events succeeded so ok_rate must be 1.0; got {ok_rate}"
    );
}

#[test]
fn contract_3_ok_rate_range_mixed() {
    // 6 success + 6 failure for "mixed_cmd"
    let success_events = fresh("mixed_cmd", 6, true, &["mixed_cmd"], None);
    // Disambiguate run_ids by building failure events manually; fresh() uses index-based
    // run-ids that would collide with the success batch.
    let now = now_ms();
    let mut ndjson = success_events;
    for i in 0..6_usize {
        let run_id = format!("contract_run_mixed_cmd_fail_{i}");
        let dur: u64 = 10 + i as u64 * 2;
        let ts = now - 120_000 - i as i64 * 100;
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": run_id,
            "timestamp_ms": ts,
            "version": "0.1.0",
            "caller": "cli",
            "command": "mixed_cmd",
            "argv": ["mixed_cmd"],
            "command_bytes": 9_u64,
            "command_token_estimate": 2_u64,
            "project_path": serde_json::Value::Null,
            "agent_info": serde_json::Value::Null,
            "session_id": "ses_contract_BBBBBBBBBBBBBBBBBBBBBBBBB",
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": run_id,
            "timestamp_ms": ts + dur as i64,
            "output_bytes": 512_u64,
            "token_estimate": 128_u64,
            "duration_ms": dur,
            "success": false,
            "session_id": "ses_contract_BBBBBBBBBBBBBBBBBBBBBBBBB",
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
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "mixed_cmd"))
        .expect("mixed_cmd row must be present");

    let ok_rate = row["ok_rate"]
        .as_f64()
        .expect("ok_rate must be a number for mixed events");
    assert!(
        (0.0..=1.0).contains(&ok_rate),
        "ok_rate={ok_rate} must be in [0.0, 1.0]"
    );
    // 6 success out of 12 total → ~0.5
    assert!(
        ok_rate > 0.0 && ok_rate < 1.0,
        "mixed success/failure must produce ok_rate in (0.0, 1.0); got {ok_rate}"
    );
}

// ── §3.4 contract test 4 ─────────────────────────────────────────────────────
// output_bytes_per_call_mean is a JSON number (f64) when output_bytes are present.

#[test]
fn contract_4_output_bytes_per_call_mean_is_number() {
    let ndjson = fresh("scan", 6, true, &["scan"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "scan"))
        .expect("scan row must be present");

    let opc = row["output_bytes_per_call_mean"]
        .as_f64()
        .expect("output_bytes_per_call_mean must be a number when output_bytes present");
    assert!(
        opc >= 0.0,
        "output_bytes_per_call_mean must be non-negative; got {opc}"
    );
}

// ── §3.4 contract test 5 ─────────────────────────────────────────────────────
// Drill mode (positional shape arg): kind="drill", shape=<s>, label_key="argv_shape"

#[test]
fn contract_5_drill_mode_shape() {
    let ndjson = fresh(
        "write",
        10,
        true,
        &["write", "handler.rs", "--find", "x", "--replace", "y"],
        None,
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "write", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats write --json");

    assert!(
        out.status.success(),
        "drill mode must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    assert_eq!(
        v["kind"].as_str(),
        Some("drill"),
        "kind must be \"drill\" in drill mode; got {:?}",
        v["kind"]
    );
    assert_eq!(
        v["shape"].as_str(),
        Some("write"),
        "shape must equal the positional arg \"write\"; got {:?}",
        v["shape"]
    );
    // design §10: drill mode label_key is "argv_shape"
    assert_eq!(
        v["label_key"].as_str(),
        Some("argv_shape"),
        "label_key must be \"argv_shape\" in drill mode; got {:?}",
        v["label_key"]
    );
    assert!(
        v["rows"].is_array(),
        "rows must be a JSON array in drill mode"
    );
    // envelope arrays must be present
    assert!(
        v["warnings"].is_array(),
        "warnings must be a JSON array in drill mode"
    );
    assert!(
        v["failure_hotspots"].is_array(),
        "failure_hotspots must be a JSON array in drill mode"
    );
}

// ── §3.4 contract test 6 ─────────────────────────────────────────────────────
// By-agent mode (--compare agent): kind="by_agent", label_key="agent"

#[test]
fn contract_6_by_agent_mode() {
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
        .expect("run 8v stats --compare agent --json");

    assert!(
        out.status.success(),
        "by-agent mode must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    assert_eq!(
        v["kind"].as_str(),
        Some("by_agent"),
        "kind must be \"by_agent\" in by-agent mode; got {:?}",
        v["kind"]
    );
    assert_eq!(
        v["label_key"].as_str(),
        Some("agent"),
        "label_key must be \"agent\" in by-agent mode; got {:?}",
        v["label_key"]
    );

    let rows = v["rows"].as_array().expect("rows must be a JSON array");
    let labels: Vec<&str> = rows.iter().filter_map(|r| r["label"].as_str()).collect();
    assert!(
        labels.contains(&"claude-code"),
        "by-agent rows must include \"claude-code\"; labels={labels:?}"
    );
    assert!(
        labels.contains(&"codex"),
        "by-agent rows must include \"codex\"; labels={labels:?}"
    );

    // shape must NOT appear in by-agent output
    assert!(
        v.get("shape").is_none() || v["shape"].is_null(),
        "shape must be absent/null in by-agent mode; got {:?}",
        v["shape"]
    );
}

// ── §3.4 contract test 7 ─────────────────────────────────────────────────────
// Empty window → exit code 2, stdout contains "no matching events"

#[test]
fn contract_7_empty_window_exit_2() {
    let home = home_with_events(""); // no events at all

    let out = bin()
        .args(["stats", "--since", "1d", "--until", "1d"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats on empty window");

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty window must exit 2; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no matching events"),
        "stdout must contain \"no matching events\"; got: {stdout}"
    );
}

// ── contract 7b: JSON path also exits 2 on empty window ─────────────────────

#[test]
fn stats_json_empty_window_exits_2() {
    let home = home_with_events(""); // no events at all

    let out = bin()
        .args(["stats", "--since", "1d", "--until", "1d", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json on empty window");

    assert_eq!(
        out.status.code(),
        Some(2),
        "JSON path: empty window must exit 2; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Must still produce valid JSON with an empty rows array.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON even on exit 2");
    let rows = v["rows"]
        .as_array()
        .expect("JSON must contain a rows array");
    assert!(
        rows.is_empty(),
        "rows must be empty on empty window; got: {rows:?}"
    );
}

// ── additional: row field types are exact ────────────────────────────────────
// n is u64, retry_cluster_count is u64, label is string. These are not optional.

#[test]
fn contract_row_field_types_are_exact() {
    let ndjson = fresh("fmt", 10, true, &["fmt"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = v["rows"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["label"] == "fmt"))
        .expect("fmt row must be present");

    // label is a non-empty string
    let label = row["label"].as_str().expect("label must be a string");
    assert!(!label.is_empty(), "label must be non-empty");

    // n is a non-zero u64 integer
    let n = row["n"].as_u64().expect("n must be a u64 integer");
    assert!(n > 0, "n must be > 0 when events are present");

    // retry_cluster_count is a u64 integer (may be 0)
    let _retry_cluster_count = row["retry_cluster_count"]
        .as_u64()
        .expect("retry_cluster_count must be a u64 integer");

    // duration_ms.p50/p95/p99 are u64 — verify they are NOT floats (integer ms).
    if let Some(dur) = row["duration_ms"].as_object() {
        assert!(
            dur["p50"]
                .as_f64()
                .map(|f| f.fract() == 0.0)
                .unwrap_or(false),
            "duration_ms.p50 must be an integer ms value; got {:?}",
            dur["p50"]
        );
    }
}

// ── counterexample tests ─────────────────────────────────────────────────────
// Each test proves that the OLD field names are gone.  These tests MUST fail
// against the pre-rewrite renderer and pass after.

/// Old field name "p50_ms" must not appear anywhere in the row object.
#[test]
fn counterexample_old_p50_ms_field_absent() {
    let ndjson = fresh("check", 10, true, &["check"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["rows"].as_array().expect("rows must be array");
    for (i, row) in rows.iter().enumerate() {
        assert!(
            row.get("p50_ms").is_none(),
            "row[{i}] must not contain old field 'p50_ms'; row={row}"
        );
        assert!(
            row.get("p95_ms").is_none(),
            "row[{i}] must not contain old field 'p95_ms'; row={row}"
        );
        assert!(
            row.get("p99_ms").is_none(),
            "row[{i}] must not contain old field 'p99_ms'; row={row}"
        );
    }
}

/// Old field name "ok_pct" must not appear anywhere in the row object.
#[test]
fn counterexample_old_ok_pct_field_absent() {
    let ndjson = fresh("build", 6, true, &["build"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["rows"].as_array().expect("rows must be array");
    for (i, row) in rows.iter().enumerate() {
        assert!(
            row.get("ok_pct").is_none(),
            "row[{i}] must not contain old field 'ok_pct'; row={row}"
        );
    }
}

/// Old field name "out_per_call_bytes" must not appear in any row.
#[test]
fn counterexample_old_out_per_call_bytes_absent() {
    let ndjson = fresh("ls", 6, true, &["ls"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["rows"].as_array().expect("rows must be array");
    for (i, row) in rows.iter().enumerate() {
        assert!(
            row.get("out_per_call_bytes").is_none(),
            "row[{i}] must not contain old field 'out_per_call_bytes'; row={row}"
        );
    }
}

/// Old field name "retries" must not appear in any row; new name is "retry_cluster_count".
#[test]
fn counterexample_old_retries_field_absent() {
    let ndjson = fresh("test", 6, true, &["test"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["rows"].as_array().expect("rows must be array");
    for (i, row) in rows.iter().enumerate() {
        assert!(
            row.get("retries").is_none(),
            "row[{i}] must not contain old field 'retries'; row={row}"
        );
        // and the new field must be present
        assert!(
            row.get("retry_cluster_count").is_some(),
            "row[{i}] must contain new field 'retry_cluster_count'; row={row}"
        );
    }
}

/// Old label_key value "cmd" must not appear — table must emit "command", drill "argv_shape".
#[test]
fn counterexample_old_label_key_cmd_absent() {
    // table mode
    let ndjson = fresh("search", 6, true, &["search"], None);
    let home_t = home_with_events(&ndjson);

    let out_t = bin()
        .args(["stats", "--json"])
        .env("HOME", home_t.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out_t.status.success());
    let vt: serde_json::Value = serde_json::from_slice(&out_t.stdout).unwrap();
    assert_ne!(
        vt["label_key"].as_str(),
        Some("cmd"),
        "table mode must NOT emit label_key=\"cmd\"; got {:?}",
        vt["label_key"]
    );
    assert_eq!(
        vt["label_key"].as_str(),
        Some("command"),
        "table mode must emit label_key=\"command\"; got {:?}",
        vt["label_key"]
    );

    // drill mode
    let ndjson2 = fresh("search", 6, true, &["search"], None);
    let home_d = home_with_events(&ndjson2);

    let out_d = bin()
        .args(["stats", "search", "--json"])
        .env("HOME", home_d.path())
        .output()
        .expect("run 8v stats search --json");

    assert!(out_d.status.success());
    let vd: serde_json::Value = serde_json::from_slice(&out_d.stdout).unwrap();
    assert_ne!(
        vd["label_key"].as_str(),
        Some("cmd"),
        "drill mode must NOT emit label_key=\"cmd\"; got {:?}",
        vd["label_key"]
    );
    assert_eq!(
        vd["label_key"].as_str(),
        Some("argv_shape"),
        "drill mode must emit label_key=\"argv_shape\"; got {:?}",
        vd["label_key"]
    );
}

// ── Counterexample: warnings surfacing ───────────────────────────────────────

/// A malformed (unparseable) line in events.ndjson must produce a
/// `warnings[].kind == "malformed_event_line"` entry with the correct `line_no`.
#[test]
fn stats_json_surfaces_malformed_event_line_warning() {
    // One valid event pair, then a junk line, then another valid pair.
    let now = now_ms();
    let good1 = {
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": "malformed_test_run_1",
            "timestamp_ms": now - 10_000,
            "version": "0.1.0",
            "caller": "cli",
            "command": "read",
            "argv": ["read"],
            "command_bytes": 4_u64,
            "command_token_estimate": 1_u64,
            "project_path": serde_json::Value::Null,
            "agent_info": serde_json::Value::Null,
            "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": "malformed_test_run_1",
            "timestamp_ms": now - 9_000,
            "output_bytes": 512_u64,
            "token_estimate": 128_u64,
            "duration_ms": 1000_u64,
            "success": true,
            "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
        });
        format!(
            "{}\n{}\n",
            serde_json::to_string(&started).unwrap(),
            serde_json::to_string(&completed).unwrap()
        )
    };
    // Line 3 is the junk line (1-indexed after 2 lines of good1).
    let junk = "THIS IS NOT JSON AT ALL\n";
    let ndjson = format!("{good1}{junk}");

    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "stats --json must exit 0 even with malformed line; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    let warnings = v["warnings"].as_array().expect("warnings must be array");
    let malformed: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("malformed_event_line"))
        .collect();
    assert!(
        !malformed.is_empty(),
        "warnings must contain at least one malformed_event_line entry; warnings: {warnings:?}"
    );
    // line_no must be 3 (the junk line)
    let line_no = malformed[0]["line_no"].as_u64().expect("line_no is u64");
    assert_eq!(
        line_no, 3,
        "malformed_event_line.line_no must be 3; got {line_no}"
    );
}

/// Two CommandStarted events with the same run_id must produce a
/// `warnings[].kind == "duplicate_started"` warning with the matching run_id.
#[test]
fn stats_json_surfaces_duplicate_started_warning() {
    let now = now_ms();
    let run_id = "dup_started_run_id_42";

    // First CommandStarted — goes into pending map.
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": now - 5_000,
        "version": "0.1.0",
        "caller": "cli",
        "command": "build",
        "argv": ["build"],
        "command_bytes": 5_u64,
        "command_token_estimate": 1_u64,
        "project_path": serde_json::Value::Null,
        "agent_info": serde_json::Value::Null,
        "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
    });
    // Second CommandStarted with same run_id before any CommandCompleted.
    // The aggregator checks pending.contains_key(run_id) and emits DuplicateStarted.
    let started2 = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": now - 4_000,
        "version": "0.1.0",
        "caller": "cli",
        "command": "build",
        "argv": ["build"],
        "command_bytes": 5_u64,
        "command_token_estimate": 1_u64,
        "project_path": serde_json::Value::Null,
        "agent_info": serde_json::Value::Null,
        "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
    });
    let ndjson = format!(
        "{}\n{}\n",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&started2).unwrap(),
    );

    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "must exit 0 even with duplicate started; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    let warnings = v["warnings"].as_array().expect("warnings must be array");
    let dup: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("duplicate_started"))
        .collect();
    assert!(
        !dup.is_empty(),
        "warnings must contain duplicate_started; warnings: {warnings:?}"
    );
    assert_eq!(
        dup[0]["run_id"].as_str(),
        Some(run_id),
        "duplicate_started.run_id must match; got {:?}",
        dup[0]["run_id"]
    );
}

// ── Counterexample: failure_hotspots ─────────────────────────────────────────

/// Two failed calls for the same command+argv_shape across two sessions must
/// produce exactly one failure_hotspot row with count=2.
#[test]
fn stats_json_failure_hotspots_cross_session() {
    let now = now_ms();
    let mut ndjson = String::new();
    for i in 0..2usize {
        let run_id = format!("hotspot_cross_{i}");
        let session = format!("ses_hotspot_cross_{:025}", i);
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": run_id,
            "timestamp_ms": now - 10_000 - i as i64 * 200,
            "version": "0.1.0",
            "caller": "cli",
            "command": "check",
            "argv": ["check", "."],
            "command_bytes": 7_u64,
            "command_token_estimate": 2_u64,
            "project_path": serde_json::Value::Null,
            "agent_info": serde_json::Value::Null,
            "session_id": session,
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": run_id,
            "timestamp_ms": now - 9_000 - i as i64 * 200,
            "output_bytes": 128_u64,
            "token_estimate": 32_u64,
            "duration_ms": 1000_u64,
            "success": false,
            "session_id": session,
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
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        !hotspots.is_empty(),
        "failure_hotspots must be non-empty for 2 failures; got: {hotspots:?}"
    );
    // Find the hotspot for command "check"
    let check_spot = hotspots
        .iter()
        .find(|h| h["command"].as_str() == Some("check"))
        .expect("failure_hotspot for 'check' must exist");
    let count = check_spot["count"].as_u64().expect("count is u64");
    assert_eq!(
        count, 2,
        "failure_hotspot count must be 2 for 2 cross-session failures; got {count}"
    );
}

/// 3 failures for the same (command, argv_shape): path A appears twice, path B once.
/// top_path must be the path-like argv token that appears most frequently (A),
/// and top_path_count must be 2.
#[test]
fn stats_json_failure_hotspots_top_path_frequency() {
    let now = now_ms();
    let mut ndjson = String::new();
    // path A fails twice, path B fails once
    let paths: &[(&str, bool)] = &[
        ("src/main.rs", true),
        ("src/main.rs", true),
        ("src/lib.rs", false),
    ];
    let session = "ses_hotspot_toppath_AAAAAAAAA";
    for (i, (path, _is_a)) in paths.iter().enumerate() {
        let run_id = format!("hotspot_top_{i}");
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": run_id,
            "timestamp_ms": now - 10_000 - i as i64 * 200,
            "version": "0.1.0",
            "caller": "cli",
            "command": "read",
            "argv": ["read", path],
            "command_bytes": 4_u64,
            "command_token_estimate": 1_u64,
            "project_path": serde_json::Value::Null,
            "agent_info": serde_json::Value::Null,
            "session_id": session,
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": run_id,
            "timestamp_ms": now - 9_000 - i as i64 * 200,
            "output_bytes": 64_u64,
            "token_estimate": 16_u64,
            "duration_ms": 500_u64,
            "success": false,
            "session_id": session,
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
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        !hotspots.is_empty(),
        "failure_hotspots must be non-empty; got: {hotspots:?}"
    );

    // Find a hotspot for "read" command
    let read_spot = hotspots
        .iter()
        .find(|h| h["command"].as_str() == Some("read"))
        .expect("failure_hotspot for 'read' must exist");

    // top_path_count must be 2 (src/main.rs appeared twice)
    let top_count = read_spot["top_path_count"]
        .as_u64()
        .expect("top_path_count is u64");
    assert_eq!(
        top_count, 2,
        "top_path_count must be 2 (src/main.rs appeared twice); got {top_count}; hotspot={read_spot}"
    );
}

/// When all events succeed, failure_hotspots must be an empty array.
#[test]
fn stats_json_failure_hotspots_empty_when_no_failures() {
    let ndjson = fresh("read", 6, true, &["read"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        hotspots.is_empty(),
        "failure_hotspots must be empty when all events succeed; got: {hotspots:?}"
    );
}

// ── Counterexample: envelope fields for by_agent + drill modes ────────────────

/// `stats --compare agent --json` must emit kind="by_agent" and label_key="agent".
#[test]
fn stats_json_by_agent_envelope() {
    // Need MCP events so agent rows are grouped. Use agent_info fixture.
    let ndjson = fresh("read", 6, true, &["read"], Some("claude-opus-4-5"));
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--compare", "agent", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats --compare agent --json");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    assert_eq!(
        v["kind"].as_str(),
        Some("by_agent"),
        "kind must be \"by_agent\" for --compare agent; got {:?}",
        v["kind"]
    );
    assert_eq!(
        v["label_key"].as_str(),
        Some("agent"),
        "label_key must be \"agent\" for --compare agent; got {:?}",
        v["label_key"]
    );
}

/// `stats <shape> --json` must emit kind="drill", shape=<shape>, label_key="argv_shape".
#[test]
fn stats_json_drill_envelope() {
    let ndjson = fresh("write", 6, true, &["write", "src/main.rs:10", "x"], None);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "write", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats write --json");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");

    assert_eq!(
        v["kind"].as_str(),
        Some("drill"),
        "kind must be \"drill\" for drill mode; got {:?}",
        v["kind"]
    );
    assert_eq!(
        v["shape"].as_str(),
        Some("write"),
        "shape must be \"write\" in drill mode; got {:?}",
        v["shape"]
    );
    assert_eq!(
        v["label_key"].as_str(),
        Some("argv_shape"),
        "label_key must be \"argv_shape\" in drill mode; got {:?}",
        v["label_key"]
    );
}

// ── Counterexample: plain-text rendering includes warnings + hotspots ─────────

/// Plain-text output must include a "warnings:" section when warnings exist.
#[test]
fn stats_plain_shows_warnings_section() {
    // Inject a malformed line so a warning is always emitted.
    let now = now_ms();
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": "plain_warn_run_1",
        "timestamp_ms": now - 5_000,
        "version": "0.1.0",
        "caller": "cli",
        "command": "ls",
        "argv": ["ls"],
        "command_bytes": 2_u64,
        "command_token_estimate": 1_u64,
        "project_path": serde_json::Value::Null,
        "agent_info": serde_json::Value::Null,
        "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
    });
    let completed = serde_json::json!({
        "event": "CommandCompleted",
        "run_id": "plain_warn_run_1",
        "timestamp_ms": now - 4_000,
        "output_bytes": 256_u64,
        "token_estimate": 64_u64,
        "duration_ms": 1000_u64,
        "success": true,
        "session_id": "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA",
    });
    let ndjson = format!(
        "{}\n{}\nNOT_JSON_GARBAGE\n",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&completed).unwrap(),
    );

    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("warnings:"),
        "plain output must contain 'warnings:' section when warnings exist; got:\n{stdout}"
    );
}

/// Plain-text output must include a "failure hotspots:" section when hotspots exist.
#[test]
fn stats_plain_shows_failure_hotspots_section() {
    let now = now_ms();
    let mut ndjson = String::new();
    let session = "ses_contract_AAAAAAAAAAAAAAAAAAAAAAAAA";
    for i in 0..3usize {
        let run_id = format!("plain_hotspot_run_{i}");
        let started = serde_json::json!({
            "event": "CommandStarted",
            "run_id": run_id,
            "timestamp_ms": now - 10_000 - i as i64 * 200,
            "version": "0.1.0",
            "caller": "cli",
            "command": "check",
            "argv": ["check", "."],
            "command_bytes": 7_u64,
            "command_token_estimate": 2_u64,
            "project_path": serde_json::Value::Null,
            "agent_info": serde_json::Value::Null,
            "session_id": session,
        });
        let completed = serde_json::json!({
            "event": "CommandCompleted",
            "run_id": run_id,
            "timestamp_ms": now - 9_000 - i as i64 * 200,
            "output_bytes": 128_u64,
            "token_estimate": 32_u64,
            "duration_ms": 1000_u64,
            "success": false,
            "session_id": session,
        });
        ndjson.push_str(&serde_json::to_string(&started).unwrap());
        ndjson.push('\n');
        ndjson.push_str(&serde_json::to_string(&completed).unwrap());
        ndjson.push('\n');
    }

    let home = home_with_events(&ndjson);
    let out = bin()
        .args(["stats"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v stats");

    assert!(
        out.status.success(),
        "must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("failure hotspots:"),
        "plain output must contain 'failure hotspots:' section when hotspots exist; got:\n{stdout}"
    );
}
