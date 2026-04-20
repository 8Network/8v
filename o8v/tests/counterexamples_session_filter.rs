// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Counterexample sweep: `--session` filter on `8v log` and `8v stats`.
//!
//! Naming convention:
//!   `_bug`   — assertion fails on current code; confirmed invariant violation.
//!   `_holds` — assertion passes; invariant confirmed correct.
//!
//! Attack vectors covered (12 total):
//!   1.  empty_session_id_not_matched_by_valid_session_holds
//!   2.  lowercase_session_id_rejected_by_parser_holds
//!   3.  whitespace_session_id_rejected_by_parser_holds
//!   4.  prefix_not_used_for_session_flag_holds
//!   5.  multi_session_orphan_isolation_holds
//!   6.  project_flag_rejected_by_clap_holds
//!   7.  since_non_default_with_session_emits_warning_holds
//!   8.  empty_events_file_session_exits_two_holds
//!   9.  missing_events_file_exits_failure_not_two_holds
//!  10.  session_no_match_exits_two_holds
//!  11.  too_long_session_id_rejected_holds
//!  12.  log_show_leaks_orphan_warnings_across_sessions_bug

use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers (match regression_orphan_session_filter.rs conventions)
// ---------------------------------------------------------------------------

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

fn home_no_events() -> TempDir {
    // .8v dir exists but no events.ndjson file
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    dir
}

fn home_empty_events() -> TempDir {
    home_with_events("")
}

fn make_orphan_started(session_id: &str, run_id: &str, command: &str) -> String {
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
    format!("{}\n", serde_json::to_string(&started).unwrap())
}

fn make_event_pair(
    session_id: &str,
    run_id: &str,
    command: &str,
    offset_ms: i64,
    output_bytes: u64,
) -> String {
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": 1_700_000_002_000_i64 + offset_ms,
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
        "timestamp_ms": 1_700_000_003_000_i64 + offset_ms,
        "output_bytes": output_bytes,
        "token_estimate": 128_u64,
        "duration_ms": 42_u64,
        "success": true,
        "session_id": session_id,
    });
    format!(
        "{}\n{}\n",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&completed).unwrap()
    )
}

// Valid session IDs (Crockford base32 = uppercase A-Z minus I/L/O/U, plus 0-9).
// 26 chars after "ses_" = valid ULID.
const SESSION_A: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA1";
const SESSION_B: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA2";
const SESSION_C: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA3";
// Differs only in last char (used for prefix-attack vector)
const SESSION_A_VARIANT: &str = "ses_01HZAAAAAAAAAAAAAAAAAAAAA9";

// ---------------------------------------------------------------------------
// Attack 1: Empty session_id in events must not be matched by a valid --session
// ---------------------------------------------------------------------------

/// Events with empty session_id must not be surfaced when filtering by a valid session.
#[test]
fn empty_session_id_not_matched_by_valid_session_holds() {
    // An event line with session_id = "" — should not match SESSION_A filter.
    let empty_session_event = serde_json::json!({
        "event": "CommandStarted",
        "run_id": "run_empty_ses",
        "timestamp_ms": 1_700_000_001_000_i64,
        "version": "0.1.0",
        "caller": "cli",
        "command": "check",
        "argv": ["check", "."],
        "command_bytes": 5_u64,
        "command_token_estimate": 1_u64,
        "project_path": null,
        "session_id": "",
    });
    let session_a_events = make_event_pair(SESSION_A, "run_a", "check", 0, 128);
    let events = format!(
        "{}\n{}\n",
        serde_json::to_string(&empty_session_event).unwrap(),
        session_a_events
    );
    let home = home_with_events(&events);

    let out = bin()
        .args(["log", "--json", "--session", SESSION_A])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session SESSION_A");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    // Must be SESSION_A's drill-in, not a blend including the empty-session event
    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        SESSION_A,
        "expected session A drill-in\ngot: {v}"
    );
    // The commands listed must all belong to session A (run_id = run_a)
    let commands = v["commands"].as_array().cloned().unwrap_or_default();
    for cmd in &commands {
        let rid = cmd["run_id"].as_str().unwrap_or("");
        assert_ne!(
            rid, "run_empty_ses",
            "empty-session command must not appear in session A output\ncmd: {cmd}"
        );
    }
}

// ---------------------------------------------------------------------------
// Attack 2: Lowercase session ID rejected at parse time (clap value_parser)
// ---------------------------------------------------------------------------

/// `--session ses_01hz...` (lowercase) must be a parse error (non-zero exit).
/// The ULID parser requires uppercase Crockford base32.
#[test]
fn lowercase_session_id_rejected_by_parser_holds() {
    let home = home_empty_events();
    // Lowercase version of SESSION_A suffix
    let lowercase_session = "ses_01hzaaaaaaaaaaaaaaaaaaaaa1";

    let out = bin()
        .args(["log", "--json", "--session", lowercase_session])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with lowercase session id");

    assert!(
        !out.status.success(),
        "lowercase session_id must be rejected; got exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 3: Whitespace in --session value must be a parse error (not trimmed)
// ---------------------------------------------------------------------------

/// `--session " ses_01HZ... "` (leading/trailing space) must not silently trim.
/// SessionId::try_from_raw sees the full string including spaces → fails MissingPrefix.
#[test]
fn whitespace_session_id_rejected_by_parser_holds() {
    let home = home_empty_events();
    let padded = format!(" {} ", SESSION_A);

    let out = bin()
        .args(["log", "--json", "--session", &padded])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with whitespace session id");

    assert!(
        !out.status.success(),
        "whitespace-padded session_id must be a parse error; got exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 4: --session uses exact match, not prefix match
// ---------------------------------------------------------------------------

/// `--session` with an ID that is a prefix of another must match the exact one only.
/// SESSION_A = "ses_01HZAAAAAAAAAAAAAAAAAAAAA1"
/// SESSION_A_VARIANT = "ses_01HZAAAAAAAAAAAAAAAAAAAAA9" (same prefix, different last char)
/// Filtering by SESSION_A must surface SESSION_A's session, not SESSION_A_VARIANT.
#[test]
fn prefix_not_used_for_session_flag_holds() {
    let session_a_events = make_event_pair(SESSION_A, "run_a", "check", 0, 256);
    let session_variant_events =
        make_event_pair(SESSION_A_VARIANT, "run_variant", "build", 10_000, 384);
    let events = format!("{}{}", session_a_events, session_variant_events);
    let home = home_with_events(&events);

    let out = bin()
        .args(["log", "--json", "--session", SESSION_A])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session SESSION_A");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for exact match\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        SESSION_A,
        "must be exact SESSION_A, not variant\ngot: {v}"
    );
    // Ensure variant's command does not appear
    let commands = v["commands"].as_array().cloned().unwrap_or_default();
    for cmd in &commands {
        let rid = cmd["run_id"].as_str().unwrap_or("");
        assert_ne!(
            rid, "run_variant",
            "session_a_variant's command must not appear in session A output"
        );
    }
}

// ---------------------------------------------------------------------------
// Attack 5: Multi-session orphan isolation (3 sessions)
// ---------------------------------------------------------------------------

/// Three sessions each with an orphan. `--session B` must show zero orphan warnings.
#[test]
fn multi_session_orphan_isolation_holds() {
    let a_orphan = make_orphan_started(SESSION_A, "run_orphan_a", "check");
    let b_events = make_event_pair(SESSION_B, "run_clean_b", "check", 0, 192);
    let c_orphan = make_orphan_started(SESSION_C, "run_orphan_c", "build");

    let all_events = format!("{}{}{}", a_orphan, b_events, c_orphan);
    let home = home_with_events(&all_events);

    let out = bin()
        .args(["log", "--json", "--session", SESSION_B])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session SESSION_B");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for session B\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        SESSION_B,
        "expected session B drill-in\ngot: {v}"
    );

    let warnings = v["warnings"].as_array().cloned().unwrap_or_default();
    let orphan_warnings: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("orphan_started"))
        .collect();
    assert!(
        orphan_warnings.is_empty(),
        "session B must show zero orphan warnings from A or C\nfound: {orphan_warnings:?}\nfull output: {v}"
    );
}

// ---------------------------------------------------------------------------
// Attack 6: --project flag does not exist on log or stats
// ---------------------------------------------------------------------------

/// `8v log --project foo` must be a clap parse error (non-zero exit).
#[test]
fn log_project_flag_rejected_by_clap_holds() {
    let home = home_empty_events();

    let out = bin()
        .args(["log", "--json", "--project", "myproject"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --project");

    assert!(
        !out.status.success(),
        "--project must be rejected by clap; got exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// `8v stats --project foo` must be a clap parse error (non-zero exit).
#[test]
fn stats_project_flag_rejected_by_clap_holds() {
    let home = home_empty_events();

    let out = bin()
        .args(["stats", "--json", "--project", "myproject"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --project");

    assert!(
        !out.status.success(),
        "--project must be rejected by clap; got exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 7: --since non-default with --session emits warning in JSON output
// ---------------------------------------------------------------------------

/// `8v stats --json --session <id> --since 1d` must include a flag_ignored_for_session warning.
#[test]
fn since_non_default_with_session_emits_warning_in_stats_holds() {
    let session_a_events = make_event_pair(SESSION_A, "run_a", "check", 0, 320);
    let home = home_with_events(&session_a_events);

    let out = bin()
        .args(["stats", "--json", "--session", SESSION_A, "--since", "1d"])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json --session SESSION_A --since 1d");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // May exit 0 or 2 (filtered_empty), but must produce valid JSON
    let v: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON\nerr: {e}\nstdout: {stdout}\nstderr: {stderr}"),
    };

    let warnings = v["warnings"].as_array().cloned().unwrap_or_default();
    let flag_ignored: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("flag_ignored_for_session"))
        .collect();
    assert!(
        !flag_ignored.is_empty(),
        "expected flag_ignored_for_session warning for --since with --session\nwarnings: {warnings:?}\nfull output: {v}"
    );
}

// ---------------------------------------------------------------------------
// Attack 8: Empty events file + --session → exit 2 (no match), not panic
// ---------------------------------------------------------------------------

/// Empty events.ndjson + `--session <id>` must exit 2 (no session found), not crash.
#[test]
fn empty_events_file_with_session_exits_two_holds() {
    let home = home_empty_events();

    let out = bin()
        .args(["log", "--json", "--session", SESSION_A])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with empty events");

    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code, 2,
        "empty events + --session must exit 2 (no session found)\ngot exit: {code}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 9: Missing events file → exit 2 indistinguishable from "no session" (BUG)
// ---------------------------------------------------------------------------

// BUG: Missing events.ndjson returns exit 2 with {"error":"no session found"} —
// identical to the "no session found" case. A missing file (I/O error) is
// semantically distinct from "file exists but session not in it". The caller
// cannot distinguish these two failure modes. Severity: LOW (operational
// observability; CI scripts treating exit 2 as "try another ID" will silently
// swallow I/O errors).
/// Missing events.ndjson must NOT produce the same exit code + JSON as "no session found".
/// Currently both return exit 2 + {"error":"no session found"} — caller cannot tell them apart.
#[ignore = "known bug: see test-audit-2026-04-18.md"]
#[test]
fn missing_events_file_indistinguishable_from_no_session_bug() {
    let home = home_no_events();

    // Case A: missing file
    let out_missing = bin()
        .args(["log", "--json", "--session", SESSION_A])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with missing events file");

    // Case B: file exists but session not found
    let other_events = make_event_pair(SESSION_B, "run_b", "check", 0, 160);
    let home_other = home_with_events(&other_events);
    let out_no_match = bin()
        .args(["log", "--json", "--session", SESSION_A])
        .env("_8V_HOME", home_other.path())
        .output()
        .expect("run 8v log with no matching session");

    let code_missing = out_missing.status.code().unwrap_or(-1);
    let code_no_match = out_no_match.status.code().unwrap_or(-1);
    let stdout_missing = String::from_utf8_lossy(&out_missing.stdout);
    let stdout_no_match = String::from_utf8_lossy(&out_no_match.stdout);

    // The two cases must produce DIFFERENT exit codes or DIFFERENT JSON payloads.
    // If they are identical, the caller cannot distinguish I/O failure from "session absent".
    assert!(
        code_missing != code_no_match || stdout_missing != stdout_no_match,
        "missing file (exit={code_missing}, stdout={stdout_missing}) must differ from \
         no-match (exit={code_no_match}, stdout={stdout_no_match}) — currently indistinguishable"
    );
}

// ---------------------------------------------------------------------------
// Attack 10: --session in JSON mode with no match → valid JSON + exit 2
// ---------------------------------------------------------------------------

/// Non-existent session ID → exit 2, stdout is valid JSON (LogReport::Empty rendering).
#[test]
fn session_no_match_exits_two_holds() {
    // Use a real session in events but query for a different one
    let session_a_events = make_event_pair(SESSION_A, "run_a", "check", 0, 448);
    let home = home_with_events(&session_a_events);

    let out = bin()
        .args(["log", "--json", "--session", SESSION_B])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json --session non-existent");

    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code,
        2,
        "`--session` with no match must exit 2\ngot exit: {code}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 11: Too-long session ID rejected at parse time
// ---------------------------------------------------------------------------

/// Session ID with 27-char suffix (one too many) must be rejected by value_parser.
#[test]
fn too_long_session_id_rejected_holds() {
    let home = home_empty_events();
    // 27 uppercase base32 chars after "ses_" → WrongLength
    let too_long = "ses_01HZAAAAAAAAAAAAAAAAAAAAA12";

    let out = bin()
        .args(["log", "--json", "--session", too_long])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log with too-long session id");

    assert!(
        !out.status.success(),
        "too-long session_id must be a parse error; got exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

// ---------------------------------------------------------------------------
// Attack 12: `log show <id>` leaks orphan warnings from OTHER sessions
// ---------------------------------------------------------------------------

// BUG: `log show <id>` passes `all_warnings` unfiltered to build_drill_report.
// When session A has an orphan and we `log show B`, session A's orphan_started
// warning appears in session B's output. This is structurally asymmetric with
// `log --session B`, which filters warnings by session_run_ids.
// Severity: MEDIUM — misleading output; user sees orphan warnings for commands
// they never ran in this session.

/// `log show <id>` must not surface orphan warnings from other sessions.
/// This test is EXPECTED TO FAIL on current code (BUG #12).
#[ignore = "known bug: see test-audit-2026-04-18.md"]
#[test]
fn log_show_leaks_orphan_warnings_across_sessions_bug() {
    // Session A: orphan CommandStarted (no CommandCompleted)
    let session_a_orphan = make_orphan_started(SESSION_A, "run_orphan_a", "check");
    // Session B: clean pair
    let session_b_events = make_event_pair(SESSION_B, "run_clean_b", "build", 0, 224);

    let all_events = format!("{}{}", session_a_orphan, session_b_events);
    let home = home_with_events(&all_events);

    // Use `log show` (prefix-match subcommand) targeting session B
    let out = bin()
        .args(["log", "--json", "show", SESSION_B])
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v log --json show SESSION_B");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected exit 0 for log show SESSION_B\nstdout: {stdout}\nstderr: {stderr}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    // Confirm we got session B's drill-in
    assert_eq!(
        v["session_id"].as_str().unwrap_or(""),
        SESSION_B,
        "expected session B drill-in\ngot: {v}"
    );

    // The warnings must NOT contain session A's orphan
    let warnings = v["warnings"].as_array().cloned().unwrap_or_default();
    let orphan_warnings: Vec<_> = warnings
        .iter()
        .filter(|w| w["kind"].as_str() == Some("orphan_started"))
        .collect();

    assert!(
        orphan_warnings.is_empty(),
        "log show must not leak session A's orphan into session B output\n\
         found: {orphan_warnings:?}\nfull output: {v}"
    );
}
