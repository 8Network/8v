// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use crate::*;
use std::path::Path;

#[test]
fn fixture_e2e_resolves() {
    let fixture = Fixture::e2e("rust-violations");
    assert!(fixture.path().is_dir());
    assert!(fixture.path().join("Cargo.toml").is_file());
    assert!(fixture.path().join("EXPECTED.toml").is_file());
}

#[test]
fn fixture_corpus_resolves() {
    let fixture = Fixture::corpus("rust-standalone-app");
    assert!(fixture.path().is_dir());
    assert!(fixture.path().join("Cargo.toml").is_file());
}

#[test]
#[should_panic(expected = "e2e fixture not found")]
fn fixture_e2e_missing_panics() {
    let _ = Fixture::e2e("nonexistent-fixture-that-does-not-exist");
}

#[test]
#[should_panic(expected = "corpus fixture not found")]
fn fixture_corpus_missing_panics() {
    let _ = Fixture::corpus("nonexistent-fixture-that-does-not-exist");
}

#[test]
fn expected_loads_from_fixture() {
    let fixture = Fixture::e2e("rust-violations");
    let expected = Expected::load(&fixture);
    assert_eq!(expected.stack, "rust", "stack should be rust");
    assert!(
        !expected.checks.is_empty(),
        "should have at least one [[check]]"
    );
    assert_eq!(
        expected.checks[0].tool, "clippy",
        "first check should be clippy"
    );
    assert!(
        !expected.checks[0].diagnostics.is_empty(),
        "clippy check should have diagnostics"
    );
}

#[test]
fn expected_diagnostic_severity_parsing() {
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "error".to_string(),
        message_contains: None,
    };
    assert_eq!(exp.severity(), Severity::Error);

    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "warning".to_string(),
        message_contains: None,
    };
    assert_eq!(exp.severity(), Severity::Warning);
}

#[test]
#[should_panic(expected = "unknown severity")]
fn expected_diagnostic_unknown_severity_panics() {
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "critical".to_string(),
        message_contains: None,
    };
    let _ = exp.severity();
}

// ─── find_result tests ────────────────────────────────────────────

#[test]
fn find_result_returns_matching_stack() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    assert_eq!(result.stack(), Stack::Rust);
}

#[test]
#[should_panic(expected = "not found in report")]
fn find_result_panics_on_missing_stack() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let _ = find_result(&report, Stack::Python);
}

// ─── find_entry tests ─────────────────────────────────────────────

#[test]
fn find_entry_returns_matching_check() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_eq!(entry.name(), "clippy");
}

#[test]
#[should_panic(expected = "not found")]
fn find_entry_panics_on_missing_check() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let _ = find_entry(result, "nonexistent-check");
}

// ─── all_check_names tests ────────────────────────────────────────

#[test]
fn all_check_names_includes_all_results() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let names = all_check_names(&report);
    // Rust has at least cargo check, clippy, cargo fmt
    assert!(names.contains(&"clippy"), "missing clippy: {names:?}");
    assert!(
        names.contains(&"cargo check"),
        "missing cargo check: {names:?}"
    );
}

// ─── assert_passed tests ──────────────────────────────────────────

#[test]
fn assert_passed_succeeds_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    // At least one check should pass on clean code
    let entry = find_entry(result, "cargo check");
    assert_passed(entry);
}

#[test]
#[should_panic(expected = "expected Passed but Failed")]
fn assert_passed_panics_on_failed() {
    // Create a project with violations to get a Failed outcome
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_passed(entry); // clippy should fail on violations
}

// ─── assert_failed tests ──────────────────────────────────────────

#[test]
fn assert_failed_succeeds_on_failed() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_failed(entry);
}

#[test]
#[should_panic(expected = "expected Failed but Passed")]
fn assert_failed_panics_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "cargo check");
    assert_failed(entry);
}

// ─── assert_error tests ───────────────────────────────────────────

#[test]
fn assert_error_succeeds_on_matching_cause() {
    // TypeScript project without node_modules — tools will Error
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_error(entry, "not installed");
}

#[test]
#[should_panic(expected = "does not contain")]
fn assert_error_panics_on_wrong_cause() {
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_error(entry, "timed out"); // wrong cause
}

#[test]
#[should_panic(expected = "expected Error but Passed")]
fn assert_error_panics_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "cargo check");
    assert_error(entry, "anything");
}

// ─── assert_parse_status tests ────────────────────────────────────

#[test]
fn assert_parse_status_succeeds_on_match() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_parse_status(entry, ParseStatus::Parsed);
}

#[test]
#[should_panic(expected = "parse_status")]
fn assert_parse_status_panics_on_mismatch() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_parse_status(entry, ParseStatus::Unparsed); // wrong
}

#[test]
#[should_panic(expected = "is Error")]
fn assert_parse_status_panics_on_error() {
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_parse_status(entry, ParseStatus::Parsed);
}

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
    assert_project_count(&report, 1);
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

// ─── coverage: assert_parsed_diagnostics ────────────────────────

#[test]
fn assert_parsed_diagnostics_passes_on_valid_parse() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: Some(o8v_core::DisplayStr::from_untrusted("E0001")),
            severity: Severity::Error,
            raw_severity: Some("error".to_string()),
            message: o8v_core::DisplayStr::from_untrusted("test error"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "rustc".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Parsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "produced no diagnostics")]
fn assert_parsed_diagnostics_panics_on_empty() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![],
        status: ParseStatus::Parsed,
        parsed_items: 0,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "expected Parsed status")]
fn assert_parsed_diagnostics_panics_on_wrong_status() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: o8v_core::DisplayStr::from_untrusted("err"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "rustc".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Unparsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "tool field mismatch")]
fn assert_parsed_diagnostics_panics_on_tool_mismatch() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: o8v_core::DisplayStr::from_untrusted("err"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "wrong".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Parsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}
