//! TypeScript compiler text parser — covers `tsc --noEmit --pretty false`.
//!
//! tsc emits diagnostics on stdout in the format:
//! `file.ts(line,col): error TSxxxx: message`
//!
//! Also used for `deno check` which emits a similar format.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse tsc text output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // deno check writes diagnostics to stderr, not stdout.
    // Use stderr when stdout is empty.
    let input = if stdout.trim().is_empty() && !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let mut diagnostics = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(d) = parse_line(line, project_root, tool, stack) {
            diagnostics.push(d);
        }
    }

    // Text parser: we scanned every line looking for patterns. If we found
    // none, the output is clean — not "unparsed". Parsed with 0 diagnostics.
    let status = ParseStatus::Parsed;
    let parsed_items = diagnostics.len() as u32;

    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

/// Parse one tsc diagnostic line: `file.ts(line,col): error TSxxxx: message`
fn parse_line(
    line: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Option<Diagnostic> {
    // Find the (line,col) group by searching backwards for a valid pattern.
    // Cannot use first '(' — filenames like handler(1).ts contain parens.
    let (file, line_num, col_num, close_paren) = super::find_location(line)?;

    // Rest after "): " is "error TSxxxx: message" or "warning TSxxxx: message"
    let rest = line[close_paren + 1..].trim();
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim();

    // Parse severity and code
    let (severity, raw_sev, rest) = match (rest.strip_prefix("error"), rest.strip_prefix("warning"))
    {
        (Some(r), _) => (Severity::Error, "error", r.trim()),
        (_, Some(r)) => (Severity::Warning, "warning", r.trim()),
        _ => return None, // Unknown severity: skip this diagnostic
    };

    // Parse TS code: "TSxxxx: message"
    let (rule, message) = rest.find(':').map_or_else(
        || (None, DisplayStr::from_untrusted(rest)),
        |colon_pos| {
            let code = rest[..colon_pos].trim();
            let msg = rest[colon_pos + 1..].trim();
            if code.starts_with("TS") {
                (
                    Some(DisplayStr::from_untrusted(code)),
                    DisplayStr::from_untrusted(msg),
                )
            } else {
                (None, DisplayStr::from_untrusted(rest))
            }
        },
    );

    let location = super::normalize_path(file, project_root);

    Some(Diagnostic {
        location,
        span: Some(Span::new(line_num, col_num, None, None)),
        rule,
        severity,
        raw_severity: Some(raw_sev.to_string()),
        message,
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str, stderr: &str) -> ParseResult {
        parse(stdout, stderr, Path::new(ROOT), "tsc", "typescript")
    }

    #[test]
    fn empty_stdout() {
        let result = run("", "");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_error() {
        let result = run(
            "src/app.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.\n",
            "",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("TS2322"));
        assert_eq!(
            d.message,
            "Type 'string' is not assignable to type 'number'."
        );
        assert_eq!(d.location, Location::File("src/app.ts".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 10);
        assert_eq!(span.column, 5);
    }

    #[test]
    fn warning() {
        let result = run(
            "src/app.ts(10,5): warning TS6133: 'x' is declared but never used.\n",
            "",
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("TS6133"));
    }

    #[test]
    fn no_ts_code() {
        let result = run("src/app.ts(10,5): error something went wrong\n", "");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule, None);
    }

    #[test]
    fn deno_stderr_fallback() {
        let result = run(
            "",
            "src/main.ts(3,1): error TS2304: Cannot find name 'foo'.\n",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("TS2304"));
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/main.ts".to_string())
        );
    }

    #[test]
    fn parens_in_filename() {
        let result = run("handler(1).ts(5,3): error TS2322: msg\n", "");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("handler(1).ts".to_string())
        );
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 3);
    }

    #[test]
    fn unknown_severity_is_skipped() {
        // Unknown severity word like "info" or "note" should be SKIPPED, not
        // silently defaulted to Error.
        let result = run("src/app.ts(10,5): info TS2322: some message\n", "");
        assert_eq!(
            result.diagnostics.len(),
            0,
            "Unknown severity should skip the diagnostic"
        );
    }

    #[test]
    fn known_severities_still_parse() {
        // Ensure we still parse the known ones correctly after the fix.
        let result_error = run("src/app.ts(10,5): error TS2322: msg\n", "");
        assert_eq!(result_error.diagnostics.len(), 1);
        assert_eq!(result_error.diagnostics[0].severity, Severity::Error);

        let result_warning = run("src/app.ts(10,5): warning TS2322: msg\n", "");
        assert_eq!(result_warning.diagnostics.len(), 1);
        assert_eq!(result_warning.diagnostics[0].severity, Severity::Warning);
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_tsc_empty_input() {
        let result = run("", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_tsc_huge_input() {
        let line = "src/app.ts(10,5): error TS2322: message\n";
        let huge = line.repeat(100_000);
        let result = run(&huge, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_tsc_binary_garbage() {
        let garbage = "src/app.ts(10,5): error\x00\x01\x02";
        let result = run(garbage, "");
        // Should not panic; will try to parse and fail gracefully
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
    }

    #[test]
    fn stress_tsc_whitespace_only() {
        let result = run("   \n\n\t\t\n   ", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_tsc_truncated_line() {
        let truncated = "src/app.ts(10,5): error TS23";
        let result = run(truncated, "");
        // Should parse but extract partial information gracefully
        assert_eq!(result.status, ParseStatus::Parsed);
        // May or may not extract a diagnostic depending on parser logic
        assert!(result.diagnostics.len() < 10);
    }

    #[test]
    fn stress_tsc_unicode_in_paths() {
        let line = "src/文件.ts(10,5): error TS2322: 错误消息 🔥 עברית\n";
        let result = run(line, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should handle Unicode without panicking without crashing
        // If we got a diagnostic, verify it exists
        if !result.diagnostics.is_empty() {
            let _ = &result.diagnostics[0];
        }
    }

    #[test]
    fn stress_tsc_extremely_long_line() {
        let long_msg = "x".repeat(1_000_000);
        let line = format!("src/app.ts(10,5): error TS2322: {}\n", long_msg);
        let result = run(&line, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should not crash on 1MB+ line
    }

    #[test]
    fn stress_tsc_malformed_location() {
        let lines = vec![
            "src/app.ts(0,0): error TS2322: zero coords\n",
            "src/app.ts(): error TS2322: empty parens\n",
            "src/app.ts(invalid,invalid): error TS2322: non-numeric\n",
            "src/app.ts(999999999,999999999): error TS2322: huge coords\n",
        ];
        for line in lines {
            let result = run(line, "");
            // Should not panic on any malformed location
            assert!(matches!(
                result.status,
                ParseStatus::Parsed | ParseStatus::Unparsed
            ));
        }
    }
}
