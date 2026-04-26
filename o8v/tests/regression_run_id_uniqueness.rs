// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Regression test: each CLI invocation must emit a unique `run_id`.
//!
//! Before the fix, `dispatch()` derived `run_id` from `TaskId::to_string()`,
//! which yields `"task-1"` for the first command in every new process.
//! Two separate `8v` invocations therefore emitted identical `run_id` values,
//! causing event-store collisions.
//!
//! After the fix, `dispatch()` calls `mint_run_id()` (ULID), so each process
//! gets a globally-unique identifier.

use std::fs;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn home() -> TempDir {
    TempDir::new().expect("create temp dir")
}

fn read_events(dir: &TempDir) -> Vec<serde_json::Value> {
    let path = dir.path().join(".8v").join("events.ndjson");
    if !path.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(&path).expect("read events.ndjson");
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("parse event line"))
        .collect()
}

fn run_log(dir: &TempDir) {
    let status = bin()
        .arg("log")
        .env("_8V_HOME", dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn 8v log");
    // log exits 0 (history present) or 2 (empty history); either is fine.
    let _ = status;
}

#[test]
fn each_cli_run_emits_a_unique_run_id() {
    let dir = home();

    // Two separate process invocations, same isolated home.
    run_log(&dir);
    run_log(&dir);

    let events = read_events(&dir);
    assert!(
        !events.is_empty(),
        "events.ndjson must contain at least one event after two runs"
    );

    // Collect run_id values from CommandStarted events only.
    let run_ids: Vec<String> = events
        .iter()
        .filter(|ev| ev.get("event").and_then(|t| t.as_str()) == Some("CommandStarted"))
        .filter_map(|ev| ev.get("run_id").and_then(|r| r.as_str()).map(String::from))
        .collect();

    assert!(
        run_ids.len() >= 2,
        "expected at least 2 CommandStarted events (one per run), got: {run_ids:?}"
    );

    // All run_ids must be distinct — no two runs may share the same id.
    let first = &run_ids[0];
    let second = &run_ids[1];
    assert_ne!(
        first, second,
        "run_id must be unique per process; both runs produced {first:?}"
    );
}
