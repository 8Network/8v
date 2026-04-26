// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

// ─── coverage: run_check ────────────────────────────────────────

#[test]
fn run_check_returns_report_for_valid_fixture() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    // rust-violations has known violations — report must have results
    assert!(!report.results().is_empty(), "report should have results");
}

// ─── coverage: assert_no_detection_errors ───────────────────────

#[test]
fn assert_no_detection_errors_passes_on_clean_report() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert_no_detection_errors(&report);
}

// ─── coverage: assert_project_count ─────────────────────────────

#[test]
fn assert_project_count_passes_on_correct_count() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert_project_count(&report, 3);
}

#[test]
#[should_panic(expected = "expected 99 projects")]
fn assert_project_count_panics_on_wrong_count() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert_project_count(&report, 99);
}

// ─── coverage: assert_expected ──────────────────────────────────

#[test]
fn assert_expected_passes_on_valid_fixture() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let expected = Expected::load(&fixture);
    assert_expected(&report, &expected);
}

#[test]
#[should_panic(expected = "no [[check]] entries")]
fn assert_expected_panics_on_empty_checks() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let empty = Expected {
        stack: "rust".to_string(),
        checks: vec![],
    };
    assert_expected(&report, &empty);
}

#[test]
fn assert_expected_skips_checks_with_no_diagnostics() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let no_diags = Expected {
        stack: "rust".to_string(),
        checks: vec![ExpectedCheck {
            tool: "cargo check".to_string(),
            diagnostics: vec![],
        }],
    };
    // Should not panic — checks with no diagnostics are skipped.
    assert_expected(&report, &no_diags);
}
