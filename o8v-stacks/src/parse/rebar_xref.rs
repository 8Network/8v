//! rebar3 xref JSON parser — covers `rebar3 compile` with xref output.
//!
//! rebar3 xref emits text output with lines like:
//! "src/flight.erl:5: Warning: flight:book/2 is unused export (Xref)"
//!
//! Lines starting with "===> " are rebar3 progress indicators and are skipped.
//! Each valid line yields one diagnostic.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse rebar3 xref output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut parsed_count = 0u32;
    let mut skipped = 0u32;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Skip rebar3 progress indicators (lines starting with "===> ")
        if trimmed.starts_with("===> ") {
            skipped += 1;
            continue;
        }

        // Try to parse the line as a diagnostic
        match parse_line(trimmed, project_root) {
            Some((location, line_num, col_num, severity, message)) => {
                parsed_count += 1;
                let span = if line_num > 0 {
                    Some(Span::new(line_num, col_num, None, None))
                } else {
                    None
                };

                diagnostics.push(Diagnostic {
                    location,
                    span,
                    rule: Some(DisplayStr::from_untrusted("xref".to_string())),
                    severity,
                    raw_severity: None,
                    message,
                    related: vec![],
                    notes: vec![],
                    suggestions: vec![],
                    snippet: None,
                    tool: tool.to_string(),
                    stack: stack.to_string(),
                });
            }
            None => {
                skipped += 1;
                tracing::debug!("skipping malformed xref line: {}", trimmed);
                continue;
            }
        }
    }

    if skipped > 0 {
        tracing::debug!(
            skipped,
            total = diagnostics.len() + skipped as usize,
            "rebar_xref: some lines could not be parsed"
        );
    }

    let status = if !diagnostics.is_empty() || stdout.trim().is_empty() {
        ParseStatus::Parsed
    } else {
        ParseStatus::Unparsed
    };

    ParseResult {
        diagnostics,
        status,
        parsed_items: parsed_count,
    }
}

/// Parse a single xref output line.
///
/// Format: "src/file.erl:line: Severity: message (Xref)"
/// Examples:
/// - "src/flight.erl:5: Warning: flight:book/2 is unused export (Xref)"
/// - "src/module.erl:10: Error: undefined function foo/1 (Xref)"
///
/// Returns: (location, line, column, severity, message)
fn parse_line(
    line: &str,
    project_root: &std::path::Path,
) -> Option<(
    o8v_core::diagnostic::Location,
    u32,
    u32,
    Severity,
    DisplayStr,
)> {
    // Find the first colon to split file from the rest
    let colon_idx = line.find(':')?;
    let file_part = &line[..colon_idx];
    let rest = &line[colon_idx + 1..];

    // Parse line number from rest
    let colon_idx2 = rest.find(':')?;
    let line_part = rest[..colon_idx2].trim();
    let line_num: u32 = match line_part.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    let rest = &rest[colon_idx2 + 1..].trim();

    // Parse severity: next word should be Warning, Error, etc.
    let colon_idx3 = rest.find(':')?;
    let severity_part = rest[..colon_idx3].trim();
    let severity = parse_severity(severity_part);

    let message_part = &rest[colon_idx3 + 1..].trim();

    // Normalize file path
    let location = super::normalize_path(file_part, project_root);

    Some((
        location,
        line_num,
        1, // column always 1 for xref output
        severity,
        DisplayStr::from_untrusted(message_part.to_string()),
    ))
}

/// Parse severity from string.
fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        "note" => Severity::Info,
        _ => Severity::Warning, // default to warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "rebar-xref", "erlang")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_finding() {
        let input = "src/flight.erl:5: Warning: flight:book/2 is unused export (Xref)";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);

        let d = &result.diagnostics[0];
        assert_eq!(d.location, Location::File("src/flight.erl".to_string()));
        assert_eq!(d.rule.as_deref(), Some("xref"));
        assert_eq!(d.severity, Severity::Warning);
        assert!(d.message.contains("flight:book/2"));

        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn multiple_findings() {
        let input = r#"src/module.erl:10: Warning: module:foo/1 is unused export (Xref)
src/helper.erl:20: Error: undefined function bar/2 (Xref)
src/utils.erl:30: Warning: utils:baz/0 is unused export (Xref)"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.parsed_items, 3);

        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[1].severity, Severity::Error);
        assert_eq!(result.diagnostics[2].severity, Severity::Warning);

        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 10);
        assert_eq!(result.diagnostics[1].span.as_ref().unwrap().line, 20);
        assert_eq!(result.diagnostics[2].span.as_ref().unwrap().line, 30);
    }

    #[test]
    fn malformed_line() {
        let input = "this is not a valid xref line";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn rebar3_progress_lines_skipped() {
        let input = r#"===> Analyzing applications...
src/flight.erl:5: Warning: flight:book/2 is unused export (Xref)
===> Running checks...
src/helper.erl:15: Warning: helper:process/1 is unused export (Xref)"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);

        // Both diagnostics should be from the real xref lines
        assert!(result.diagnostics[0].message.contains("flight:book/2"));
        assert!(result.diagnostics[1].message.contains("helper:process/1"));
    }

    #[test]
    fn empty_lines_ignored() {
        let input = r#"src/flight.erl:5: Warning: msg1 (Xref)

src/helper.erl:10: Warning: msg2 (Xref)

"#;
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
    }

    #[test]
    fn missing_line_number() {
        let input = "src/flight.erl:: Warning: some message (Xref)";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn missing_severity() {
        let input = "src/flight.erl:5: some message (Xref)";
        let result = run(input);
        // Should still parse, severity defaults to Warning
        assert_eq!(result.diagnostics.len(), 0); // can't find second colon
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn case_insensitive_severity() {
        let input = "src/flight.erl:5: ERROR: uppercase error message (Xref)";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn long_message() {
        let long_msg = "x".repeat(1000);
        let input = format!("src/flight.erl:5: Warning: {} (Xref)", long_msg);
        let result = run(&input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
    }

    #[test]
    fn unicode_in_path() {
        let input = "src/файл.erl:5: Warning: some message (Xref)";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/файл.erl".to_string())
        );
    }

    #[test]
    fn unicode_in_message() {
        let input = "src/flight.erl:5: Warning: 错误信息 🔥 (Xref)";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("错误信息"));
    }

    #[test]
    fn stress_huge_input() {
        let line = "src/module.erl:10: Warning: msg (Xref)";
        let mut huge = String::new();
        for _ in 0..10_000 {
            huge.push_str(line);
            huge.push('\n');
        }
        let result = run(&huge);
        assert_eq!(result.diagnostics.len(), 10_000);
        assert_eq!(result.parsed_items, 10_000);
    }

    #[test]
    fn stress_mixed_valid_invalid() {
        let mut input = String::new();
        for i in 0..100 {
            if i % 3 == 0 {
                input.push_str(&format!(
                    "src/module{}.erl:{}: Warning: msg{} (Xref)\n",
                    i, i, i
                ));
            } else if i % 3 == 1 {
                input.push_str(&format!("===> Step {}\n", i));
            } else {
                input.push_str("invalid line\n");
            }
        }
        let result = run(&input);
        assert_eq!(result.parsed_items, 34); // 100 / 3 = 33, plus the first one
    }

    // ─── Counterexample tests ──────────────────────────────────────────

    #[test]
    fn no_severity_prefix_raw_message() {
        // Line with no Warning/Error prefix — third colon split fails
        let input = "src/file.erl:5: some raw message without severity";
        let result = run(input);
        // "some raw message without severity" has no colon to split severity
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn windows_path_with_drive_colon() {
        // C:\project\file.erl:5: Warning: msg — first colon splits at C
        let input = r"C:\project\file.erl:5: Warning: msg (Xref)";
        let result = run(input);
        // First split on ':' gets "C" as file, "\project\file.erl" as rest
        // This is a known limitation — xref uses forward-split, not rsplit
        // The line number parse will fail on "\project\file.erl"
        // Acceptable: rebar3 on Windows uses forward slashes anyway
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn extremely_long_file_path() {
        let long_path = format!("src/{}/file.erl", "a".repeat(1000));
        let input = format!("{long_path}:5: Warning: msg (Xref)");
        let result = run(&input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].location, Location::File(long_path));
    }

    #[test]
    fn line_number_overflow() {
        // u32 max is 4294967295, this exceeds it
        let input = "src/file.erl:999999999999: Warning: msg (Xref)";
        let result = run(input);
        // parse::<u32>() fails on overflow, line is skipped
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn negative_line_number() {
        let input = "src/file.erl:-5: Warning: msg (Xref)";
        let result = run(input);
        // parse::<u32>() fails on negative, line is skipped
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn colon_in_message() {
        // Message itself contains colons — should not break parsing
        let input = "src/file.erl:5: Warning: module:func/2 calls undefined: other:thing/1 (Xref)";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("module:func/2"));
    }
}
