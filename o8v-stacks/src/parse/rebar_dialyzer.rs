//! rebar3 dialyzer output parser — covers `rebar3 dialyzer`.
//!
//! rebar3 dialyzer outputs findings in a line-by-line format:
//! - Progress lines start with "===> " and are skipped
//! - Filenames appear as standalone lines ending in ".erl"
//! - Findings appear as "Line N Column M: message" or "Line N: message"

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Strip ANSI escape codes from a string.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse rebar3 dialyzer output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let clean = strip_ansi(stdout);
    let mut diagnostics = Vec::new();
    let mut current_file: Option<String> = None;
    let mut parsed_any = false;

    for line in clean.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Skip rebar3 progress lines (===> ...)
        if trimmed.starts_with("===> ") {
            parsed_any = true;
            continue;
        }

        // Check if this line is a filename (.erl source or .hrl header)
        if trimmed.ends_with(".erl") || trimmed.ends_with(".hrl") {
            current_file = Some(trimmed.to_string());
            continue;
        }

        // Parse finding lines: "Line N Column M: message" or "Line N: message"
        if trimmed.starts_with("Line ") {
            parsed_any = true;

            if let Some(file) = &current_file {
                let location = super::normalize_path(file, project_root);

                // Parse the finding line
                if let Some((span, message)) = parse_finding(trimmed) {
                    diagnostics.push(Diagnostic {
                        location,
                        span: Some(span),
                        rule: Some(DisplayStr::from_untrusted("dialyzer".to_string())),
                        severity: Severity::Warning,
                        raw_severity: None,
                        message: DisplayStr::from_untrusted(message),
                        related: vec![],
                        notes: vec![],
                        suggestions: vec![],
                        snippet: None,
                        tool: tool.to_string(),
                        stack: stack.to_string(),
                    });
                }
            }
        }
    }

    let status = if !diagnostics.is_empty() || parsed_any || stdout.trim().is_empty() {
        ParseStatus::Parsed
    } else {
        ParseStatus::Unparsed
    };

    let count = diagnostics.len() as u32;
    ParseResult {
        diagnostics,
        status,
        parsed_items: count,
    }
}

/// Parse a finding line: "Line N Column M: message" or "Line N: message"
/// Returns (Span, message) if successful.
fn parse_finding(line: &str) -> Option<(Span, String)> {
    // Remove "Line " prefix
    let after_line = line.strip_prefix("Line ")?;

    // Split by the first colon to separate location from message
    let (location_part, message) = after_line.split_once(':')?;
    let message = message.trim().to_string();

    // location_part is either "N" or "N Column M"
    if let Some(column_idx) = location_part.find(" Column ") {
        // Format: "N Column M"
        let line_str = &location_part[..column_idx];
        let col_str = &location_part[column_idx + 8..]; // Skip " Column "

        let line: u32 = match line_str.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };
        let col: u32 = match col_str.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };

        Some((Span::new(line, col, None, None), message))
    } else {
        // Format: "N" (no column)
        let line: u32 = match location_part.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };
        Some((Span::new(line, 1, None, None), message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::Location;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "dialyzer", "erlang")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_finding() {
        let input = "src/flight.erl\nLine 7 Column 14: Unknown function nonexistent_module:save/2";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("dialyzer"));
        assert_eq!(d.message, "Unknown function nonexistent_module:save/2");
        assert_eq!(d.location, Location::File("src/flight.erl".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 7);
        assert_eq!(span.column, 14);
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn multiple_findings_same_file() {
        let input = "src/flight.erl\nLine 7 Column 14: Unknown function nonexistent_module:save/2\nLine 10 Column 5: Type mismatch";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 7);
        assert_eq!(result.diagnostics[1].span.as_ref().unwrap().line, 10);
    }

    #[test]
    fn multiple_files() {
        let input = "src/flight.erl\nLine 7 Column 14: Unknown function nonexistent_module:save/2\nsrc/other.erl\nLine 3 Column 2: Type error";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/flight.erl".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/other.erl".to_string())
        );
    }

    #[test]
    fn progress_lines_skipped() {
        let input = "===> Dialyzer starting...\nsrc/flight.erl\nLine 7 Column 14: Unknown function nonexistent_module:save/2\n===> Dialyzer done";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn no_column() {
        let input = "src/flight.erl\nLine 7: Unknown function nonexistent_module:save/2";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 7);
        assert_eq!(span.column, 1);
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_empty_input() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_huge_input() {
        let mut huge = String::new();
        huge.push_str("===> Dialyzer starting...\n");
        for i in 0..1000 {
            huge.push_str(&format!("src/file{}.erl\n", i));
            for j in 0..10 {
                huge.push_str(&format!("Line {} Column 1: Error message {}\n", j + 1, j));
            }
        }
        let result = run(&huge);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10000);
    }

    #[test]
    fn stress_whitespace_only() {
        let result = run("   \n\n\t\t\n   ");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_unicode_in_paths_and_messages() {
        let input = "src/文件.erl\nLine 7 Column 14: 错误 🔥 עברית";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // Should handle Unicode in paths and messages
    }

    #[test]
    fn stress_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let input = format!("src/file.erl\nLine 7 Column 14: {}", long_msg);
        let result = run(&input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_malformed_finding_lines() {
        let malformed = vec![
            "src/file.erl\nLine : Unknown function", // missing line number
            "src/file.erl\nLine abc: Unknown function", // non-numeric line
            "src/file.erl\nLine 7 Column : Unknown", // missing column number
            "src/file.erl\nLine 7 Column abc: Unknown", // non-numeric column
        ];
        for input in malformed {
            let result = run(input);
            // Malformed lines should be skipped gracefully
            assert!(matches!(
                result.status,
                ParseStatus::Parsed | ParseStatus::Unparsed
            ));
        }
    }

    #[test]
    fn stress_file_without_findings() {
        let input = "src/flight.erl\nsrc/other.erl\nLine 3 Column 2: Error";
        let result = run(input);
        // first file has no findings, second file has one
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/other.erl".to_string())
        );
    }

    #[test]
    fn stress_no_file_before_finding() {
        let input = "Line 7 Column 14: Unknown function nonexistent_module:save/2";
        let result = run(input);
        // Finding without a preceding file should be skipped
        assert_eq!(result.diagnostics.len(), 0);
    }

    // ─── Counterexample tests ──────────────────────────────────────────

    #[test]
    fn hrl_header_file() {
        // .hrl header files are valid Erlang — dialyzer can report on them
        let input = "include/types.hrl\nLine 5 Column 1: Type spec mismatch";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("include/types.hrl".to_string())
        );
    }

    #[test]
    fn zero_position() {
        let input = "src/file.erl\nLine 0 Column 0: something";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        // Span::new clamps to min 1
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn filename_with_spaces() {
        let input = "src/my module.erl\nLine 7 Column 14: error msg";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/my module.erl".to_string())
        );
    }

    #[test]
    fn ansi_codes_in_output() {
        // Real rebar3 dialyzer output has ANSI codes
        let input = "\x1b[4msrc/\x1b[0;36m\x1b[4mflight\x1b[0m\x1b[4m.erl\x1b[0m\x1b[0m\n\
            Line \x1b[0;36m7\x1b[0m Column \x1b[0;36m14\x1b[0m: \x1b[1mUnknown function \x1b[0;31mnonexistent_module:save/2\x1b[0m\x1b[0m";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/flight.erl".to_string())
        );
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 7);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().column, 14);
    }

    #[test]
    fn line_without_column_keyword() {
        // "Line 7: message" — no Column keyword
        let input = "src/file.erl\nLine 7: Type mismatch in return";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 7);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().column, 1);
    }

    #[test]
    fn colon_in_message_body() {
        let input =
            "src/file.erl\nLine 7 Column 1: The call module:func(X :: atom()) breaks the contract";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("module:func"));
    }
}
