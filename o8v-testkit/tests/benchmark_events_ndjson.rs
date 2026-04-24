// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Integration test: benchmark events land in events.ndjson with correct fields.

use o8v_core::events::benchmark::{BenchmarkRunFinished, BenchmarkRunStarted};
use o8v_testkit::benchmark::{emit_benchmark_event, events_ndjson_path_with};
use tempfile::TempDir;

#[test]
fn benchmark_events_written_to_events_ndjson() {
    let dir = TempDir::new().expect("create tempdir");
    let path = dir.path().join(".8v").join("events.ndjson");

    let run_id = "test-run-abc123";

    // Emit BenchmarkRunStarted
    let started = BenchmarkRunStarted::new(
        run_id,
        "fix-go/8v",
        "fix-go",
        "8v",
        0,
        0,
        serde_json::Value::Null,
    );
    let started_json = serde_json::to_string(&started).expect("serialize started");
    emit_benchmark_event(&path, &started_json);

    // Emit BenchmarkRunFinished
    let finished = BenchmarkRunFinished::new(run_id, 1234, 0, true, 0.01, 1000, 100, 200, 5, 3);
    let finished_json = serde_json::to_string(&finished).expect("serialize finished");
    emit_benchmark_event(&path, &finished_json);

    // Read back and verify
    let content = std::fs::read_to_string(&path).expect("read events.ndjson");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "expected exactly 2 events");

    let ev1: serde_json::Value = serde_json::from_str(lines[0]).expect("parse line 1");
    let ev2: serde_json::Value = serde_json::from_str(lines[1]).expect("parse line 2");

    assert_eq!(ev1["event"], "BenchmarkRunStarted");
    assert_eq!(ev2["event"], "BenchmarkRunFinished");

    // run_id must match across both events
    assert_eq!(ev1["run_id"], ev2["run_id"]);
    assert_eq!(ev1["run_id"].as_str().unwrap(), run_id);

    // spot-check key fields
    assert_eq!(ev1["scenario"], "fix-go/8v");
    assert_eq!(ev1["arm"], "8v");
    assert_eq!(ev1["run_idx"], 0);
    assert_eq!(ev2["tests_pass"], true);
    assert_eq!(ev2["exit_code"], 0);
}

/// Tests for the `events_ndjson_path` resolver using dependency injection.
///
/// Uses `events_ndjson_path_with` so no process-environment mutation is needed —
/// safe in a parallel test runner.
#[test]
fn events_ndjson_path_resolver() {
    // ── branch 1: _8V_HOME takes priority ───────────────────────────────────
    let home_dir = TempDir::new().expect("create tempdir for _8V_HOME branch");
    let home_str = home_dir.path().to_str().unwrap().to_string();

    let path = events_ndjson_path_with(|key| match key {
        "_8V_HOME" => Some(home_str.clone()),
        "HOME" => Some("/should-not-be-used".to_string()),
        _ => None,
    });
    let expected = home_dir.path().join(".8v").join("events.ndjson");
    assert_eq!(
        path, expected,
        "events_ndjson_path must use _8V_HOME when set; got {path:?}"
    );

    // ── branch 2: fallback to HOME when _8V_HOME is absent ──────────────────
    let fallback_dir = TempDir::new().expect("create tempdir for HOME fallback branch");
    let fallback_str = fallback_dir.path().to_str().unwrap().to_string();

    let path = events_ndjson_path_with(|key| match key {
        "_8V_HOME" => None,
        "HOME" => Some(fallback_str.clone()),
        _ => None,
    });
    let expected = fallback_dir.path().join(".8v").join("events.ndjson");
    assert_eq!(
        path, expected,
        "events_ndjson_path must fall back to HOME when _8V_HOME is absent; got {path:?}"
    );
}
