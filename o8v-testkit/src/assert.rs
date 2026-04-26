//! Outcome assertions — assert Passed, Failed, Error on check entries.

use o8v_core::{CheckEntry, CheckOutcome, ParseStatus};

/// Assert the entry passed. Panics with details if Failed or Error.
pub fn assert_passed(entry: &CheckEntry) {
    match entry.outcome() {
        CheckOutcome::Passed { .. } => {}
        CheckOutcome::Failed {
            code,
            diagnostics,
            raw_stdout,
            raw_stderr,
            ..
        } => {
            panic!(
                "'{}' expected Passed but Failed (code={code:?}, {} diagnostics)\nstdout: {}\nstderr: {}",
                entry.name(),
                diagnostics.len(),
                &raw_stdout[..raw_stdout.len().min(500)],
                &raw_stderr[..raw_stderr.len().min(500)]
            );
        }
        CheckOutcome::Error { cause, .. } => {
            panic!("'{}' expected Passed but Error: {cause}", entry.name());
        }
        #[allow(unreachable_patterns)]
        other => panic!("'{}' unexpected outcome: {other:?}", entry.name()),
    }
}

/// Assert the entry failed. Panics if Passed or Error.
pub fn assert_failed(entry: &CheckEntry) {
    match entry.outcome() {
        CheckOutcome::Failed { .. } => {}
        CheckOutcome::Passed { .. } => {
            panic!("'{}' expected Failed but Passed", entry.name());
        }
        CheckOutcome::Error { cause, .. } => {
            panic!("'{}' expected Failed but Error: {cause}", entry.name());
        }
        #[allow(unreachable_patterns)]
        other => panic!("'{}' unexpected outcome: {other:?}", entry.name()),
    }
}

/// Assert the entry errored and the cause contains the given substring.
pub fn assert_error(entry: &CheckEntry, cause_contains: &str) {
    match entry.outcome() {
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains(cause_contains),
                "'{}' Error cause '{}' does not contain '{cause_contains}'",
                entry.name(),
                cause
            );
        }
        CheckOutcome::Passed { .. } => {
            panic!("'{}' expected Error but Passed", entry.name());
        }
        CheckOutcome::Failed { .. } => {
            panic!("'{}' expected Error but Failed", entry.name());
        }
        #[allow(unreachable_patterns)]
        other => panic!("'{}' unexpected outcome: {other:?}", entry.name()),
    }
}

/// Assert the parse status on an entry's outcome.
pub fn assert_parse_status(entry: &CheckEntry, expected: ParseStatus) {
    let actual = match entry.outcome() {
        CheckOutcome::Passed { parse_status, .. } | CheckOutcome::Failed { parse_status, .. } => {
            *parse_status
        }
        CheckOutcome::Error { .. } => {
            panic!("'{}' is Error — has no parse_status", entry.name());
        }
        #[allow(unreachable_patterns)]
        other => panic!("'{}' unexpected outcome: {other:?}", entry.name()),
    };
    assert_eq!(
        actual,
        expected,
        "'{}' parse_status: expected {expected:?}, got {actual:?}",
        entry.name()
    );
}

/// Assert a parse result has valid diagnostics.
pub fn assert_parsed_diagnostics(result: &o8v_core::ParseResult, tool: &str, stack: &str) {
    assert_eq!(
        result.status,
        o8v_core::ParseStatus::Parsed,
        "expected Parsed status for {tool}"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "{tool} produced no diagnostics"
    );
    for d in &result.diagnostics {
        assert!(
            !d.message.is_empty(),
            "{tool} diagnostic has empty message: {d:?}"
        );
        assert_eq!(d.tool, tool, "diagnostic tool field mismatch");
        assert_eq!(d.stack, stack, "diagnostic stack field mismatch");
    }
}
