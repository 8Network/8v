//! Tests for `enrich()` — the shared enrichment function that all stacks use.
//!
//! These tests exercise every branch of the enrichment logic without needing
//! real tools installed. Uses synthetic parsers that return controlled output.

// Synthetic parsers must match ParseFn signature — cannot be const.
#![allow(clippy::missing_const_for_fn)]

use o8v_core::diagnostic::{Diagnostic, Location, ParseResult, ParseStatus, Severity, Span};
use o8v_core::CheckOutcome;
use o8v_core::DisplayStr;
use o8v_fs::ContainmentRoot;
use o8v_project::ProjectRoot;
use o8v_stacks::{enrich, ParseFn};

/// A parser that always returns empty diagnostics (tool passed clean).
fn parser_clean(
    _stdout: &str,
    _stderr: &str,
    _root: &std::path::Path,
    _tool: &str,
    _stack: &str,
) -> ParseResult {
    ParseResult {
        diagnostics: vec![],
        status: ParseStatus::Parsed,
        parsed_items: 0,
    }
}

/// A parser that returns one diagnostic (tool found an issue).
fn parser_one_error(
    _stdout: &str,
    _stderr: &str,
    _root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let d = Diagnostic {
        location: Location::File("src/main.rs".to_string()),
        span: Some(Span {
            line: 10,
            column: 5,
            end_line: None,
            end_column: None,
        }),
        rule: Some(DisplayStr::from_untrusted("test-rule")),
        severity: Severity::Error,
        raw_severity: Some("error".to_string()),
        message: DisplayStr::from_untrusted("test error"),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    };
    ParseResult {
        diagnostics: vec![d],
        status: ParseStatus::Parsed,
        parsed_items: 1,
    }
}

/// A parser that returns Unparsed (couldn't parse the output).
fn parser_unparsed(
    _stdout: &str,
    _stderr: &str,
    _root: &std::path::Path,
    _tool: &str,
    _stack: &str,
) -> ParseResult {
    ParseResult {
        diagnostics: vec![],
        status: ParseStatus::Unparsed,
        parsed_items: 0,
    }
}

fn temp_project() -> (tempfile::TempDir, ContainmentRoot) {
    let dir = tempfile::tempdir().unwrap();
    let path = ProjectRoot::new(dir.path()).unwrap();
    let containment = path.as_containment_root().unwrap();
    (dir, containment)
}

fn passed(stdout: &str) -> CheckOutcome {
    CheckOutcome::passed(
        stdout.to_string(),
        String::new(),
        ParseStatus::Unparsed,
        false,
        false,
    )
}

fn failed(stdout: &str) -> CheckOutcome {
    CheckOutcome::failed(
        None,
        vec![],
        stdout.to_string(),
        String::new(),
        ParseStatus::Unparsed,
        false,
        false,
    )
}

fn error() -> CheckOutcome {
    CheckOutcome::error(o8v_core::ErrorKind::Runtime, "tool not found".to_string())
}

// ─── Passed + clean parser = stays Passed ────────────────────────────────

#[test]
fn passed_stays_passed_when_parser_finds_nothing() {
    let (_dir, path) = temp_project();
    let result = enrich(
        passed("some output"),
        &path,
        "test",
        "test",
        parser_clean as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Passed { .. }),
        "clean parser on passing outcome should stay Passed"
    );
    if let CheckOutcome::Passed {
        diagnostics,
        parse_status,
        ..
    } = result
    {
        assert!(
            diagnostics.is_empty(),
            "clean parser should produce no diagnostics"
        );
        assert_eq!(
            parse_status,
            ParseStatus::Parsed,
            "parse_status should be Parsed after clean parse"
        );
    }
}

// ─── Passed + unparsed non-empty stdout = Error (cannot trust exit 0) ────

#[test]
fn passed_becomes_error_when_parser_fails_on_nonempty_stdout() {
    let (_dir, path) = temp_project();
    let result = enrich(
        passed("unparseable output"),
        &path,
        "go vet",
        "go",
        parser_unparsed as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Error { .. }),
        "must be Error when parser fails on non-empty stdout: {result:?}"
    );
    if let CheckOutcome::Error { kind, cause, .. } = &result {
        assert_eq!(
            *kind,
            o8v_core::ErrorKind::Verification,
            "error kind should be Verification when output cannot be parsed"
        );
        assert!(
            cause.contains("could not be parsed"),
            "cause should explain: {cause}"
        );
    }
}

// ─── Passed + unparsed truncated stdout = Passed (truncation is expected) ─

#[test]
fn passed_stays_passed_when_stdout_was_truncated() {
    let (_dir, path) = temp_project();
    // Simulate truncated output — the stdout_truncated bool is the authoritative
    // signal, NOT a marker string in the content (o8v-process does not embed one).
    let outcome = CheckOutcome::passed(
        "some json garbage that was cut mid-stream".to_string(),
        String::new(),
        ParseStatus::Unparsed,
        true,
        false,
    );
    let result = enrich(
        outcome,
        &path,
        "cargo check",
        "rust",
        parser_unparsed as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Passed { .. }),
        "truncated output + Unparsed should stay Passed, not Error: {result:?}"
    );
}

// ─── Passed + unparsed empty stdout = still Passed (nothing to parse) ───

#[test]
fn passed_stays_passed_when_stdout_is_empty_and_unparsed() {
    let (_dir, path) = temp_project();
    let result = enrich(
        passed(""),
        &path,
        "test",
        "test",
        parser_unparsed as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Passed { .. }),
        "empty stdout + Unparsed should stay Passed"
    );
}

// ─── Passed + diagnostics found = promoted to Failed ─────────────────────

#[test]
fn passed_promoted_to_failed_when_parser_finds_diagnostics() {
    let (_dir, path) = temp_project();
    let result = enrich(
        passed("some output"),
        &path,
        "mytool",
        "mystack",
        parser_one_error as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Failed { .. }),
        "should promote to Failed"
    );
    if let CheckOutcome::Failed {
        diagnostics,
        parse_status,
        ..
    } = result
    {
        assert_eq!(
            diagnostics.len(),
            1,
            "should have exactly one diagnostic from parser"
        );
        assert_eq!(
            diagnostics[0].tool, "mytool",
            "diagnostic tool should match"
        );
        assert_eq!(
            diagnostics[0].stack, "mystack",
            "diagnostic stack should match"
        );
        assert_eq!(
            diagnostics[0].message, "test error",
            "diagnostic message should match"
        );
        assert_eq!(
            parse_status,
            ParseStatus::Parsed,
            "parse_status should be Parsed after successful parse"
        );
    }
}

// ─── Failed + parser enriches with diagnostics ──────────────────────────

#[test]
fn failed_enriched_with_diagnostics() {
    let (_dir, path) = temp_project();
    let result = enrich(
        failed("error output"),
        &path,
        "mytool",
        "mystack",
        parser_one_error as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Failed { .. }),
        "failed outcome should stay Failed after enrichment"
    );
    if let CheckOutcome::Failed {
        diagnostics,
        parse_status,
        raw_stdout,
        ..
    } = result
    {
        assert_eq!(diagnostics.len(), 1, "should have exactly one diagnostic");
        assert_eq!(
            parse_status,
            ParseStatus::Parsed,
            "parse_status should be Parsed after successful parse"
        );
        assert_eq!(raw_stdout, "error output", "raw_stdout should be preserved");
    }
}

// ─── Failed + 0 diagnostics + non-empty stdout → Unparsed (show raw) ────

#[test]
fn failed_with_zero_diagnostics_forces_unparsed_for_raw_fallback() {
    let (_dir, path) = temp_project();
    let result = enrich(
        failed("some output"),
        &path,
        "test",
        "test",
        parser_clean as ParseFn,
    );
    assert!(
        matches!(result, CheckOutcome::Failed { .. }),
        "Failed must stay Failed"
    );
    // Tool failed (non-zero exit) but parser found nothing — renderers need
    // to show the raw output. parse_status must be Unparsed so the fallback
    // path triggers. Without this, tool format drift becomes invisible.
    if let CheckOutcome::Failed { parse_status, .. } = result {
        assert_eq!(
            parse_status,
            ParseStatus::Unparsed,
            "0 diagnostics on Failed should force Unparsed for raw fallback"
        );
    }
}

// ─── Failed + 0 diagnostics + empty stdout → stays Parsed ──────────────

#[test]
fn failed_with_empty_stdout_stays_parsed() {
    let (_dir, path) = temp_project();
    let result = enrich(failed(""), &path, "test", "test", parser_clean as ParseFn);
    if let CheckOutcome::Failed { parse_status, .. } = result {
        assert_eq!(
            parse_status,
            ParseStatus::Parsed,
            "empty stdout should not force Unparsed"
        );
    } else {
        panic!("expected Failed");
    }
}

// ─── Failed + unparsed = Failed with Unparsed status ────────────────────

#[test]
fn failed_with_unparsed_parser() {
    let (_dir, path) = temp_project();
    let result = enrich(
        failed("garbage"),
        &path,
        "test",
        "test",
        parser_unparsed as ParseFn,
    );
    if let CheckOutcome::Failed {
        parse_status,
        diagnostics,
        ..
    } = result
    {
        assert_eq!(
            parse_status,
            ParseStatus::Unparsed,
            "unparsed parser should set Unparsed status"
        );
        assert!(
            diagnostics.is_empty(),
            "unparsed parser should produce no diagnostics"
        );
    } else {
        panic!("expected Failed");
    }
}

// ─── Truncation flag combinations ───────────────────────────────────────

#[test]
fn passed_stays_passed_when_stderr_truncated() {
    let (_dir, path) = temp_project();
    let outcome = CheckOutcome::passed(
        String::new(),
        "some stderr that was cut".to_string(),
        ParseStatus::Unparsed,
        false,
        true, // stderr truncated
    );
    let result = enrich(outcome, &path, "test", "test", parser_unparsed as ParseFn);
    assert!(
        matches!(result, CheckOutcome::Passed { .. }),
        "stderr-only truncation should stay Passed: {result:?}"
    );
}

#[test]
fn passed_stays_passed_when_both_truncated() {
    let (_dir, path) = temp_project();
    let outcome = CheckOutcome::passed(
        "stdout cut".to_string(),
        "stderr cut".to_string(),
        ParseStatus::Unparsed,
        true,
        true, // both truncated
    );
    let result = enrich(outcome, &path, "test", "test", parser_unparsed as ParseFn);
    assert!(
        matches!(result, CheckOutcome::Passed { .. }),
        "both truncated should stay Passed: {result:?}"
    );
}

#[test]
fn passed_with_diagnostics_and_truncation_promotes_to_failed() {
    let (_dir, path) = temp_project();
    let outcome = CheckOutcome::passed(
        "output".to_string(),
        String::new(),
        ParseStatus::Parsed,
        true, // stdout truncated
        false,
    );
    let result = enrich(outcome, &path, "test", "test", parser_one_error as ParseFn);
    // Diagnostics found → must promote to Failed regardless of truncation
    assert!(
        matches!(result, CheckOutcome::Failed { .. }),
        "diagnostics + truncation should still promote to Failed: {result:?}"
    );
    if let CheckOutcome::Failed {
        stdout_truncated, ..
    } = result
    {
        assert!(stdout_truncated, "truncation flag must propagate to Failed");
    }
}

#[test]
fn failed_with_stderr_truncation_preserves_flag() {
    let (_dir, path) = temp_project();
    let outcome = CheckOutcome::failed(
        Some(1),
        vec![],
        String::new(),
        "stderr content".to_string(),
        ParseStatus::Unparsed,
        false,
        true, // stderr truncated
    );
    let result = enrich(outcome, &path, "test", "test", parser_unparsed as ParseFn);
    assert!(
        matches!(result, CheckOutcome::Failed { .. }),
        "Failed should stay Failed: {result:?}"
    );
    if let CheckOutcome::Failed {
        stderr_truncated, ..
    } = result
    {
        assert!(
            stderr_truncated,
            "stderr truncation flag must propagate through enrichment"
        );
    }
}

// ─── Error passes through untouched ─────────────────────────────────────

#[test]
fn error_passes_through() {
    let (_dir, path) = temp_project();
    let result = enrich(error(), &path, "test", "test", parser_one_error as ParseFn);
    assert!(
        matches!(result, CheckOutcome::Error { .. }),
        "Error must pass through"
    );
    if let CheckOutcome::Error { cause, .. } = result {
        assert_eq!(cause, "tool not found");
    }
}

// ─── raw_stdout preserved through enrichment ────────────────────────────

#[test]
fn raw_stdout_preserved() {
    let (_dir, path) = temp_project();
    let result = enrich(
        passed("original stdout"),
        &path,
        "test",
        "test",
        parser_clean as ParseFn,
    );
    if let CheckOutcome::Passed { raw_stdout, .. } = result {
        assert_eq!(raw_stdout, "original stdout");
    } else {
        panic!("expected Passed");
    }
}
