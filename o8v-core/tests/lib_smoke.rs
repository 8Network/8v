//! Smoke tests — run real tools on corpus fixtures.
//!
//! These tests verify that tools run and parsing works. They do NOT assert
//! on specific diagnostics — that's what e2e_violations.rs is for.
//!
//! If a tool is not installed, the test verifies the error is reported cleanly.
//! Only Rust rejects Error outcomes (cargo is always present).

use o8v_core::diagnostic::ParseStatus;
use o8v_core::CheckOutcome;
use o8v_testkit::*;

// ─── Rust (cargo check + clippy + fmt) ───────────────────────────────────

#[test]
fn rust_standalone_runs_all_checks() {
    let fixture = Fixture::corpus("rust-standalone-app");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);

    // Assert expected checks exist — not just iterate whatever ran.
    assert!(has_check(&report, "cargo check"), "missing cargo check");
    assert!(has_check(&report, "clippy"), "missing clippy");
    assert!(has_check(&report, "cargo fmt"), "missing cargo fmt");
}

#[test]
fn rust_standalone_clippy_produces_parsed_diagnostics() {
    let fixture = Fixture::corpus("rust-standalone-app");
    let report = run_check(&fixture);

    let result = &report.results()[0];
    let entry = find_entry(result, "clippy");
    // Clippy on clean code should pass or fail — but always parse.
    assert_parse_status(entry, ParseStatus::Parsed);
}

#[test]
fn rust_workspace_checks_members() {
    let fixture = Fixture::corpus("rust-virtual-workspace");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 3);

    assert!(has_check(&report, "cargo check"), "missing cargo check");
    assert!(has_check(&report, "clippy"), "missing clippy");
}

// ─── Go (go vet + staticcheck) ──────────────────────────────────────────

#[test]
fn go_runs_checks() {
    let fixture = Fixture::corpus("go-service");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);

    // Assert expected checks exist.
    assert!(has_check(&report, "go vet"), "missing go vet");
    assert!(has_check(&report, "staticcheck"), "missing staticcheck");

    let result = &report.results()[0];

    // go vet: must run and parse (go is installed if go-service fixture exists).
    let govet = find_entry(result, "go vet");
    match govet.outcome() {
        CheckOutcome::Passed { parse_status, .. } | CheckOutcome::Failed { parse_status, .. } => {
            assert_eq!(*parse_status, ParseStatus::Parsed);
        }
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("could not run"),
                "go vet: expected tool-not-found, got: {cause}"
            );
        }
        #[allow(unreachable_patterns)]
        other => panic!("go vet: unexpected outcome: {other:?}"),
    }

    // staticcheck: may not be installed — accept Error(could not run) or Parsed.
    let sc = find_entry(result, "staticcheck");
    match sc.outcome() {
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("could not run"),
                "staticcheck: expected tool-not-found, got: {cause}"
            );
        }
        CheckOutcome::Passed { parse_status, .. } | CheckOutcome::Failed { parse_status, .. } => {
            assert_eq!(*parse_status, ParseStatus::Parsed);
        }
        #[allow(unreachable_patterns)]
        other => panic!("staticcheck: unexpected outcome: {other:?}"),
    }
}

// ─── Python (ruff) ──────────────────────────────────────────────────────

#[test]
fn python_runs_ruff() {
    let fixture = Fixture::corpus("python-uv-workspace");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 3);

    let result = find_result(&report, Stack::Python);
    let entry = find_entry(result, "ruff");

    match entry.outcome() {
        CheckOutcome::Passed { parse_status, .. } | CheckOutcome::Failed { parse_status, .. } => {
            assert_eq!(*parse_status, ParseStatus::Parsed);
        }
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("could not run"),
                "ruff: expected tool-not-found, got: {cause}"
            );
        }
        #[allow(unreachable_patterns)]
        other => panic!("ruff: unexpected outcome: {other:?}"),
    }
}

// ─── TypeScript (tsc + eslint) ──────────────────────────────────────────

#[test]
fn typescript_runs_checks() {
    let fixture = Fixture::corpus("typescript-workspace");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 2);

    assert!(has_check(&report, "tsc"), "missing tsc");
    assert!(has_check(&report, "eslint"), "missing eslint");
}

#[test]
fn typescript_tools_report_not_installed_cleanly() {
    let fixture = Fixture::corpus("typescript-workspace");
    let report = run_check(&fixture);

    let result = find_result(&report, Stack::TypeScript);
    for entry in result.entries() {
        match entry.outcome() {
            CheckOutcome::Error { cause, .. } => {
                assert!(
                    cause.contains("not installed"),
                    "{}: expected 'not installed' message, got: {cause}",
                    entry.name()
                );
            }
            CheckOutcome::Passed { .. } | CheckOutcome::Failed { .. } => {
                // Tool is installed — fine, nothing to check about install message.
            }
            #[allow(unreachable_patterns)]
            other => panic!("{}: unexpected outcome: {other:?}", entry.name()),
        }
    }
}

// ─── JavaScript (eslint) ────────────────────────────────────────────────

#[test]
fn javascript_runs_eslint() {
    let fixture = Fixture::corpus("javascript-workspace");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 3);
    assert!(has_check(&report, "eslint"), "missing eslint");
}

// ─── .NET (dotnet build) ────────────────────────────────────────────────

#[test]
fn dotnet_runs_build() {
    let fixture = Fixture::corpus("dotnet-standalone-fallback");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);

    let result = &report.results()[0];
    let entry = find_entry(result, "dotnet build");

    match entry.outcome() {
        CheckOutcome::Passed { .. } | CheckOutcome::Failed { .. } => {
            // dotnet installed — tool ran. Parse status can be Parsed or Unparsed.
        }
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("could not run"),
                "dotnet build: expected tool-not-found, got: {cause}"
            );
        }
        #[allow(unreachable_patterns)]
        other => panic!("dotnet build: unexpected outcome: {other:?}"),
    }
}

// ─── Deno ───────────────────────────────────────────────────────────────

#[test]
fn deno_runs_check() {
    let fixture = Fixture::corpus("deno-workspace");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);
    assert!(has_check(&report, "deno check"), "missing deno check");

    let result = &report.results()[0];
    let entry = find_entry(result, "deno check");

    match entry.outcome() {
        CheckOutcome::Passed { .. } | CheckOutcome::Failed { .. } => {
            // deno installed — tool ran.
        }
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("could not run"),
                "deno check: expected tool-not-found, got: {cause}"
            );
        }
        #[allow(unreachable_patterns)]
        other => panic!("deno check: unexpected outcome: {other:?}"),
    }
}

// ─── Polyglot (multiple stacks) ─────────────────────────────────────────

#[test]
fn polyglot_detects_multiple_stacks() {
    let fixture = Fixture::corpus("polyglot-studio");
    let report = run_check(&fixture);

    assert!(
        report.results().len() > 1,
        "polyglot should detect multiple stacks"
    );
}
