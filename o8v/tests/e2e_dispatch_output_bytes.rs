// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Regression test: CommandCompleted events written to events.ndjson must carry
//! a nonzero output_bytes value when the command produces output.
//!
//! This is a failing test for bug F3: output_bytes=0 in real CLI events.

use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Run a real CLI command that produces nonzero stdout, then assert that the
/// CommandCompleted event written to events.ndjson has output_bytes > 0.
///
/// Setup:
///   - project: TempProject::rust_passing() gives a valid Cargo.toml so
///     ProjectRoot::new() succeeds → StorageSubscriber is subscribed.
///   - _8V_HOME: isolated TempDir so events go to a known location.
///   - current_dir: the project root (required for workspace resolution).
#[test]
fn completed_event_carries_nonzero_output_bytes() {
    // A valid Rust project so resolve_workspace() succeeds and StorageSubscriber
    // is subscribed to the EventBus.
    let project = o8v_testkit::TempProject::rust_passing();

    // Isolated home dir — StorageDir::open() reads _8V_HOME first.
    let home = TempDir::new().expect("create temp home dir");

    // Run `8v ls <path>` — always produces nonzero stdout on a non-empty project.
    let out = bin()
        .args(["ls", project.path().to_str().unwrap()])
        .current_dir(project.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v ls");

    assert!(
        out.status.success(),
        "8v ls must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Confirm the command actually produced output — if this fires, the test
    // premise is wrong (command produced no bytes).
    let stdout_len = out.stdout.len();
    assert!(
        stdout_len > 0,
        "8v ls must write nonzero bytes to stdout; got 0"
    );

    // Read the events file written during the CLI run.
    let events_path = home.path().join(".8v").join("events.ndjson");
    assert!(
        events_path.exists(),
        "events.ndjson must exist at {:?}; workspace resolution may have failed",
        events_path
    );

    let raw = std::fs::read_to_string(&events_path).expect("read events.ndjson");

    // Find the CommandCompleted event.
    // Parse each line; skip parse failures by collecting only Ok results.
    let events: Vec<serde_json::Value> = raw
        .lines()
        .flat_map(serde_json::from_str::<serde_json::Value>)
        .collect();
    let completed: &serde_json::Value = events
        .iter()
        .find(|v| v["event"].as_str() == Some("CommandCompleted"))
        .expect("events.ndjson must contain a CommandCompleted event");

    let output_bytes = completed["output_bytes"]
        .as_u64()
        .expect("output_bytes must be a u64 in CommandCompleted");

    assert!(
        output_bytes > 0,
        "CommandCompleted.output_bytes must be > 0 for a command that produced {} stdout bytes; got output_bytes={}",
        stdout_len,
        output_bytes
    );
}
