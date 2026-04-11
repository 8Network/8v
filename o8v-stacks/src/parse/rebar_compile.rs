//! rebar3 compile output parser — covers `rebar3 compile`.
//!
//! rebar3 produces human-readable output with box-drawing characters:
//! - `┌─ <file>:` for file location
//! - `N │` for line numbers
//! - `╰── [Warning:|Error:] message` for the diagnostic
//!
//! ANSI escape codes are stripped before parsing.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use std::path::Path;

/// Strip ANSI escape codes from a string.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm' (end of ANSI sequence)
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

/// Parse rebar3 compile output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let clean = strip_ansi(stdout);

    if clean.trim().is_empty() {
        return ParseResult {
            diagnostics: vec![],
            status: ParseStatus::Parsed,
            parsed_items: 0,
        };
    }

    let mut diagnostics = Vec::new();
    let mut current_file: Option<String> = None;
    let mut current_line: Option<u32> = None;

    for line in clean.lines() {
        let trimmed = line.trim();

        // Skip progress lines
        if trimmed.starts_with("===> ") || trimmed.is_empty() {
            continue;
        }

        // File location: "┌─ src/file.erl:"
        if let Some(rest) = trimmed.strip_prefix("┌─") {
            let rest = rest.trim();
            if let Some(file) = rest.strip_suffix(':') {
                current_file = Some(file.to_string());
                current_line = None;
            }
            continue;
        }

        // Line number: "N │ ..."
        if trimmed.contains('│') {
            if let Some(num_part) = trimmed.split('│').next() {
                if let Ok(n) = num_part.trim().parse::<u32>() {
                    current_line = Some(n);
                }
            }
        }

        // Diagnostic message: "╰── [Warning: |] message"
        // The ╰── may appear after │ on the same line: "│      ╰── Warning: ..."
        let arrow_content = trimmed
            .strip_prefix("╰──")
            .or_else(|| trimmed.find("╰──").map(|pos| &trimmed[pos + "╰──".len()..]));
        if let Some(rest) = arrow_content {
            let msg = rest.trim();
            let (severity, message) = if let Some(after) = msg.strip_prefix("Warning:") {
                (Severity::Warning, after.trim())
            } else if let Some(after) = msg.strip_prefix("Error:") {
                (Severity::Error, after.trim())
            } else {
                // No explicit severity prefix — treat as error (e.g. "syntax error before: ok")
                (Severity::Error, msg)
            };

            if let Some(file) = &current_file {
                let location = super::normalize_path(file, project_root);
                let span = current_line.map(|n| Span::new(n, 1, None, None));

                diagnostics.push(Diagnostic {
                    location,
                    span,
                    rule: None,
                    severity,
                    raw_severity: None,
                    message: DisplayStr::from_untrusted(message.to_string()),
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

    let count = diagnostics.len() as u32;
    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "rebar3-compile", "erlang")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_warning() {
        let input = "\
   ┌─ src/flight.erl:
   │
 6 │      UnusedVar = \"not used\",
   │      ╰── Warning: variable 'UnusedVar' is unused
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.location, Location::File("src/flight.erl".to_string()));
        assert_eq!(d.span.as_ref().map(|s| s.line), Some(6));
        assert_eq!(d.severity, Severity::Warning);
        assert!(d.message.as_str().contains("UnusedVar"));
    }

    #[test]
    fn syntax_error() {
        let input = "\
   ┌─ src/broken.erl:
   │
 5 │      ok.
   │      ╰── syntax error before: ok
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.as_str().contains("syntax error"));
    }

    #[test]
    fn multiple_diagnostics() {
        let input = "\
   ┌─ src/flight.erl:
   │
 6 │      UnusedVar = \"not used\",
   │      ╰── Warning: variable 'UnusedVar' is unused

   ┌─ src/flight.erl:
   │
 7 │      Result = nonexistent_module:save(FlightId, Passenger),
   │      ╰── Warning: variable 'Result' is unused
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn ansi_codes_stripped() {
        let input = "\x1b[0;32m===> Compiling airline_test\x1b[0m\n\
   ┌─ src/flight.erl:\n\
   │\n\
 6 │      \x1b[1;31mUnusedVar\x1b[0m = \"not used\",\n\
   │      \x1b[1;31m╰──\x1b[0m\x1b[0m Warning: variable 'UnusedVar' is unused\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn progress_lines_skipped() {
        let input = "\
===> Verifying dependencies...
===> Analyzing applications...
===> Compiling airline_test
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    // ─── Counterexample Tests ───────────────────────────────────────

    #[test]
    fn combined_ansi_and_box_drawing() {
        let input = "\x1b[0;32m===> Compiling airline_test\x1b[0m\n\
   \x1b[1;37m┌─\x1b[0m src/flight.erl:\n\
   │\n\
 6 │      UnusedVar = \"not used\",\n\
   │      \x1b[1;31m╰──\x1b[0m\x1b[1;33m Warning: variable 'UnusedVar' is unused\x1b[0m\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert!(result.diagnostics[0].message.as_str().contains("UnusedVar"));
    }

    #[test]
    fn multiple_diagnostics_same_file_block() {
        let input = "\
   ┌─ src/flight.erl:
   │
 6 │      UnusedVar = \"not used\",
   │      ╰── Warning: variable 'UnusedVar' is unused
 7 │      Result = nonexistent(),
   │      ╰── Error: undefined function nonexistent/0
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[1].severity, Severity::Error);
    }

    #[test]
    fn orphan_message_without_file() {
        let input = "\
   │      ╰── Warning: orphan message with no file context
";
        let result = run(input);
        // Should skip orphan messages without a current_file
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn file_path_with_spaces() {
        let input = "\
   ┌─ src/my project/flight.erl:
   │
 6 │      UnusedVar = \"not used\",
   │      ╰── Warning: variable 'UnusedVar' is unused
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/my project/flight.erl".to_string())
        );
    }

    #[test]
    fn line_number_zero() {
        let input = "\
   ┌─ src/flight.erl:
   │
 0 │      UnusedVar = \"not used\",
   │      ╰── Warning: line zero diagnostic
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        // Span::new clamps line to max(1) — line 0 is not valid
        assert_eq!(span.line, 1);
    }

    #[test]
    fn deeply_nested_box_drawing() {
        let input = "\
   ┌─ src/flight.erl:
   │
 6 │      match result {
   │           ╰── Warning: incomplete pattern match
 7 │        _ => ok
   │        ╰── Error: unreachable clause
";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 6);
        assert_eq!(result.diagnostics[1].span.as_ref().unwrap().line, 7);
    }
}
