//! Parser enrichment — runs parsers on tool output with panic recovery.
//!
//! `enrich()` is the single place where raw `CheckOutcome` gains structured
//! diagnostics. All stacks use it. Promotes `Passed` → `Failed` when
//! diagnostics are found.

use o8v_core::diagnostic::{Diagnostic, Location, Severity};
use o8v_core::display_str::DisplayStr;
use o8v_core::CheckOutcome;

/// Maximum number of diagnostics to retain per tool run.
/// Malicious tools could produce millions of diagnostics — truncate to prevent OOM.
const MAX_DIAGNOSTICS: usize = 10_000;

/// Parser function type for enriching tool output.
///
/// Takes `(stdout, stderr, project_root, tool, stack)`. Most parsers
/// only use stdout — tools like `deno check` write diagnostics to stderr.
pub type ParseFn =
    fn(&str, &str, &std::path::Path, &str, &str) -> o8v_core::diagnostic::ParseResult;

/// Enrich a `CheckOutcome` with parsed diagnostics.
///
/// Runs `parse_fn` on `raw_stdout` and replaces empty diagnostics with
/// parsed ones. Promotes `Passed` → `Failed` when diagnostics are found.
/// All stacks use this — the enrichment logic lives in one place.
pub fn enrich(
    outcome: CheckOutcome,
    project_dir: &o8v_fs::ContainmentRoot,
    tool: &str,
    stack: &str,
    parse_fn: ParseFn,
) -> CheckOutcome {
    match outcome {
        CheckOutcome::Passed {
            raw_stdout,
            raw_stderr,
            stdout_truncated,
            stderr_truncated,
            ..
        } => {
            let parsed = safe_parse(parse_fn, &raw_stdout, &raw_stderr, project_dir, tool, stack);
            let diagnostics = parsed.diagnostics;
            let parse_status = parsed.status;
            if !diagnostics.is_empty() {
                // Diagnostics found on exit 0 — promote to Failed.
                CheckOutcome::failed(
                    Some(0),
                    diagnostics,
                    raw_stdout,
                    raw_stderr,
                    parse_status,
                    stdout_truncated,
                    stderr_truncated,
                )
            } else if parse_status == o8v_core::diagnostic::ParseStatus::Unparsed
                && (!raw_stdout.trim().is_empty() || !raw_stderr.trim().is_empty())
                && !stdout_truncated
                && !stderr_truncated
            {
                // Parser couldn't parse non-empty output. We cannot trust
                // exit 0 — tools like go vet exit 0 even with findings.
                CheckOutcome::error_with_output(
                    o8v_core::ErrorKind::Verification,
                    format!(
                        "'{tool}' exited 0 but output could not be parsed — cannot verify pass"
                    ),
                    raw_stdout,
                    raw_stderr,
                )
            } else {
                CheckOutcome::passed(
                    raw_stdout,
                    raw_stderr,
                    parse_status,
                    stdout_truncated,
                    stderr_truncated,
                )
            }
        }
        CheckOutcome::Failed {
            code,
            raw_stdout,
            raw_stderr,
            stdout_truncated,
            stderr_truncated,
            ..
        } => {
            let parsed = safe_parse(parse_fn, &raw_stdout, &raw_stderr, project_dir, tool, stack);
            let diagnostics = parsed.diagnostics;
            let parse_status = parsed.status;
            let parsed_items = parsed.parsed_items;
            // If the tool failed (non-zero exit) but the parser found 0
            // diagnostics, force Unparsed so renderers show the raw fallback.
            // Without this, a tool output format change silently produces
            // "Failed with 0 diagnostics" and no visible evidence.
            // Check both streams — tools like deno write to stderr, not stdout.
            // Exception: if parsed_items > 0, the parser understood the format
            // and found no violations — trust it (e.g. eslint clean on exit 1).
            let has_output = !raw_stdout.trim().is_empty() || !raw_stderr.trim().is_empty();
            let parse_status = if diagnostics.is_empty()
                && parse_status == o8v_core::diagnostic::ParseStatus::Parsed
                && has_output
                && parsed_items == 0
            {
                o8v_core::diagnostic::ParseStatus::Unparsed
            } else {
                parse_status
            };
            CheckOutcome::failed(
                code,
                diagnostics,
                raw_stdout,
                raw_stderr,
                parse_status,
                stdout_truncated,
                stderr_truncated,
            )
        }
        error => error,
    }
}

/// Run a parser with panic recovery.
fn safe_parse(
    parse_fn: ParseFn,
    stdout: &str,
    stderr: &str,
    project_dir: &o8v_fs::ContainmentRoot,
    tool: &str,
    stack: &str,
) -> o8v_core::diagnostic::ParseResult {
    #[allow(clippy::disallowed_methods)] // panic recovery, not silent fallback
    let mut result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parse_fn(stdout, stderr, project_dir.as_path(), tool, stack)
    }))
    .unwrap_or_else(|e| {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        tracing::error!(tool, panic = %msg, "parser panicked — this is a bug in 8v");
        o8v_core::diagnostic::ParseResult {
            diagnostics: vec![],
            status: o8v_core::diagnostic::ParseStatus::Unparsed,
            parsed_items: 0,
        }
    });

    // Sanitize at the data boundary: strip ANSI from all external string fields.
    for d in &mut result.diagnostics {
        d.sanitize();
    }

    // Truncate diagnostics if they exceed the limit to prevent OOM.
    if result.diagnostics.len() > MAX_DIAGNOSTICS {
        let truncated_count = result.diagnostics.len() - MAX_DIAGNOSTICS;
        result.diagnostics.truncate(MAX_DIAGNOSTICS);
        tracing::warn!(
            tool,
            truncated = truncated_count,
            max = MAX_DIAGNOSTICS,
            "diagnostic count exceeded limit — truncated"
        );
        // Add synthetic diagnostic to inform user of truncation
        result.diagnostics.push(Diagnostic {
            location: Location::Absolute(String::new()),
            span: None,
            rule: Some(DisplayStr::from_trusted("max-diagnostics-truncated")),
            severity: Severity::Warning,
            raw_severity: None,
            message: DisplayStr::from_trusted(format!(
                "{} diagnostics truncated (limit: {})",
                truncated_count, MAX_DIAGNOSTICS
            )),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::{Diagnostic, Location, ParseStatus, Severity};
    use o8v_core::{CheckOutcome, ErrorKind};
    use o8v_project::ProjectRoot;

    fn dummy_diagnostic() -> Diagnostic {
        Diagnostic {
            location: Location::Absolute(String::new()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_trusted("test"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "test".to_string(),
            stack: "test".to_string(),
        }
    }

    fn project_path() -> o8v_fs::ContainmentRoot {
        let dir = tempfile::tempdir().unwrap();
        // Leak the tempdir so it stays alive for the duration of the test.
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        let root = ProjectRoot::new(path).unwrap();
        root.as_containment_root().unwrap()
    }

    /// Parser that returns one diagnostic, Parsed.
    fn parse_one_diag(
        _stdout: &str,
        _stderr: &str,
        _root: &std::path::Path,
        _tool: &str,
        _stack: &str,
    ) -> o8v_core::diagnostic::ParseResult {
        o8v_core::diagnostic::ParseResult {
            diagnostics: vec![dummy_diagnostic()],
            status: ParseStatus::Parsed,
            parsed_items: 1,
        }
    }

    /// Parser that returns zero diagnostics, Parsed.
    fn parse_zero_parsed(
        _stdout: &str,
        _stderr: &str,
        _root: &std::path::Path,
        _tool: &str,
        _stack: &str,
    ) -> o8v_core::diagnostic::ParseResult {
        o8v_core::diagnostic::ParseResult {
            diagnostics: vec![],
            status: ParseStatus::Parsed,
            parsed_items: 0,
        }
    }

    /// Parser that returns zero diagnostics, Unparsed.
    fn parse_zero_unparsed(
        _stdout: &str,
        _stderr: &str,
        _root: &std::path::Path,
        _tool: &str,
        _stack: &str,
    ) -> o8v_core::diagnostic::ParseResult {
        o8v_core::diagnostic::ParseResult {
            diagnostics: vec![],
            status: ParseStatus::Unparsed,
            parsed_items: 0,
        }
    }

    // ─── Passed transitions ────────────────────────────────────────────

    #[test]
    fn passed_with_diagnostics_promotes_to_failed() {
        let outcome = CheckOutcome::passed(
            "some output".to_string(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_one_diag);
        assert!(
            matches!(result, CheckOutcome::Failed { ref diagnostics, .. } if diagnostics.len() == 1),
            "expected Failed with 1 diagnostic, got {result:?}"
        );
    }

    #[test]
    fn passed_clean_stays_passed() {
        let outcome = CheckOutcome::passed(
            String::new(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_parsed);
        assert!(
            matches!(result, CheckOutcome::Passed { ref diagnostics, parse_status, .. }
                if diagnostics.is_empty() && parse_status == ParseStatus::Parsed),
            "expected Passed with 0 diagnostics, got {result:?}"
        );
    }

    #[test]
    fn passed_unparsed_nonempty_becomes_error() {
        let outcome = CheckOutcome::passed(
            "unexpected output".to_string(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_unparsed);
        assert!(
            matches!(
                result,
                CheckOutcome::Error {
                    kind: ErrorKind::Verification,
                    ..
                }
            ),
            "expected Error(Verification), got {result:?}"
        );
    }

    #[test]
    fn passed_unparsed_truncated_stays_passed() {
        let outcome = CheckOutcome::passed(
            "some truncated output".to_string(),
            String::new(),
            ParseStatus::Parsed,
            true,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_unparsed);
        assert!(
            matches!(result, CheckOutcome::Passed { .. }),
            "expected Passed (truncation exception), got {result:?}"
        );
    }

    #[test]
    fn passed_unparsed_empty_stdout_stays_passed() {
        let outcome = CheckOutcome::passed(
            "   ".to_string(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_unparsed);
        assert!(
            matches!(result, CheckOutcome::Passed { .. }),
            "expected Passed (empty stdout), got {result:?}"
        );
    }

    #[test]
    fn passed_unparsed_stderr_only_becomes_error() {
        let outcome = CheckOutcome::passed(
            String::new(),
            "error: some deno diagnostic".to_string(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_unparsed);
        assert!(
            matches!(
                result,
                CheckOutcome::Error {
                    kind: ErrorKind::Verification,
                    ..
                }
            ),
            "expected Error(Verification) for stderr-only unparsed, got {result:?}"
        );
    }

    // ─── Failed transitions ────────────────────────────────────────────

    #[test]
    fn failed_preserves_exit_code_through_enrichment() {
        let outcome = CheckOutcome::failed(
            Some(42),
            vec![],
            "error output".to_string(),
            String::new(),
            ParseStatus::Unparsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_one_diag);
        match result {
            CheckOutcome::Failed { code, .. } => {
                assert_eq!(code, Some(42), "exit code should survive enrichment");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn failed_with_diagnostics_enriched() {
        let outcome = CheckOutcome::failed(
            None,
            vec![],
            "error output".to_string(),
            String::new(),
            ParseStatus::Unparsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_one_diag);
        assert!(
            matches!(result, CheckOutcome::Failed { ref diagnostics, parse_status, .. }
                if diagnostics.len() == 1 && parse_status == ParseStatus::Parsed),
            "expected Failed with 1 diagnostic (Parsed), got {result:?}"
        );
    }

    #[test]
    fn failed_zero_diagnostics_forces_unparsed() {
        let outcome = CheckOutcome::failed(
            None,
            vec![],
            "some unknown output".to_string(),
            String::new(),
            ParseStatus::Unparsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_parsed);
        assert!(
            matches!(
                result,
                CheckOutcome::Failed {
                    parse_status: ParseStatus::Unparsed,
                    ..
                }
            ),
            "expected Failed with Unparsed (forced fallback), got {result:?}"
        );
    }

    #[test]
    fn failed_zero_diagnostics_no_output_stays_parsed() {
        let outcome = CheckOutcome::failed(
            None,
            vec![],
            String::new(),
            String::new(),
            ParseStatus::Unparsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_parsed);
        assert!(
            matches!(
                result,
                CheckOutcome::Failed {
                    parse_status: ParseStatus::Parsed,
                    ..
                }
            ),
            "expected Failed with Parsed (no output to fallback to), got {result:?}"
        );
    }

    #[test]
    fn failed_stderr_only_forces_unparsed() {
        let outcome = CheckOutcome::failed(
            None,
            vec![],
            String::new(),
            "error on stderr".to_string(),
            ParseStatus::Unparsed,
            false,
            false,
        );
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_zero_parsed);
        assert!(
            matches!(
                result,
                CheckOutcome::Failed {
                    parse_status: ParseStatus::Unparsed,
                    ..
                }
            ),
            "expected Failed with Unparsed (stderr-only fallback), got {result:?}"
        );
    }

    // ─── Error pass-through ────────────────────────────────────────────

    #[test]
    fn error_passes_through() {
        let outcome = CheckOutcome::error(ErrorKind::Runtime, "spawn failed".to_string());
        let pp = project_path();
        let result = enrich(outcome, &pp, "test", "test", parse_one_diag);
        assert!(
            matches!(result, CheckOutcome::Error { kind: ErrorKind::Runtime, ref cause, .. } if cause == "spawn failed"),
            "expected Error(Runtime) pass-through, got {result:?}"
        );
    }

    #[test]
    fn parser_panic_is_recovered() {
        /// Parser that panics with a string message.
        fn parse_panics(
            _stdout: &str,
            _stderr: &str,
            _root: &std::path::Path,
            _tool: &str,
            _stack: &str,
        ) -> o8v_core::diagnostic::ParseResult {
            panic!("parser bug: invalid state");
        }

        let outcome = CheckOutcome::passed(
            "some output".to_string(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        );
        let pp = project_path();
        // This should not crash. It should return unparsed and log the panic message.
        let result = enrich(outcome, &pp, "test", "test", parse_panics);
        // With unparsed output and non-empty stdout, should become an error.
        assert!(
            matches!(
                result,
                CheckOutcome::Error {
                    kind: ErrorKind::Verification,
                    ..
                }
            ),
            "panic should be recovered and output treated as unparsed error, got {result:?}"
        );
    }
}
