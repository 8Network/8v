// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;
use std::path::Path;

// ─── collect_diagnostics panics on tool not found ─────────────────

#[test]
#[should_panic(expected = "not found in stack")]
fn collect_diagnostics_panics_on_missing_tool() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let _ = collect_diagnostics(&report, "nonexistent-tool", "rust");
}

// ─── run_check_interrupted tests ──────────────────────────────────

#[test]
fn interrupted_produces_empty_report() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check_interrupted(&fixture);
    assert!(
        report.results().is_empty(),
        "interrupted should produce empty results"
    );
}

// ─── adversarial: collect_diagnostics with nonexistent stack ─────

#[test]
#[should_panic(expected = "stack 'python' not found in report")]
fn collect_diagnostics_panics_on_nonexistent_stack() {
    // When the stack doesn't exist, the error now correctly says
    // "stack not found" with available stacks listed.
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let _ = collect_diagnostics(&report, "clippy", "python");
}

// ─── adversarial: has_check coverage ─────────────────────────────

#[test]
fn has_check_finds_existing_check() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert!(has_check(&report, "clippy"));
}

#[test]
fn has_check_returns_false_for_missing() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert!(!has_check(&report, "nonexistent-check"));
}

// ─── adversarial: assert_sanitized checks snippet field ──────────

#[test]
fn assert_sanitized_covers_all_fields() {
    // FIXED: assert_sanitized now checks snippet, related span labels,
    // and related span locations in addition to the original fields
    // (message, rule, location, notes, suggestion.message).
    //
    // Smoke test: verify it runs without panic on a real report.
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert_sanitized(&report);
}

// ─── adversarial: run_check_path on nonexistent directory ────────

#[test]
#[should_panic(expected = "path should be a valid ProjectRoot")]
fn run_check_path_panics_on_nonexistent_dir() {
    let _ = run_check_path(Path::new("/tmp/nonexistent-dir-8v-testkit-test"));
}

// ─── adversarial: Fixture::e2e with path traversal ───────────────

#[test]
#[should_panic(expected = "must not contain path separators")]
fn fixture_e2e_rejects_path_traversal() {
    let _ = Fixture::e2e("../../../Cargo.toml");
}

#[test]
#[should_panic(expected = "must not contain path separators")]
fn fixture_e2e_rejects_dotdot() {
    let _ = Fixture::e2e("../../..");
}

#[test]
#[should_panic(expected = "must not contain path separators")]
fn fixture_corpus_rejects_path_traversal() {
    let _ = Fixture::corpus("../../..");
}

// ─── adversarial: Fixture::e2e with file, not directory ──────────

#[test]
#[should_panic(expected = "must not contain path separators")]
fn fixture_e2e_file_not_dir_panics() {
    // Names with path separators are now rejected before the is_dir check.
    let _ = Fixture::e2e("rust-violations/EXPECTED.toml");
}

// ─── adversarial: all_check_names on empty report ────────────────

#[test]
fn all_check_names_returns_empty_on_no_results() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check_interrupted(&fixture);
    let names = all_check_names(&report);
    assert!(
        names.is_empty(),
        "interrupted report should have no check names"
    );
}

// ─── adversarial: find_result on empty report ────────────────────

#[test]
#[should_panic(expected = "not found in report")]
fn find_result_panics_on_empty_report() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check_interrupted(&fixture);
    let _ = find_result(&report, Stack::Rust);
}

// ─── adversarial: severity parsing exhaustive ────────────────────

#[test]
fn expected_diagnostic_all_severities() {
    for (input, expected) in [
        ("error", Severity::Error),
        ("warning", Severity::Warning),
        ("info", Severity::Info),
        ("hint", Severity::Hint),
    ] {
        let exp = ExpectedDiagnostic {
            rule: None,
            file: String::new(),
            severity: input.to_string(),
            message_contains: None,
        };
        assert_eq!(exp.severity(), expected, "severity mismatch for '{input}'");
    }
}

#[test]
#[should_panic(expected = "unknown severity")]
fn expected_diagnostic_uppercase_severity_panics() {
    // Severity parsing is case-sensitive — "Error" != "error"
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "Error".to_string(),
        message_contains: None,
    };
    let _ = exp.severity();
}

#[test]
#[should_panic(expected = "unknown severity")]
fn expected_diagnostic_empty_severity_panics() {
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: String::new(),
        message_contains: None,
    };
    let _ = exp.severity();
}
