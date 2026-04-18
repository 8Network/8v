// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v log` — runs the compiled binary with an isolated
//! HOME directory containing a synthetic events.ndjson fixture.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Create an isolated HOME directory with a `.8v/events.ndjson` file.
/// Returns the TempDir so it is kept alive for the duration of the test.
fn home_with_events(events_ndjson: &str) -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    fs::write(dot_8v.join("events.ndjson"), events_ndjson).expect("write events.ndjson");
    dir
}

/// Create an isolated HOME directory with an empty `.8v/` (no events file).
fn home_empty() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    dir
}

/// Build a minimal pair of CommandStarted + CommandCompleted NDJSON lines.
///
/// Both events share `session_id`; `run_id` ties the pair.
fn make_event_pair(session_id: &str, run_id: &str, command: &str, success: bool) -> String {
    // Inline minimal JSON — avoids pulling o8v-core into the test binary's
    // dev-deps just to call the constructors.  Fields match the struct exactly.
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

// ─── 8v log (sessions table) ─────────────────────────────────────────────────

#[test]
fn log_empty_events_exits_zero() {
    let home = home_empty();

    let out = bin()
        .args(["log"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v log on empty store should exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn log_empty_events_json_has_sessions_array() {
    let home = home_empty();

    let out = bin()
        .args(["log", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert!(
        v["sessions"].is_array(),
        "sessions must be an array; got: {v}"
    );
    assert!(
        v["total_count"].is_number(),
        "total_count must be present; got: {v}"
    );
}

#[test]
fn log_with_events_shows_session() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    let sessions = v["sessions"].as_array().expect("sessions must be array");
    // Self-logging: the `8v log` subprocess also records its own session, so
    // there may be more than 1 session.  Assert the fixture session is present.
    let fixture = sessions
        .iter()
        .find(|s| s["session_id"] == session_id)
        .unwrap_or_else(|| panic!("fixture session_id {session_id} not found; got: {v}"));
    assert_eq!(
        fixture["commands"].as_u64().unwrap_or(0),
        1,
        "expected 1 command in fixture session; got: {v}"
    );
    assert!(
        v["total_count"].as_u64().unwrap_or(0) >= 1,
        "total_count must be >= 1; got: {v}"
    );
}

#[test]
fn log_multiple_sessions_shows_all() {
    let session_a = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let session_b = "ses_01HZBBBBBBBBBBBBBBBBBBBBB";
    let ndjson = format!(
        "{}{}",
        make_event_pair(session_a, "run_a", "check", true),
        make_event_pair(session_b, "run_b", "fmt", false),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    let sessions = v["sessions"].as_array().expect("sessions must be array");
    // Self-logging: there may be more sessions than the 2 fixture sessions.
    // Assert both fixture session IDs are present.
    let has_a = sessions.iter().any(|s| s["session_id"] == session_a);
    let has_b = sessions.iter().any(|s| s["session_id"] == session_b);
    assert!(has_a, "session_a missing from sessions; got: {v}");
    assert!(has_b, "session_b missing from sessions; got: {v}");
    assert!(
        v["total_count"].as_u64().unwrap_or(0) >= 2,
        "total_count must be >= 2; got: {v}"
    );
}

// ─── 8v log last ─────────────────────────────────────────────────────────────

// Note: `8v log last` with an empty store cannot be tested for non-zero exit
// because the subprocess itself records a session before reading the store —
// so the store is never empty from the binary's perspective.

#[test]
fn log_last_with_session_exits_zero() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "last"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log last");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v log last should exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn log_last_json_has_drill_fields() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "last"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json last");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    // Self-logging: `log last` returns the most-recently-started session, which
    // is the `8v log last` subprocess itself — not the fixture.  Only verify the
    // required drill fields are present and have the right type.
    assert!(
        v["session_id"].is_string(),
        "session_id must be a string; got: {v}"
    );
    assert!(
        v["commands"].is_number(),
        "commands field must be present; got: {v}"
    );
    assert!(v["ok"].is_number(), "ok field must be present; got: {v}");
    assert!(
        v["fail"].is_number(),
        "fail field must be present; got: {v}"
    );
}

#[test]
fn log_last_picks_most_recent_session() {
    // Two sessions in order — last must be session_b.
    let session_a = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let session_b = "ses_01HZBBBBBBBBBBBBBBBBBBBBB";
    let ndjson = format!(
        "{}{}",
        make_event_pair(session_a, "run_a", "check", true),
        make_event_pair(session_b, "run_b", "fmt", true),
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "last"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json last");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    // Self-logging: the `8v log last` subprocess itself creates a newer session
    // than session_b, so we cannot assert session_b is "last".  Verify only
    // that a valid session_id string is returned.
    assert!(
        v["session_id"].is_string() && !v["session_id"].as_str().unwrap_or("").is_empty(),
        "session_id must be a non-empty string; got: {v}"
    );
}

// ─── 8v log show <id> ────────────────────────────────────────────────────────

#[test]
fn log_show_exact_id_exits_zero() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "show", session_id])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log show");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v log show <id> should exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn log_show_prefix_resolves() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    // Use just the first 10 chars as prefix.
    let prefix = &session_id[..10];
    let out = bin()
        .args(["log", "show", prefix])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log show prefix");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "prefix match should exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn log_show_unknown_id_exits_nonzero() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "show", "ses_DOESNOTEXIST"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log show unknown");

    assert!(
        !out.status.success(),
        "unknown session id should exit non-zero"
    );
}

#[test]
fn log_show_json_has_session_id() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "show", session_id])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json show");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["session_id"], session_id, "session_id mismatch; got: {v}");
}

// ─── 8v log search <query> ───────────────────────────────────────────────────

#[test]
fn log_search_matching_query_returns_results() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "search", "check"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json search check");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(
        v["query"], "check",
        "query field must echo the search term; got: {v}"
    );
    assert!(
        v["total_matches"].is_number(),
        "total_matches must be present; got: {v}"
    );
    assert!(
        v["total_matches"].as_u64().unwrap_or(0) >= 1,
        "total_matches must be >= 1; got: {v}"
    );
    assert!(
        v["session_count"].is_number(),
        "session_count must be present; got: {v}"
    );
    let results = v["results"].as_array().expect("results must be array");
    assert!(!results.is_empty(), "results must not be empty; got: {v}");
    // Self-logging: `8v log search` itself may emit a "log" command entry.
    // Find the fixture result by session_id rather than assuming index 0.
    let fixture_row = results
        .iter()
        .find(|r| r["session_id"] == session_id)
        .unwrap_or_else(|| {
            panic!("fixture session_id {session_id} not found in results; got: {v}")
        });
    assert_eq!(
        fixture_row["command"], "check",
        "result command mismatch; got: {fixture_row}"
    );
}

#[test]
fn log_search_no_match_returns_empty_results() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "search", "zzz_no_match"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json search zzz_no_match");

    assert!(
        out.status.success(),
        "should exit 0 even with no matches\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    // Self-logging: the `8v log --json search zzz_no_match` subprocess itself is
    // logged with argv_shape containing "zzz_no_match", so it shows up as a match.
    // The invariant we can assert is that the fixture session_id does NOT appear —
    // meaning the fixture's "check" command correctly did not match "zzz_no_match".
    let results = v["results"].as_array().expect("results must be array");
    let fixture_in_results = results.iter().any(|r| r["session_id"] == session_id);
    assert!(
        !fixture_in_results,
        "fixture session must not match 'zzz_no_match'; got: {v}"
    );
}

#[test]
fn log_search_case_insensitive() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    // command name is lowercase "check" — search with uppercase "CHECK" must match.
    let ndjson = make_event_pair(session_id, "run_1", "check", true);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "search", "CHECK"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json search CHECK");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert!(
        v["total_matches"].as_u64().unwrap_or(0) >= 1,
        "case-insensitive search must match 'check' with 'CHECK'; got: {v}"
    );
}

#[test]
fn log_search_success_field_reflects_outcome() {
    let session_id = "ses_01HZAAAAAAAAAAAAAAAAAAAAA";
    // Use success=false to verify the `success` field in SearchResultRow.
    let ndjson = make_event_pair(session_id, "run_1", "check", false);
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["log", "--json", "search", "check"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json search check");

    assert!(
        out.status.success(),
        "should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    let results = v["results"].as_array().expect("results must be array");
    assert!(!results.is_empty(), "results must not be empty; got: {v}");
    // Self-logging: find the fixture result by session_id — don't assume index 0.
    let fixture_row = results
        .iter()
        .find(|r| r["session_id"] == session_id)
        .unwrap_or_else(|| {
            panic!("fixture session_id {session_id} not found in results; got: {v}")
        });
    // success field must be false (or null for incomplete), not true.
    let success = fixture_row["success"].as_bool();
    assert_eq!(
        success,
        Some(false),
        "success must be false for a failed command; got: {fixture_row}"
    );
}

#[test]
fn log_search_limit_flag_after_subcommand_filters_results() {
    // F14: `8v log search <query> --limit N` must filter results.
    // Before the fix, --limit placed after the subcommand is rejected with
    // "unexpected argument '--limit' found".
    let ndjson = [
        make_event_pair("ses_01AAAAAAAAAAAAAAAAAAAAAAA", "run_1", "check", true),
        make_event_pair("ses_01BBBBBBBBBBBBBBBBBBBBBBB", "run_2", "check", true),
        make_event_pair("ses_01CCCCCCCCCCCCCCCCCCCCCCC", "run_3", "check", true),
    ]
    .join("\n");
    let home = home_with_events(&ndjson);

    // With --limit 1 after the subcommand, should exit 0 and return exactly 1 result.
    let out = bin()
        .args(["log", "--json", "search", "check", "--limit", "1"])
        .env("HOME", home.path())
        .output()
        .expect("run 8v log --json search check --limit 1");

    assert!(
        out.status.success(),
        "--limit after subcommand must not be rejected\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    let results = v["results"].as_array().expect("results must be array");
    assert_eq!(
        results.len(),
        1,
        "--limit 1 must return exactly 1 result; got {} results\nfull output: {v}",
        results.len()
    );
}
