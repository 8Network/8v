// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial counterexample tests for the stats v2 pipeline.
//!
//! Each test maps to one attack vector from the review.
//!
//! **Protocol:**
//! - Tests marked `#[ignore]` are confirmed bugs found via failing-first methodology.
//! - Passing tests (no `#[ignore]`) confirm the invariant holds.
//! - NO bugs are fixed in this file — only documented.
//!
//! Scope: warning.rs, stats_report.rs, aggregator.rs, stats.rs, stats_histogram.rs

use o8v::aggregator::{aggregate_events, ArgvNormalizer};
use o8v_core::caller::Caller;
use o8v_core::events::{CommandCompleted, CommandStarted, Event};
use o8v_core::types::{SessionId, Warning, WarningSink};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A unique session id for tests (ses_ + 25 uppercase ASCII chars).
fn sess(suffix: &str) -> String {
    // Pad to exactly 25 chars after "ses_"
    let base = format!("ses_{suffix}");
    let need = 29usize.saturating_sub(base.len()); // 4 + 25 = 29
    format!("{base}{:0>width$}", "", width = need)
}

fn make_started(run_id: &str, session: &str, command: &str, argv: Vec<&str>) -> Event {
    let mut ev = CommandStarted::new(
        run_id.to_string(),
        Caller::Cli,
        command,
        argv.into_iter().map(String::from).collect(),
        None,
    );
    // Override the session_id so tests can control it.
    ev.session_id = SessionId::from_raw_unchecked(session.to_string());
    Event::CommandStarted(ev)
}

fn make_completed(run_id: &str, session: &str, output_bytes: u64, success: bool) -> Event {
    let mut ev = CommandCompleted::new(run_id.to_string(), output_bytes, 10, success);
    ev.session_id = SessionId::from_raw_unchecked(session.to_string());
    Event::CommandCompleted(ev)
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn home_with_events(events_ndjson: &str) -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let dot_8v = dir.path().join(".8v");
    fs::create_dir_all(&dot_8v).expect("create .8v dir");
    fs::write(dot_8v.join("events.ndjson"), events_ndjson).expect("write events.ndjson");
    dir
}

struct NdjsonTiming {
    success: bool,
    timestamp_ms: i64,
    duration_ms: u64,
    output_bytes: u64,
}

/// Build a minimal NDJSON pair for one command invocation.
fn ndjson_pair(
    run_id: &str,
    session_id: &str,
    command: &str,
    argv: &[&str],
    timing: NdjsonTiming,
) -> String {
    let NdjsonTiming {
        success,
        timestamp_ms,
        duration_ms,
        output_bytes,
    } = timing;
    let argv_owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    let started = serde_json::json!({
        "event": "CommandStarted",
        "run_id": run_id,
        "timestamp_ms": timestamp_ms,
        "version": "0.1.0",
        "caller": "cli",
        "command": command,
        "argv": argv_owned,
        "command_bytes": command.len() as u64,
        "command_token_estimate": (command.len() / 4) as u64,
        "project_path": serde_json::Value::Null,
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

// ── Attack #1: hotspot sort tie-break missing argv_shape ─────────────────────
// BUG: sort is `count DESC, command ASC` — missing third key `argv_shape ASC`.
// Two hotspots with same count and command but different argv_shapes produce
// non-deterministic output across runs.
//
// This cannot be unit-tested (compute_failure_hotspots is private), so we
// drive it via the CLI binary and assert stable ordering across N iterations.
#[test]
fn hotspot_sort_ties_argv_shape_is_nondeterministic() {
    // Two failure clusters: same command "read", different argv_shapes ("<abs>" vs "src/<str>"),
    // same count (3 failures each).
    let now_ms = now_ms();
    let session = "ses_HOTSPOT1AAAAAAAAAAAAAAAAAAA";

    let mut ndjson = String::new();
    // Cluster A: "read" with path "/abs/path/to/file.rs" → argv_shape = "<abs>"
    for i in 0..3u32 {
        ndjson.push_str(&ndjson_pair(
            &format!("run_a_{i}"),
            session,
            "read",
            &["/abs/path/to/file.rs"],
            NdjsonTiming {
                success: false,
                timestamp_ms: now_ms + i as i64 * 1000,
                duration_ms: 10,
                output_bytes: 128 + i as u64 * 64,
            },
        ));
    }
    // Cluster B: "read" with no path token → argv_shape = "read" (no path)
    // Use a different argv that still parses as "read" command
    for i in 0..3u32 {
        ndjson.push_str(&ndjson_pair(
            &format!("run_b_{i}"),
            session,
            "read",
            &["src/main.rs"],
            NdjsonTiming {
                success: false,
                timestamp_ms: now_ms + 10_000 + i as i64 * 1000,
                duration_ms: 10,
                output_bytes: 256 + i as u64 * 32,
            },
        ));
    }

    let home = home_with_events(&ndjson);

    // Run 20 times and collect the order of argv_shapes in failure_hotspots.
    // A correct implementation must always produce the same order.
    let mut orders: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..20 {
        let out = bin()
            .args(["stats", "--json"])
            .current_dir(home.path())
            .env("_8V_HOME", home.path())
            .output()
            .expect("run 8v stats --json");
        let v: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
        if let Some(hotspots) = v["failure_hotspots"].as_array() {
            let order: String = hotspots
                .iter()
                .filter_map(|h| h["argv_shape"].as_str().map(str::to_string))
                .collect::<Vec<_>>()
                .join(",");
            orders.insert(order);
        }
    }

    // Bug: more than one distinct order observed → non-deterministic
    assert_eq!(
        orders.len(),
        1,
        "hotspot sort is non-deterministic: observed {} distinct orderings: {:?}",
        orders.len(),
        orders
    );
}

// ── Attack #2: top_path tie non-deterministic ─────────────────────────────────
// BUG: `paths.into_iter().max_by_key(|(_, c)| *c)` — HashMap-backed.
// Two paths with equal frequency → non-deterministic winner.
#[test]
fn top_path_ties_are_nondeterministic() {
    let now_ms = now_ms();
    let session = "ses_TOPPATH1AAAAAAAAAAAAAAAAAA";

    let mut ndjson = String::new();
    // Two different paths, each appearing once in failure argv, same command+argv_shape.
    ndjson.push_str(&ndjson_pair(
        "run_p1",
        session,
        "check",
        &["/alpha/path/file.rs"],
        NdjsonTiming {
            success: false,
            timestamp_ms: now_ms,
            duration_ms: 10,
            output_bytes: 256,
        },
    ));
    ndjson.push_str(&ndjson_pair(
        "run_p2",
        session,
        "check",
        &["/beta/path/file.rs"],
        NdjsonTiming {
            success: false,
            timestamp_ms: now_ms + 1000,
            duration_ms: 10,
            output_bytes: 384,
        },
    ));

    let home = home_with_events(&ndjson);

    let mut top_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..20 {
        let out = bin()
            .args(["stats", "--json"])
            .current_dir(home.path())
            .env("_8V_HOME", home.path())
            .output()
            .expect("run 8v stats --json");
        let v: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
        if let Some(hotspots) = v["failure_hotspots"].as_array() {
            for h in hotspots {
                if let Some(p) = h["top_path"].as_str() {
                    top_paths.insert(p.to_string());
                }
            }
        }
    }

    assert_eq!(
        top_paths.len(),
        1,
        "top_path selection non-deterministic: saw {:?}",
        top_paths
    );
}

// ── Attack #22a: OrphanCompleted warning never emitted ────────────────────────
// BUG: A CommandCompleted with no matching CommandStarted is silently ignored.
// aggregate_events comment says "orphan completed, ignore" — no Warning::OrphanCompleted.
#[test]
fn orphan_completed_no_warning_emitted() {
    let session = sess("ORPHANC");
    // Emit only a CommandCompleted, no CommandStarted with this run_id.
    let events = vec![make_completed("run_orphan_c", &session, 256, true)];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let _sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);
    let warnings = sink.into_inner();

    let has_orphan_completed = warnings
        .iter()
        .any(|w| matches!(w, Warning::OrphanCompleted { .. }));

    assert!(
        has_orphan_completed,
        "expected Warning::OrphanCompleted but got: {:?}",
        warnings
    );
}

// ── Attack #22b: OrphanStarted warning never emitted ─────────────────────────
// BUG: CommandStarted with no matching CommandCompleted is flushed to completed
// list as CommandRecord { completed: None } but no Warning::OrphanStarted is pushed.
#[test]
fn orphan_started_no_warning_emitted() {
    let session = sess("ORPHANS");
    // Only a CommandStarted — no matching CommandCompleted.
    let events = vec![make_started(
        "run_orphan_s",
        &session,
        "check",
        vec!["check", "."],
    )];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let _sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);
    let warnings = sink.into_inner();

    let has_orphan_started = warnings
        .iter()
        .any(|w| matches!(w, Warning::OrphanStarted { .. }));

    assert!(
        has_orphan_started,
        "expected Warning::OrphanStarted but got: {:?}",
        warnings
    );
}

// ── Attack #22c: DuplicateCompleted warning never emitted ─────────────────────
// BUG: Two CommandCompleted for the same run_id — the second is silently dropped.
// find_session_for_run_id searches the pending map; once the first Completed removes
// the run_id, the second finds nothing and is dropped with no warning.
#[test]
fn duplicate_completed_no_warning_emitted() {
    let session = sess("DUPCOMP");
    let events = vec![
        make_started("run_dup", &session, "read", vec!["read", "a.rs"]),
        make_completed("run_dup", &session, 256, true),
        // Second Completed for the same run_id
        make_completed("run_dup", &session, 128, false),
    ];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let _sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);
    let warnings = sink.into_inner();

    let has_dup_completed = warnings
        .iter()
        .any(|w| matches!(w, Warning::DuplicateCompleted { .. }));

    assert!(
        has_dup_completed,
        "expected Warning::DuplicateCompleted but got: {:?}",
        warnings
    );
}

// ── Attack #4: DuplicateStarted IS emitted ───────────────────────────────────
// Verified invariant: two CommandStarted with same run_id → Warning::DuplicateStarted.
#[test]
fn duplicate_started_warning_is_emitted() {
    let session = sess("DUPSTART");
    let events = vec![
        make_started("run_dup_s", &session, "read", vec!["read", "a.rs"]),
        // Second Started with same run_id
        make_started("run_dup_s", &session, "read", vec!["read", "a.rs"]),
        make_completed("run_dup_s", &session, 256, true),
    ];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let _sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);
    let warnings = sink.into_inner();

    let has_dup_started = warnings
        .iter()
        .any(|w| matches!(w, Warning::DuplicateStarted { .. }));

    assert!(
        has_dup_started,
        "expected Warning::DuplicateStarted but got: {:?}",
        warnings
    );
}

// ── Attack #5: EmptySessionId IS emitted ─────────────────────────────────────
// Verified invariant: a CommandStarted with empty session_id → Warning::EmptySessionId.
#[test]
fn empty_session_id_warning_is_emitted() {
    let events = vec![make_started(
        "run_empty_sess",
        "",
        "read",
        vec!["read", "a.rs"],
    )];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let _sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);
    let warnings = sink.into_inner();

    let has_empty = warnings
        .iter()
        .any(|w| matches!(w, Warning::EmptySessionId { .. }));

    assert!(
        has_empty,
        "expected Warning::EmptySessionId but got: {:?}",
        warnings
    );
}

// ── Attack #6: Single-session aggregate contains all records ──────────────────
// Verified: all events in one session are grouped together.
#[test]
fn single_session_collects_all_records() {
    let session = sess("SINGLESESS");
    let n = 5usize;
    let mut events = Vec::new();
    for i in 0..n {
        events.push(make_started(
            &format!("run_ss_{i}"),
            &session,
            "read",
            vec!["read", "x.rs"],
        ));
        events.push(make_completed(
            &format!("run_ss_{i}"),
            &session,
            128 + i as u64 * 64,
            true,
        ));
    }

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);

    assert_eq!(sessions.len(), 1, "expected exactly one session");
    assert_eq!(
        sessions[0].commands.len(),
        n,
        "expected {n} commands in the session"
    );
}

// ── Attack #7: Two sessions produce two aggregates ────────────────────────────
// Verified: events in distinct session_ids produce distinct SessionAggregates.
#[test]
fn two_sessions_produce_two_aggregates() {
    let sess_a = sess("SESSIONAAA");
    let sess_b = sess("SESSIONBBB");

    let events = vec![
        make_started("run_a", &sess_a, "read", vec!["read", "a.rs"]),
        make_completed("run_a", &sess_a, 256, true),
        make_started("run_b", &sess_b, "check", vec!["check", "."]),
        make_completed("run_b", &sess_b, 512, true),
    ];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);

    assert_eq!(sessions.len(), 2, "expected two sessions");
}

// ── Attack #8: orphan-started record appears in session.commands ──────────────
// Verified: a CommandStarted with no Completed is still in commands as incomplete.
#[test]
fn orphan_started_record_appears_in_commands() {
    let session = sess("ORPHANSREC");
    let events = vec![make_started("run_o", &session, "build", vec!["build", "."])];

    let mut normalizer = ArgvNormalizer::new();
    let mut sink = WarningSink::new();
    let sessions = aggregate_events(&events, 30_000, &mut normalizer, &mut sink);

    assert_eq!(sessions.len(), 1, "expected one session");
    assert_eq!(sessions[0].commands.len(), 1, "expected one command record");
    assert!(
        sessions[0].commands[0].completed.is_none(),
        "orphan command record must have completed=None"
    );
}

// ── Attack #13: stats --json exits 0 with empty event file ───────────────────
// Verified: an empty events.ndjson does not crash the binary.
#[test]
fn stats_json_empty_events_exits_zero() {
    let home = home_with_events("");

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(
        out.status.success(),
        "expected exit 0 for empty events; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");
    assert_eq!(v["kind"].as_str(), Some("table"), "expected kind=table");
    // rows may contain the stats invocation itself — just verify it's an array.
    assert!(v["rows"].is_array(), "rows must be an array");
}

// ── Attack #14: stats --json top-level has required fields ───────────────────
// Verified: output always contains kind, label_key, rows, warnings, failure_hotspots.
#[test]
fn stats_json_top_level_fields_present() {
    let ndjson = ndjson_pair(
        "run_fields",
        "ses_FIELDS1AAAAAAAAAAAAAAAAAA",
        "read",
        &["read", "src/main.rs"],
        NdjsonTiming {
            success: true,
            timestamp_ms: now_ms(),
            duration_ms: 15,
            output_bytes: 256,
        },
    );
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    for field in &["kind", "label_key", "rows", "warnings", "failure_hotspots"] {
        assert!(
            !v[field].is_null(),
            "expected top-level field '{}' to be present",
            field
        );
    }
}

// ── Attack #15: failure_hotspots only for failed commands ────────────────────
// Verified: 100% success rate means failure_hotspots is empty.
#[test]
fn failure_hotspots_empty_when_all_succeed() {
    let now_ms = now_ms();
    let session = "ses_HOTSPOT0AAAAAAAAAAAAAAAAAAA";
    let mut ndjson = String::new();
    for i in 0..5u32 {
        ndjson.push_str(&ndjson_pair(
            &format!("run_succ_{i}"),
            session,
            "read",
            &["read", "src/main.rs"],
            NdjsonTiming {
                success: true, // all succeed
                timestamp_ms: now_ms + i as i64 * 1000,
                duration_ms: 10,
                output_bytes: 128 + i as u64 * 32,
            },
        ));
    }
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        hotspots.is_empty(),
        "expected empty failure_hotspots for all-success events"
    );
}

// ── Attack #19: failure_hotspots appear for failed commands ──────────────────
// Verified: failed commands produce entries in failure_hotspots.
#[test]
fn failure_hotspots_populated_for_failures() {
    let now_ms = now_ms();
    let session = "ses_HOTSPOTFAAAAAAAAAAAAAAAAA";
    let mut ndjson = String::new();
    for i in 0..3u32 {
        ndjson.push_str(&ndjson_pair(
            &format!("run_fail_{i}"),
            session,
            "check",
            &["check", "."],
            NdjsonTiming {
                success: false,
                timestamp_ms: now_ms - (2 - i as i64) * 1000,
                duration_ms: 10,
                output_bytes: 256 + i as u64 * 64,
            },
        ));
    }
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        !hotspots.is_empty(),
        "expected at least one failure_hotspot for failed commands"
    );

    let h = &hotspots[0];
    assert_eq!(
        h["command"].as_str(),
        Some("check"),
        "hotspot command must be 'check'"
    );
    let count = h["count"].as_u64().expect("count must be u64");
    assert_eq!(count, 3, "expected count=3 failures");
}

// ── Attack #20: failure_hotspots cap at 10 ───────────────────────────────────
// Verified: even with >10 distinct (command, argv_shape) failure pairs, at most 10 appear.
#[test]
fn failure_hotspots_capped_at_ten() {
    let now_ms = now_ms();
    let session = "ses_HOTSPOT10AAAAAAAAAAAAAAAA";
    let mut ndjson = String::new();
    // 15 distinct commands, each failing once
    for i in 0..15u32 {
        let cmd = format!("cmd{i:02}");
        ndjson.push_str(&ndjson_pair(
            &format!("run_cap_{i}"),
            session,
            &cmd,
            &[],
            NdjsonTiming {
                success: false,
                timestamp_ms: now_ms + i as i64 * 1000,
                duration_ms: 10,
                output_bytes: 128 + i as u64 * 16,
            },
        ));
    }
    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        hotspots.len() <= 10,
        "failure_hotspots must be capped at 10; got {}",
        hotspots.len()
    );
}

// ── Attack #F3: flag-value content appears verbatim in argv_shape ─────────────
// §6.1 contract: tokens following content-carrying flags (--insert, --find,
// --replace, --append) are user-supplied strings and MUST normalize to `<str>`.
// BUG: `normalize_argv` returns `tok.clone()` for flag-value tokens instead of
// `"<str>".to_string()`, leaking arbitrary content (e.g. large markdown text)
// into argv_shape stored in failure_hotspots output.
#[test]
fn flag_value_after_append_normalizes_to_str_in_hotspot() {
    let now_ms = now_ms();
    let session = "ses_FLAGVAL1AAAAAAAAAAAAAAAAAAA";

    // argv: write README.md --append <large content that must not leak>
    let ndjson = ndjson_pair(
        "run_flag_val",
        session,
        "write",
        &[
            "README.md",
            "--append",
            "lots of text here that should be stripped — never appear verbatim",
        ],
        NdjsonTiming {
            success: false,
            timestamp_ms: now_ms,
            duration_ms: 10,
            output_bytes: 1024,
        },
    );

    let home = home_with_events(&ndjson);

    let out = bin()
        .args(["stats", "--json"])
        .current_dir(home.path())
        .env("_8V_HOME", home.path())
        .output()
        .expect("run 8v stats --json");

    assert!(out.status.success(), "8v stats must exit 0");
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be valid JSON");

    let hotspots = v["failure_hotspots"]
        .as_array()
        .expect("failure_hotspots must be array");
    assert!(
        !hotspots.is_empty(),
        "expected at least one failure hotspot"
    );

    let argv_shape = hotspots[0]["argv_shape"]
        .as_str()
        .expect("argv_shape must be a string");

    // §6.1: flag value must be replaced with <str>
    assert!(
        argv_shape.contains("<str>"),
        "flag value must normalize to <str>; got: {argv_shape:?}"
    );
    // The verbatim content must NOT appear in argv_shape
    assert!(
        !argv_shape.contains("lots of text"),
        "raw flag value must not appear in argv_shape; got: {argv_shape:?}"
    );
}
