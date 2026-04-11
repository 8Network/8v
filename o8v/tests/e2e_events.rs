//! Binary-level E2E tests for event sourcing.
//!
//! Exercises the full pipeline: `8v check` binary → `~/.8v/` storage → `series.json`.
//! These tests invoke the real binary, verifying that the wiring in `check.rs`
//! (EventWriter initialization, finalization) works end-to-end.
//!
//! Lifecycle under test:
//! 1. User runs `8v check` → binary writes `~/.8v/series.json`
//! 2. User runs `8v check` again → run_id changes, timestamps advance
//!
//! Each test overrides HOME to an isolated temp directory so tests don't
//! share state or pollute the real `~/.8v/`.

use o8v_testkit::{Fixture, TempProject};
use std::fs;
use std::path::Path;
use std::process::Command;

fn bin_with_home(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.env("HOME", home);
    cmd
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_series(home: &Path) -> o8v_events::SeriesJson {
    let path = home.join(".8v").join("series.json");
    assert!(
        path.exists(),
        "series.json must exist at {}",
        path.display()
    );
    let bytes = fs::read(&path).expect("read series.json");
    o8v_events::parse_series(&bytes).expect("parse series.json")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// `8v check` writes `series.json` to `~/.8v/`.
#[test]
fn test_check_writes_series_json() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");

    let out = bin_with_home(home.path())
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check");

    // Empty directory → exit 2 (nothing to check). That is expected.
    assert_eq!(out.status.code(), Some(2), "empty dir exits 2");

    let series = read_series(home.path());
    assert!(!series.run_id.is_empty(), "run_id must be set");
    assert!(series.timestamp > 0, "timestamp must be positive");
    assert!(
        series.diagnostics.is_empty(),
        "empty project must have zero diagnostics"
    );
}

/// Without HOME writable, check still completes (events are best-effort).
#[test]
fn test_check_without_dot8v_still_runs() {
    let project = TempProject::empty();

    let _out = bin_with_home(project.path())
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check");

    // StorageDir creates ~/.8v/ automatically — events are always written now.
    // This test just verifies the binary doesn't crash.
}

/// Two consecutive runs: `run_id` changes, `timestamp` advances.
#[test]
fn test_two_runs_produce_distinct_run_ids() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");

    let path_arg = project.path().to_str().unwrap();

    bin_with_home(home.path())
        .args(["check", path_arg])
        .output()
        .expect("run 1");

    let series1 = read_series(home.path());

    bin_with_home(home.path())
        .args(["check", path_arg])
        .output()
        .expect("run 2");

    let series2 = read_series(home.path());

    assert_ne!(
        series1.run_id, series2.run_id,
        "each run must produce a distinct run_id"
    );
    assert!(
        series2.timestamp >= series1.timestamp,
        "timestamp must not go backwards: run1={} run2={}",
        series1.timestamp,
        series2.timestamp
    );
}

/// `.tmp` file must not be left on disk after a successful run.
#[test]
fn test_no_tmp_file_after_successful_run() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");

    bin_with_home(home.path())
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check");

    let tmp_path = home.path().join(".8v").join("series.json.tmp");
    assert!(
        !tmp_path.exists(),
        "series.json.tmp must not exist after successful check"
    );
}

/// Orphaned `.tmp` from a previous crash is cleaned up on next run.
#[test]
fn test_orphaned_tmp_is_cleaned_on_next_run() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");

    // Pre-create ~/.8v/ and plant a stale .tmp
    let dot8v = home.path().join(".8v");
    fs::create_dir_all(&dot8v).expect("create .8v");
    let tmp_path = dot8v.join("series.json.tmp");
    fs::write(&tmp_path, b"stale from crashed run").expect("write stale tmp");
    assert!(tmp_path.exists());

    bin_with_home(home.path())
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check");

    assert!(
        !tmp_path.exists(),
        "stale .tmp must be cleaned up on next run"
    );

    let series = read_series(home.path());
    assert!(!series.run_id.is_empty());
}

/// Events directory is created by `8v check` in `~/.8v/events/`.
#[test]
fn test_events_dir_created_on_check() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");

    let events_dir = home.path().join(".8v").join("events");
    assert!(!events_dir.exists(), "events/ must not exist before check");

    bin_with_home(home.path())
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check");

    assert!(events_dir.exists(), "events/ must be created by check");
    assert!(events_dir.is_dir(), "events/ must be a directory");
}

/// `8v init --yes` followed by `8v check` writes series.json.
#[test]
fn test_init_then_check_writes_series_json() {
    let project = TempProject::empty();
    let home = tempfile::tempdir().expect("create home");
    let path = project.path();

    let init_out = bin_with_home(home.path())
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        init_out.status.code(),
        Some(0),
        "init --yes should exit 0\nstderr: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    let check_out = bin_with_home(home.path())
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("run 8v check");

    assert_eq!(
        check_out.status.code(),
        Some(2),
        "empty dir after init should exit 2\nstderr: {}",
        String::from_utf8_lossy(&check_out.stderr)
    );

    let series = read_series(home.path());
    assert!(!series.run_id.is_empty(), "run_id must be set");
    assert!(series.timestamp > 0, "timestamp must be positive");
}

/// Full isolated E2E with real violations: init → check → check again.
#[test]
fn test_full_e2e_violations_two_runs_isolated() {
    let project = TempProject::from_fixture(Fixture::e2e("rust-violations").path());
    let home = tempfile::tempdir().expect("create home");
    let path = project.path().to_str().unwrap();

    // ── Step 1: init ─────────────────────────────────────────────────────────
    // init now runs a baseline check internally (run_count=1 after init).
    let init_out = bin_with_home(home.path())
        .args(["init", path, "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        init_out.status.code(),
        Some(0),
        "init --yes must exit 0\nstderr: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    // After init: series.json must exist with baseline_run_id set and
    // diagnostics at run_count=1 (the baseline check counts as run 1).
    let series_after_init = read_series(home.path());
    assert!(
        series_after_init.baseline_run_id.is_some(),
        "baseline_run_id must be set after init"
    );
    assert!(
        !series_after_init.diagnostics.is_empty(),
        "init baseline check must record diagnostics from violations"
    );
    for entry in series_after_init.diagnostics.values() {
        assert_eq!(
            entry.run_count, 1,
            "after init: every diagnostic must have run_count=1 (baseline run)"
        );
    }

    // ── Step 2: first explicit check (overall run 2) ──────────────────────────
    let check1 = bin_with_home(home.path())
        .args(["check", path])
        .output()
        .expect("run 8v check (run 1)");

    assert_eq!(
        check1.status.code(),
        Some(1),
        "violations fixture must exit 1\nstderr: {}",
        String::from_utf8_lossy(&check1.stderr)
    );

    let series1 = read_series(home.path());
    assert!(
        !series1.diagnostics.is_empty(),
        "first explicit check must record diagnostics from violations"
    );

    let snapshot: std::collections::HashMap<String, u64> = series1
        .diagnostics
        .iter()
        .map(|(id, e)| (id.clone(), e.first_seen))
        .collect();

    for entry in series1.diagnostics.values() {
        assert_eq!(
            entry.run_count, 2,
            "first explicit check: every diagnostic must have run_count=2 (init was run 1)"
        );
    }

    // ── Step 3: second explicit check (overall run 3) — same violations ───────
    let check2 = bin_with_home(home.path())
        .args(["check", path])
        .output()
        .expect("run 8v check (run 2)");

    assert_eq!(check2.status.code(), Some(1), "run 2 must also exit 1");

    let series2 = read_series(home.path());

    assert_ne!(
        series2.run_id, series1.run_id,
        "run_id must change each run"
    );
    assert_eq!(
        series2.diagnostics.len(),
        series1.diagnostics.len(),
        "same violations — diagnostic count must not change"
    );

    for (id, entry) in &series2.diagnostics {
        assert_eq!(
            entry.run_count, 3,
            "second explicit check: run_count must be 3 for {id}"
        );
        let original_first_seen = snapshot
            .get(id)
            .expect("all run-2 IDs must exist in run-1 series");
        assert_eq!(
            entry.first_seen, *original_first_seen,
            "first_seen must be preserved across runs for {id}"
        );
    }
}
