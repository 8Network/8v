// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Plain text rendering for CheckReport — token-efficient output for AI agents.
//!
//! Every token carries information. No colors, no symbols, no ANSI codes.
//! One line per check. Paginated error output with ANSI stripped.
//! For programmatic piping, use JSON (`--json`) which has unambiguous fields.

use std::fmt::Write as FmtWrite;

use crate::diagnostic::{Diagnostic, Location, ParseStatus};
use crate::{CheckEntry, CheckOutcome, CheckReport};

/// Render a `CheckReport` as plain text.
///
/// Token-efficient format for AI agents. No color, no symbols.
/// One line per check. Paginated error detail, ANSI stripped.
pub(in crate::render) fn render_check_plain(
    report: &CheckReport,
    config: &super::RenderConfig,
) -> super::Output {
    let mut buf = String::new();

    for err in report.detection_errors() {
        let msg = super::sanitize_for_display(&err.to_string());
        writeln!(buf, "detection error: {msg}").unwrap();
    }

    for result in report.results() {
        // Project header
        let name = super::sanitize_for_display(result.project_name());
        let stack = result.stack();
        if config.verbose {
            let path = super::sanitize_for_display(&result.project_path().to_string());
            writeln!(buf, "{name} {stack} {path}").unwrap();
        } else {
            writeln!(buf, "{name} {stack}").unwrap();
        }

        for entry in result.entries() {
            write_entry(&mut buf, entry, config);
        }
    }

    write_summary(&mut buf, report);

    super::Output::new(buf)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn write_entry(buf: &mut String, entry: &CheckEntry, config: &super::RenderConfig) {
    let ms = entry.duration().as_millis();
    match entry.outcome() {
        CheckOutcome::Passed {
            parse_status,
            stdout_truncated,
            stderr_truncated,
            ..
        } => {
            let mut note = String::new();
            if *stdout_truncated || *stderr_truncated {
                note.push_str(" (output truncated)");
            }
            if *parse_status == ParseStatus::Unparsed {
                note.push_str(" (unparsed)");
            }
            writeln!(buf, "{} passed {ms}ms{note}", entry.name()).unwrap();
        }
        CheckOutcome::Failed {
            diagnostics,
            raw_stdout,
            raw_stderr,
            parse_status,
            ..
        } => {
            writeln!(
                buf,
                "{} failed {ms}ms {} diagnostics",
                entry.name(),
                diagnostics.len()
            )
            .unwrap();
            if !diagnostics.is_empty() {
                write_diagnostics_plain(buf, diagnostics, config.limit);
            } else if *parse_status == ParseStatus::Unparsed {
                // Show both streams — don't drop stderr when stdout present.
                if !raw_stdout.is_empty() {
                    write_paginated(buf, raw_stdout, config.limit, config.page);
                }
                if !raw_stderr.is_empty() {
                    write_paginated(buf, raw_stderr, config.limit, config.page);
                }
            }
        }
        CheckOutcome::Error {
            cause,
            raw_stdout,
            raw_stderr,
            ..
        } => {
            let cause = super::sanitize_for_display(cause);
            writeln!(buf, "{} error {ms}ms {cause}", entry.name()).unwrap();
            // Show both streams.
            if !raw_stdout.is_empty() {
                write_paginated(buf, raw_stdout, config.limit, config.page);
            }
            if !raw_stderr.is_empty() {
                write_paginated(buf, raw_stderr, config.limit, config.page);
            }
        }
        #[allow(unreachable_patterns)]
        other => {
            tracing::warn!(
                "unknown CheckOutcome variant for '{}': {other:?}",
                entry.name()
            );
            writeln!(buf, "{} unknown {ms}ms", entry.name()).unwrap();
        }
    }
}

fn write_summary(buf: &mut String, report: &CheckReport) {
    let s = super::Summary::from_report(report);
    writeln!(buf, "---").unwrap();
    if s.passed == 0 && s.failed == 0 && s.errors == 0 && s.detection_errors == 0 {
        writeln!(
            buf,
            "no projects detected  {}ms",
            s.total_duration.as_millis()
        )
        .unwrap();
    } else {
        let result_label = if s.success { "pass" } else { "fail" };
        write!(buf, "result: {result_label}").unwrap();
        write!(
            buf,
            " {} passed {} failed {} errors",
            s.passed, s.failed, s.errors
        )
        .unwrap();
        if s.detection_errors > 0 {
            write!(buf, " {} detection_errors", s.detection_errors).unwrap();
        }
        writeln!(buf, " {}ms", s.total_duration.as_millis()).unwrap();
    }

    if let Some(delta) = &report.delta {
        writeln!(
            buf,
            "delta: +{} new, -{} fixed, {} unchanged",
            delta.new, delta.fixed, delta.unchanged
        )
        .unwrap();
    }
}

/// Write structured diagnostics — one line per diagnostic, token-efficient.
fn write_diagnostics_plain(buf: &mut String, diagnostics: &[Diagnostic], limit: Option<usize>) {
    let max_show = match limit {
        Some(0) | None => usize::MAX,
        Some(n) => n,
    };

    for (i, d) in diagnostics.iter().enumerate() {
        if i >= max_show {
            let remaining = diagnostics.len().saturating_sub(i);
            writeln!(buf, "  … {remaining} more diagnostics").unwrap();
            break;
        }

        let file = match &d.location {
            Location::File(f) | Location::Absolute(f) => f.as_str(),
        };

        let loc = d
            .span
            .as_ref()
            .map(|s| match (s.end_line, s.end_column) {
                (Some(el), Some(ec)) if el != s.line || ec != s.column => {
                    format!(":{}:{}-{}:{}", s.line, s.column, el, ec)
                }
                (Some(el), None) if el != s.line => {
                    format!(":{}:{}-{}", s.line, s.column, el)
                }
                _ => format!(":{}:{}", s.line, s.column),
            })
            .unwrap_or_default();

        let rule = d.rule.as_deref().unwrap_or("-");
        let sev = d.severity;

        writeln!(buf, "  {file}{loc} {sev} {rule} {}", d.message.as_str()).unwrap();
    }
}

/// Max bytes per line in paginated output. Lines longer than this are truncated
/// with a marker. Prevents single-line floods (e.g., 2MB of minified JS on one line).
const MAX_LINE_BYTES: usize = 4096;

/// Write error output lines, paginated by limit and page. ANSI codes stripped.
/// Individual lines are capped at MAX_LINE_BYTES to prevent single-line floods.
/// Page 1 shows lines 0..limit, page 2 shows limit..2*limit, etc.
fn write_paginated(buf: &mut String, output: &str, limit: Option<usize>, page: usize) {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();

    let (start, show) = match limit {
        Some(0) | None => (0, total),
        Some(n) => {
            let p = page.max(1);
            let start = (p - 1) * n;
            let end = (p * n).min(total);
            // If start is past the end, clamp to show nothing.
            let show = end.saturating_sub(start);
            (start, show)
        }
    };

    for line in &lines[start..start + show] {
        let sanitized = super::sanitize_for_display(line);
        if sanitized.len() > MAX_LINE_BYTES {
            let truncated = &sanitized[..sanitized.floor_char_boundary(MAX_LINE_BYTES)];
            writeln!(buf, "  {truncated} … (line truncated)").unwrap();
        } else {
            writeln!(buf, "  {sanitized}").unwrap();
        }
    }

    let remaining = total.saturating_sub(start + show);
    if remaining > 0 {
        let next_page = page.max(1) + 1;
        let limit_n = match limit {
            Some(0) | None => remaining,
            Some(n) => n,
        };
        writeln!(
            buf,
            "  … {remaining} more lines (--page {next_page} for next {limit_n})"
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DisplayStr;

    #[test]
    fn write_paginated_truncates_long_line() {
        let long_line = "x".repeat(10_000);
        let mut buf = String::new();
        write_paginated(&mut buf, &long_line, None, 1);
        assert!(
            buf.contains("line truncated"),
            "long line should be truncated: len={}",
            buf.len()
        );
        assert!(
            buf.len() < 6000,
            "output should be much less than 10K: {}",
            buf.len()
        );
    }

    #[test]
    fn write_paginated_short_line_untouched() {
        let mut buf = String::new();
        write_paginated(&mut buf, "short line", None, 1);
        assert!(buf.contains("short line"));
        assert!(!buf.contains("truncated"));
    }

    fn make_diagnostic(msg: &str) -> Diagnostic {
        use crate::diagnostic::{Location, Severity};
        Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: Some(DisplayStr::from_untrusted("test-rule")),
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_untrusted(msg),
            related: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            snippet: None,
            tool: "test-tool".to_string(),
            stack: "test".to_string(),
        }
    }

    #[test]
    fn write_diagnostics_plain_limit_zero_shows_all() {
        let diagnostics = vec![
            make_diagnostic("first error"),
            make_diagnostic("second error"),
        ];

        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &diagnostics, Some(0));

        assert!(
            buf.contains("first error"),
            "limit=Some(0) should show all diagnostics (same as None)"
        );
        assert!(
            buf.contains("second error"),
            "limit=Some(0) should show all diagnostics (same as None)"
        );
        assert!(
            !buf.contains("more diagnostics"),
            "should not show remaining count when all shown"
        );
    }

    #[test]
    fn write_diagnostics_plain_limit_none_shows_all() {
        let diagnostics = vec![
            make_diagnostic("first error"),
            make_diagnostic("second error"),
        ];

        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &diagnostics, None);

        assert!(
            buf.contains("first error"),
            "limit=None should show all diagnostics"
        );
        assert!(
            buf.contains("second error"),
            "limit=None should show all diagnostics"
        );
        assert!(
            !buf.contains("more diagnostics"),
            "should not show remaining count when all shown"
        );
    }

    #[test]
    fn write_diagnostics_plain_limit_one_shows_single() {
        let diagnostics = vec![
            make_diagnostic("first error"),
            make_diagnostic("second error"),
        ];

        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &diagnostics, Some(1));

        assert!(
            buf.contains("first error"),
            "limit=Some(1) should show first diagnostic"
        );
        assert!(
            !buf.contains("second error"),
            "limit=Some(1) should suppress second diagnostic"
        );
        assert!(
            buf.contains("1 more diagnostic"),
            "should show remaining count"
        );
    }

    #[test]
    fn write_paginated_limit_zero_shows_all() {
        let output = "line 1\nline 2\nline 3";
        let mut buf = String::new();
        write_paginated(&mut buf, output, Some(0), 1);

        assert!(
            buf.contains("line 1"),
            "limit=Some(0) should show all lines (same as None)"
        );
        assert!(
            buf.contains("line 2"),
            "limit=Some(0) should show all lines (same as None)"
        );
        assert!(
            buf.contains("line 3"),
            "limit=Some(0) should show all lines (same as None)"
        );
        assert!(
            !buf.contains("more lines"),
            "should not show remaining count when all shown"
        );
    }

    #[test]
    fn write_paginated_limit_none_shows_all() {
        let output = "line 1\nline 2\nline 3";
        let mut buf = String::new();
        write_paginated(&mut buf, output, None, 1);

        assert!(buf.contains("line 1"), "limit=None should show all lines");
        assert!(buf.contains("line 2"), "limit=None should show all lines");
        assert!(buf.contains("line 3"), "limit=None should show all lines");
        assert!(
            !buf.contains("more lines"),
            "should not show remaining count when all shown"
        );
    }

    #[test]
    fn write_paginated_limit_one_shows_single() {
        let output = "line 1\nline 2\nline 3";
        let mut buf = String::new();
        write_paginated(&mut buf, output, Some(1), 1);

        assert!(
            buf.contains("line 1"),
            "limit=Some(1) should show first line"
        );
        assert!(
            !buf.contains("line 2"),
            "limit=Some(1) should suppress second line"
        );
        assert!(
            !buf.contains("line 3"),
            "limit=Some(1) should suppress third line"
        );
        assert!(buf.contains("2 more lines"), "should show remaining count");
    }

    #[test]
    fn write_diagnostics_plain_span_with_end_line_no_end_column() {
        use crate::diagnostic::Span;

        let mut diag = make_diagnostic("error with partial span");
        diag.span = Some(Span::new(5, 10, Some(10), None));

        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &[diag], None);

        // Should show `:5:10-10` (start line:col to end_line, but no end_column)
        assert!(
            buf.contains(":5:10-10"),
            "span with end_line but no end_column should show `:line:col-end_line`, got: {}",
            buf
        );
    }

    // ─── Security Tests ────────────────────────────────────────────────────

    #[test]
    fn security_huge_output_dos_protection() {
        // Test that huge output doesn't cause unbounded memory allocation or CPU exhaustion.
        // 100 MB of output should be processable with truncation.
        let huge_output = "x".repeat(100_000_000);
        let mut buf = String::new();
        write_paginated(&mut buf, &huge_output, Some(10), 1);
        assert!(buf.len() < huge_output.len(), "output should be truncated");
    }

    #[test]
    fn security_line_injection_newlines_in_message() {
        // If a diagnostic message contains newlines, they should be stripped during sanitization.
        // This prevents log injection where a single diagnostic becomes multiple log lines.
        let mut diag = make_diagnostic("error\nfaked_log_entry: all_passed");
        diag.sanitize();
        // After sanitization, the message should have no newline
        assert_eq!(diag.message, "errorfaked_log_entry: all_passed");
        // The newline character itself should not be present
        assert!(
            !diag.message.contains('\n'),
            "sanitize should remove newlines from message"
        );
    }

    #[test]
    fn security_ansi_escape_injection() {
        // ANSI sequences in diagnostic fields should be stripped before rendering
        let mut diag = make_diagnostic("\x1b]0;evil_title\x07message");
        diag.sanitize();
        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &[diag], None);
        // OSC sequences should be stripped
        assert!(!buf.contains("\x1b]"), "OSC sequences should be stripped");
        assert!(!buf.contains("evil_title"), "OSC title should be stripped");
    }

    #[test]
    fn security_control_char_injection() {
        // Control characters should be stripped
        let mut diag = make_diagnostic("message\x07\x01\x02\x7f");
        diag.sanitize();
        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &[diag], None);
        // Only printable chars should remain
        assert!(buf.contains("message"), "printable text should remain");
        assert!(!buf.contains("\x07"), "BEL should be stripped");
    }

    #[test]
    fn security_extremely_long_single_line() {
        // A single extremely long line should be truncated
        let long_line = "x".repeat(1_000_000);
        let mut buf = String::new();
        write_paginated(&mut buf, &long_line, None, 1);
        assert!(
            buf.contains("line truncated"),
            "very long line should be truncated"
        );
        assert!(buf.len() < 1_000_000 / 100, "output should be much smaller");
    }

    #[test]
    fn security_many_diagnostics_with_limit() {
        // With limit=Some(5), only 5 should be shown, rest suppressed
        let diagnostics: Vec<_> = (0..10000)
            .map(|i| make_diagnostic(&format!("diagnostic {}", i)))
            .collect();
        let mut buf = String::new();
        write_diagnostics_plain(&mut buf, &diagnostics, Some(5));
        // Only first 5 should appear
        assert!(
            buf.contains("diagnostic 0"),
            "first diagnostic should appear"
        );
        assert!(buf.contains("diagnostic 4"), "5th diagnostic should appear");
        assert!(
            !buf.contains("diagnostic 5"),
            "6th diagnostic should NOT appear"
        );
        assert!(buf.contains("9995 more"), "remaining count should appear");
    }
}
