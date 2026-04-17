//! Integration tests for the `check()` pipeline.
//! Tests detection → planning → execution end-to-end.

use o8v_testkit::*;

// ─── Rust checks ─────────────────────────────────────────────────────────

#[test]
fn rust_standalone_detected_and_checked() {
    let project = TempProject::rust_passing();

    let report = run_check_path(project.path());

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);
    assert_eq!(report.results()[0].project_name(), "test-app");

    let names = all_check_names(&report);
    assert!(
        names.contains(&"cargo check"),
        "missing cargo check: {names:?}"
    );
    assert!(names.contains(&"clippy"), "missing clippy: {names:?}");
    assert!(names.contains(&"cargo fmt"), "missing cargo fmt: {names:?}");
}

// ─── No project detected ─────────────────────────────────────────────────

#[test]
fn empty_directory_no_results() {
    let dir = tempfile::tempdir().unwrap();
    let report = run_check_path(dir.path());

    assert_no_detection_errors(&report);
    assert!(report.results().is_empty());
}

// ─── Corrupt manifest → detection error ──────────────────────────────────

#[test]
fn corrupt_manifest_surfaces_detection_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();

    let report = run_check_path(dir.path());

    assert!(!report.detection_errors().is_empty());
    assert!(!report.is_ok());
}

// ─── Stack planning ──────────────────────────────────────────────────────

// ─── CheckEvent contract ─────────────────────────────────────────────────

#[test]
fn events_fire_in_order_for_rust_project() {
    use o8v_core::project::ProjectRoot;
    use o8v_core::{CheckConfig, CheckEvent};
    use std::sync::atomic::AtomicBool;

    let project = TempProject::rust_passing();
    let root = ProjectRoot::new(project.path()).unwrap();
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };

    let mut events: Vec<String> = Vec::new();
    let report = o8v_check::check(&root, &config, |event| match event {
        CheckEvent::DetectionError { .. } => events.push("detection_error".into()),
        CheckEvent::ProjectStart { name, .. } => events.push(format!("project_start:{name}")),
        CheckEvent::CheckStart { name } => events.push(format!("check_start:{name}")),
        CheckEvent::CheckDone { entry } => events.push(format!("check_done:{}", entry.name())),
    });

    // Must have ProjectStart before any CheckStart/CheckDone.
    assert!(
        events[0].starts_with("project_start:"),
        "first event should be ProjectStart: {events:?}"
    );

    // Every CheckStart must be followed by a CheckDone for the same name.
    let starts: Vec<&str> = events
        .iter()
        .filter_map(|e| e.strip_prefix("check_start:"))
        .collect();
    let dones: Vec<&str> = events
        .iter()
        .filter_map(|e| e.strip_prefix("check_done:"))
        .collect();
    assert_eq!(starts, dones, "CheckStart and CheckDone must pair up");

    // Number of CheckDone events must match entries in the report.
    let report_entries: usize = report.results().iter().map(|r| r.entries().len()).sum();
    assert_eq!(
        dones.len(),
        report_entries,
        "CheckDone count must match report entries"
    );
}

#[test]
fn noop_callback_produces_same_report() {
    use o8v_core::project::ProjectRoot;
    use o8v_core::{CheckConfig, CheckEvent};
    use std::sync::atomic::AtomicBool;

    let project = TempProject::rust_passing();
    let root = ProjectRoot::new(project.path()).unwrap();
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };

    // Collecting callback
    let mut event_count = 0usize;
    let report_with = o8v_check::check(&root, &config, |_: CheckEvent<'_>| {
        event_count += 1;
    });

    // No-op callback
    let report_without = o8v_check::check(&root, &config, |_: CheckEvent<'_>| {});

    // Same structure: same project count, same check count.
    assert_eq!(
        report_with.results().len(),
        report_without.results().len(),
        "callback should not affect report structure"
    );
    for (a, b) in report_with.results().iter().zip(report_without.results()) {
        assert_eq!(a.entries().len(), b.entries().len());
    }
    assert!(event_count > 0, "collecting callback should receive events");
}

#[test]
fn detection_errors_fire_before_projects() {
    use o8v_core::project::ProjectRoot;
    use o8v_core::{CheckConfig, CheckEvent};
    use std::sync::atomic::AtomicBool;

    let dir = tempfile::tempdir().unwrap();
    // Corrupt manifest → detection error.
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };

    let mut events: Vec<String> = Vec::new();
    let _ = o8v_check::check(&root, &config, |event| match event {
        CheckEvent::DetectionError { .. } => events.push("detection_error".into()),
        CheckEvent::ProjectStart { .. } => events.push("project_start".into()),
        CheckEvent::CheckStart { .. } | CheckEvent::CheckDone { .. } => {
            // Not relevant for this test — only checking event ordering.
        }
    });

    assert!(!events.is_empty(), "corrupt manifest should produce events");
    assert_eq!(
        events[0], "detection_error",
        "detection errors must fire before projects: {events:?}"
    );
}

// ─── Interrupt scenarios ─────────────────────────────────────────────

#[test]
fn interrupt_preserves_checkstart_checkdone_pairing() {
    use o8v_core::project::ProjectRoot;
    use o8v_core::{CheckConfig, CheckEvent};
    use std::sync::atomic::{AtomicBool, Ordering};

    let project = TempProject::rust_passing();
    let root = ProjectRoot::new(project.path()).unwrap();
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    let config = CheckConfig {
        timeout: None,
        interrupted,
    };

    let mut events: Vec<String> = Vec::new();
    let mut check_count = 0;

    let _report = o8v_check::check(&root, &config, |event| match event {
        CheckEvent::DetectionError { .. } => {}
        CheckEvent::ProjectStart { .. } => {}
        CheckEvent::CheckStart { name } => {
            check_count += 1;
            events.push(format!("check_start:{name}"));
            // Set interrupt after first check starts to simulate Ctrl-C during check
            if check_count == 1 {
                interrupted.store(true, Ordering::Release);
            }
        }
        CheckEvent::CheckDone { entry } => {
            events.push(format!("check_done:{}", entry.name()));
        }
    });

    // Verify we got at least one check started
    assert!(
        check_count > 0,
        "should have started at least one check before interrupt"
    );

    // Verify: every CheckStart has a matching CheckDone
    let starts: Vec<&str> = events
        .iter()
        .filter_map(|e| e.strip_prefix("check_start:"))
        .collect();
    let dones: Vec<&str> = events
        .iter()
        .filter_map(|e| e.strip_prefix("check_done:"))
        .collect();
    assert_eq!(
        starts, dones,
        "interrupt should not create orphaned CheckStart events: starts={starts:?}, dones={dones:?}"
    );

    // We should have emitted at least one CheckDone before the interrupt took effect
    assert!(
        !dones.is_empty(),
        "should have completed at least one check"
    );
}

// ─── Stack planning ──────────────────────────────────────────────────────

#[test]
fn every_stack_has_checks() {
    use o8v_core::project::Stack;

    let stacks = [
        Stack::Rust,
        Stack::TypeScript,
        Stack::JavaScript,
        Stack::Python,
        Stack::Go,
        Stack::Deno,
        Stack::DotNet,
    ];

    for stack in stacks {
        let checks = o8v_stacks::checks_for(stack);
        assert!(!checks.is_empty(), "{stack} should have at least one check");
    }
}
