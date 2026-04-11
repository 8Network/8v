// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Human-readable (colored, aligned) rendering for `CheckReport`.
//!
//! Project name bold, stack dim. Check names aligned. Symbols: ✓ ✗ !.
//! Error output in bordered boxes. Paginated.

use crate::diagnostic::{Diagnostic, Location, ParseStatus, Severity};
use crate::{CheckOutcome, CheckReport};

// ANSI codes
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Render a `CheckReport` as colored, aligned human-readable output.
pub(in crate::render) fn render_check_human(
    report: &CheckReport,
    config: &super::RenderConfig,
) -> super::Output {
    let mut buf = String::new();
    let c = config.color;

    for err in report.detection_errors() {
        let msg = super::sanitize_for_display(&err.to_string());
        buf.push_str(&format!(
            "  {}error:{} {msg}\n",
            color(c, RED),
            color(c, RESET)
        ));
    }

    for result in report.results() {
        // Project header
        buf.push('\n');
        let name = super::sanitize_for_display(result.project_name());
        let stack = result.stack();
        if config.verbose {
            let path = super::sanitize_for_display(&result.project_path().to_string());
            buf.push_str(&format!(
                "  {}{}{} {}{}{} ({})\n",
                color(c, BOLD),
                name,
                color(c, RESET),
                color(c, DIM),
                stack,
                color(c, RESET),
                path
            ));
        } else {
            buf.push_str(&format!(
                "  {}{}{} {}{}{}\n",
                color(c, BOLD),
                name,
                color(c, RESET),
                color(c, DIM),
                stack,
                color(c, RESET)
            ));
        }
        buf.push('\n');

        // Compute alignment width from longest check name in this group.
        let max_name = result
            .entries()
            .iter()
            .map(|e| e.name().len())
            .max()
            .unwrap_or(0);

        // Two-pass layout: all status lines, then all diagnostics.
        for entry in result.entries() {
            write_check_line(&mut buf, entry, max_name, c);
        }
        for entry in result.entries() {
            write_entry_detail(&mut buf, entry, config, c);
        }
    }

    // Summary
    buf.push('\n');
    write_summary(&mut buf, report, c);

    super::Output::new(buf)
}

/// Write diagnostics/error detail for one check entry.
fn write_entry_detail(
    buf: &mut String,
    entry: &crate::CheckEntry,
    config: &super::RenderConfig,
    c: bool,
) {
    match entry.outcome() {
        CheckOutcome::Failed {
            diagnostics,
            raw_stdout,
            raw_stderr,
            parse_status,
            ..
        } => {
            if !diagnostics.is_empty() {
                buf.push('\n');
                write_diagnostics(buf, diagnostics, config.limit, c);
            } else if *parse_status == ParseStatus::Unparsed {
                // Show both streams — don't drop stderr when stdout is present.
                let mut combined = String::new();
                if !raw_stdout.is_empty() {
                    combined.push_str(raw_stdout);
                }
                if !raw_stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(raw_stderr);
                }
                if !combined.is_empty() {
                    buf.push('\n');
                    write_error_box(buf, entry.name(), &combined, config.limit, c);
                }
            }
        }
        CheckOutcome::Error {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            // Show both streams — don't drop stderr when stdout is present.
            let mut output = String::new();
            if !raw_stdout.is_empty() {
                output.push_str(raw_stdout);
            }
            if !raw_stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(raw_stderr);
            }
            if !output.is_empty() {
                buf.push('\n');
                write_error_box(buf, entry.name(), &output, config.limit, c);
            }
        }
        CheckOutcome::Passed { .. } => {
            // Passed checks have no detail to display.
        }
        #[allow(unreachable_patterns)]
        other => {
            tracing::warn!(
                "unknown CheckOutcome variant for '{}': {other:?}",
                entry.name()
            );
        }
    }
}

/// Write a single check status line.
fn write_check_line(buf: &mut String, entry: &crate::CheckEntry, max_name: usize, c: bool) {
    let padded = format!("{:width$}", entry.name(), width = max_name);
    let dur = format_duration(entry.duration());

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
            if *parse_status == crate::diagnostic::ParseStatus::Unparsed {
                note.push_str(" (unparsed)");
            }
            buf.push_str(&format!(
                "    {padded}  {}✓{}   {}{dur}{}{}{}{}",
                color(c, GREEN),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET),
                color(c, DIM),
                note,
                color(c, RESET),
            ));
            buf.push('\n');
        }
        CheckOutcome::Failed { .. } => {
            buf.push_str(&format!(
                "    {padded}  {}✗{}   {}{dur}{}\n",
                color(c, RED),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET)
            ));
        }
        CheckOutcome::Error { cause, .. } => {
            let cause = super::sanitize_for_display(cause);
            buf.push_str(&format!(
                "    {padded}  {}!{}   {}{cause}{}\n",
                color(c, YELLOW),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET)
            ));
        }
        #[allow(unreachable_patterns)]
        other => {
            tracing::warn!(
                "unknown CheckOutcome variant for '{}': {other:?}",
                entry.name()
            );
            buf.push_str(&format!(
                "    {padded}  {}?{}   unknown\n",
                color(c, YELLOW),
                color(c, RESET)
            ));
        }
    }
}

/// Write structured diagnostics grouped by file.
fn write_diagnostics(buf: &mut String, diagnostics: &[Diagnostic], limit: Option<usize>, c: bool) {
    // Group by file.
    let mut by_file: std::collections::BTreeMap<&str, Vec<&Diagnostic>> =
        std::collections::BTreeMap::new();
    for d in diagnostics {
        let file = match &d.location {
            Location::File(f) | Location::Absolute(f) => f.as_str(),
        };
        by_file.entry(file).or_default().push(d);
    }

    let max_show = match limit {
        Some(0) | None => usize::MAX,
        Some(n) => n,
    };
    let mut shown = 0usize;

    'outer: for (file, diags) in &by_file {
        // Check limit BEFORE printing the file header — no orphan headers.
        if shown >= max_show {
            let remaining = diagnostics.len().saturating_sub(shown);
            if remaining > 0 {
                buf.push_str(&format!(
                    "    {}… {remaining} more diagnostics{}\n",
                    color(c, DIM),
                    color(c, RESET)
                ));
            }
            break 'outer;
        }

        buf.push_str(&format!(
            "    {}{file}{}\n",
            color(c, BOLD),
            color(c, RESET)
        ));

        for d in diags {
            if shown >= max_show {
                let remaining = diagnostics.len().saturating_sub(shown);
                if remaining > 0 {
                    buf.push_str(&format!(
                        "    {}… {remaining} more diagnostics{}\n",
                        color(c, DIM),
                        color(c, RESET)
                    ));
                }
                break 'outer;
            }

            let loc = d
                .span
                .as_ref()
                .map_or(String::new(), |s| match (s.end_line, s.end_column) {
                    (Some(el), Some(ec)) if el != s.line || ec != s.column => {
                        format!("{}:{}-{}:{}", s.line, s.column, el, ec)
                    }
                    (Some(el), None) if el != s.line => {
                        format!("{}:{}-{}", s.line, s.column, el)
                    }
                    _ => format!("{}:{}", s.line, s.column),
                });

            let sev_color = match d.severity {
                Severity::Error => RED,
                Severity::Warning => YELLOW,
                _ => DIM,
            };

            let rule = d.rule.as_deref().unwrap_or("");

            buf.push_str(&format!(
                "      {}{loc:>8}{}  {}{:7}{}  {}  {}{}{}\n",
                color(c, DIM),
                color(c, RESET),
                color(c, sev_color),
                d.severity,
                color(c, RESET),
                d.message.as_str(),
                color(c, DIM),
                rule,
                color(c, RESET),
            ));

            shown += 1;
        }

        buf.push('\n');
    }
}

/// Write a bordered error box for a failed check.
fn write_error_box(buf: &mut String, name: &str, output: &str, limit: Option<usize>, c: bool) {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();
    // 0 means "no limit" — show everything.
    let show = match limit {
        Some(0) | None => total,
        Some(n) => n.min(total),
    };

    buf.push_str(&format!(
        "    {}┌ {name}{}\n",
        color(c, RED),
        color(c, RESET)
    ));

    const MAX_LINE_BYTES: usize = 4096;
    for line in &lines[..show] {
        let clean = super::sanitize_for_display(line);
        if clean.len() > MAX_LINE_BYTES {
            let truncated = &clean[..clean.floor_char_boundary(MAX_LINE_BYTES)];
            buf.push_str(&format!(
                "    {}│{} {truncated} … (line truncated)\n",
                color(c, RED),
                color(c, RESET)
            ));
        } else {
            buf.push_str(&format!(
                "    {}│{} {clean}\n",
                color(c, RED),
                color(c, RESET)
            ));
        }
    }

    let remaining = total.saturating_sub(show);
    if remaining > 0 {
        buf.push_str(&format!(
            "    {}│{} {}{remaining} more lines{}\n",
            color(c, RED),
            color(c, RESET),
            color(c, DIM),
            color(c, RESET)
        ));
    }

    buf.push_str(&format!("    {}└{}\n", color(c, RED), color(c, RESET)));
}

/// Write the summary line.
fn write_summary(buf: &mut String, report: &CheckReport, c: bool) {
    let s = super::Summary::from_report(report);
    let total = s.passed + s.failed + s.errors;
    let passed = s.passed;
    let failed = s.failed;
    let errors = s.errors;
    let det = s.detection_errors;
    let total_duration = s.total_duration;

    if total == 0 && det == 0 {
        buf.push_str(&format!(
            "  {}no projects detected{}",
            color(c, YELLOW),
            color(c, RESET)
        ));
    } else if failed == 0 && errors == 0 && det == 0 && total > 0 {
        buf.push_str(&format!(
            "  {}✓ all passed{}",
            color(c, GREEN),
            color(c, RESET)
        ));
    } else {
        buf.push(' ');
        if failed > 0 {
            buf.push_str(&format!(
                " {}{failed} failed{}",
                color(c, RED),
                color(c, RESET)
            ));
        }
        if passed > 0 {
            buf.push_str(&format!(
                "  {}{passed} passed{}",
                color(c, GREEN),
                color(c, RESET)
            ));
        }
        if errors > 0 {
            buf.push_str(&format!(
                "  {}{errors} error{}{}",
                color(c, YELLOW),
                if errors == 1 { "" } else { "s" },
                color(c, RESET)
            ));
        }
        if det > 0 {
            buf.push_str(&format!(
                "  {}{det} detection error{}{}",
                color(c, YELLOW),
                if det == 1 { "" } else { "s" },
                color(c, RESET)
            ));
        }
    }

    buf.push_str(&format!(
        "  {}{}{}\n",
        color(c, DIM),
        format_duration(total_duration),
        color(c, RESET)
    ));

    if let Some(delta) = &report.delta {
        buf.push_str(&format!(
            "  {}{} new{}  {}{} fixed{}  {}{} unchanged{}\n",
            color(c, RED),
            delta.new,
            color(c, RESET),
            color(c, GREEN),
            delta.fixed,
            color(c, RESET),
            color(c, DIM),
            delta.unchanged,
            color(c, RESET),
        ));
    }
}

/// Return ANSI code if color is enabled, empty string otherwise.
const fn color(enabled: bool, code: &str) -> &str {
    if enabled {
        code
    } else {
        ""
    }
}

/// Format a duration for human display. Compact style: `123ms`, `1.5s`.
fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::sanitize_for_display;

    #[test]
    fn write_error_box_truncates_long_line() {
        let long_line = "y".repeat(10_000);
        let mut buf = String::new();
        write_error_box(&mut buf, "test", &long_line, None, false);
        assert!(
            buf.contains("line truncated"),
            "long line should be truncated"
        );
        assert!(
            buf.len() < 6000,
            "output should be much less than 10K: {}",
            buf.len()
        );
    }

    #[test]
    fn write_error_box_short_line_untouched() {
        let mut buf = String::new();
        write_error_box(&mut buf, "test", "short error", None, false);
        assert!(buf.contains("short error"));
        assert!(!buf.contains("truncated"));
    }

    #[test]
    fn write_diagnostics_span_with_end_line_no_end_column() {
        use crate::diagnostic::{Location, Severity, Span};
        use crate::DisplayStr;

        let diag = Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: Some(Span::new(3, 5, Some(7), None)),
            rule: Some(DisplayStr::from_untrusted("test-rule")),
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_untrusted("test error"),
            related: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            snippet: None,
            tool: "test-tool".to_string(),
            stack: "test".to_string(),
        };

        let mut buf = String::new();
        write_diagnostics(&mut buf, &[diag], None, false);

        // Should show `3:5-7` (start line:col to end_line, but no end_column)
        assert!(
            buf.contains("3:5-7"),
            "span with end_line but no end_column should show `line:col-end_line`, got: {}",
            buf
        );
    }

    #[test]
    fn write_error_box_limit_respected() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let mut buf = String::new();
        write_error_box(&mut buf, "test", output, Some(2), false);
        assert!(buf.contains("line1"));
        assert!(buf.contains("line2"));
        assert!(!buf.contains("line3"));
        assert!(buf.contains("3 more lines"));
    }

    #[test]
    fn write_error_box_zero_limit_shows_all() {
        let output = "line1\nline2\nline3";
        let mut buf = String::new();
        write_error_box(&mut buf, "test", output, Some(0), false);
        assert!(buf.contains("line1"));
        assert!(buf.contains("line2"));
        assert!(buf.contains("line3"));
        assert!(!buf.contains("more lines"));
    }

    #[test]
    fn format_duration_millis() {
        assert_eq!(
            format_duration(std::time::Duration::from_millis(42)),
            "42ms"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_millis(999)),
            "999ms"
        );
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(
            format_duration(std::time::Duration::from_millis(1500)),
            "1.5s"
        );
        assert_eq!(format_duration(std::time::Duration::from_secs(2)), "2.0s");
    }

    #[test]
    fn color_disabled_returns_empty() {
        assert_eq!(color(false, RED), "");
        assert_eq!(color(false, GREEN), "");
        assert_eq!(color(false, BOLD), "");
    }

    #[test]
    fn color_enabled_returns_code() {
        assert_eq!(color(true, RED), RED);
        assert_eq!(color(true, GREEN), GREEN);
    }

    #[test]
    fn sanitize_strips_ansi_in_check_human() {
        // Verify sanitize_for_display is correctly wired through the module.
        let result = sanitize_for_display("\x1b[31mbold\x1b[0m");
        assert_eq!(result, "bold");
    }

    #[test]
    fn write_diagnostics_limit_prevents_orphan_headers() {
        use crate::diagnostic::{Location, Severity, Span};
        use crate::DisplayStr;

        // 3 diagnostics in different files, limit = 1
        let diags: Vec<Diagnostic> = (0..3)
            .map(|i| Diagnostic {
                location: Location::File(format!("file{i}.rs")),
                span: Some(Span::new(1, 1, None, None)),
                rule: None,
                severity: Severity::Error,
                raw_severity: None,
                message: DisplayStr::from_untrusted(format!("error {i}")),
                related: Vec::new(),
                notes: Vec::new(),
                suggestions: Vec::new(),
                snippet: None,
                tool: "tool".to_string(),
                stack: "test".to_string(),
            })
            .collect();

        let mut buf = String::new();
        write_diagnostics(&mut buf, &diags, Some(1), false);

        // Only the first file header should appear (limit = 1, so after first diag we stop).
        assert!(buf.contains("file0.rs"), "first file should be shown");
        // The remaining count message should appear.
        assert!(
            buf.contains("more diagnostics"),
            "should show remaining count"
        );
    }
}
