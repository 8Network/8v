//! Mypy JSON Lines parser — covers Python static type checking.
//!
//! Mypy with `-O json` emits JSON Lines (one JSON object per line, NOT an array).
//! Each line is a complete diagnostic object.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse mypy JSON Lines output into diagnostics.
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

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Each line is a JSON object. Parse it.
        let Ok(mypy_diag) = serde_json::from_str::<MypyDiagnostic>(line) else {
            tracing::debug!(line, "skipping non-JSON line in mypy output");
            continue;
        };
        parsed_count += 1;

        let diagnostic = convert_diagnostic(&mypy_diag, project_root, tool, stack);
        diagnostics.push(diagnostic);
    }

    let parse_status = if !diagnostics.is_empty() {
        ParseStatus::Parsed
    } else if parsed_count > 0 {
        // We parsed JSON lines but found no diagnostics — tool passed.
        ParseStatus::Parsed
    } else if stdout.trim().is_empty() {
        ParseStatus::Parsed // empty stdout = no output
    } else {
        ParseStatus::Unparsed // couldn't parse anything
    };

    ParseResult {
        diagnostics,
        status: parse_status,
        parsed_items: parsed_count,
    }
}

/// Convert a mypy diagnostic into our Diagnostic.
fn convert_diagnostic(
    mypy_diag: &MypyDiagnostic,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Diagnostic {
    let location = super::normalize_path(&mypy_diag.file, project_root);

    // Mypy column is 0-indexed; we need 1-indexed.
    let column = mypy_diag.column + 1;

    let span = Some(Span::new(
        mypy_diag.line,
        column,
        mypy_diag.end_line,
        mypy_diag.end_column.map(|c| c + 1), // end_column also 0-indexed
    ));

    let severity = match mypy_diag.severity.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "note" => Severity::Info,
        _ => Severity::Info,
    };

    let rule = mypy_diag.code.as_deref().map(DisplayStr::from_untrusted);

    let mut notes = Vec::new();
    if let Some(hint) = &mypy_diag.hint {
        notes.push(hint.clone());
    }

    Diagnostic {
        location,
        span,
        rule,
        severity,
        raw_severity: Some(mypy_diag.severity.clone()),
        message: DisplayStr::from_untrusted(mypy_diag.message.clone()),
        related: Vec::new(),
        notes,
        suggestions: Vec::new(),
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    }
}

// ─── Serde types for Mypy JSON ──────────────────────────────────────────

#[derive(Deserialize)]
struct MypyDiagnostic {
    file: String,
    line: u32,
    column: u32,
    #[serde(default)]
    end_line: Option<u32>,
    #[serde(default)]
    end_column: Option<u32>,
    message: String,
    hint: Option<String>,
    code: Option<String>,
    severity: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/project")
    }

    #[test]
    fn empty_output() {
        let result = parse("", "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_error() {
        let stdout = r#"{"file": "test.py", "line": 2, "column": 11, "end_line": 2, "end_column": 13, "message": "Incompatible return value type (got \"int\", expected \"str\")", "hint": null, "code": "return-value", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("return-value"));
        assert_eq!(
            d.message,
            "Incompatible return value type (got \"int\", expected \"str\")"
        );
        assert_eq!(d.location, Location::File("test.py".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 2);
        // column is 0-indexed in mypy, we add 1
        assert_eq!(span.column, 12);
        assert_eq!(span.end_line, Some(2));
        // end_column is also 0-indexed
        assert_eq!(span.end_column, Some(14));
    }

    #[test]
    fn multiple_errors() {
        let stdout = r#"{"file": "test.py", "line": 2, "column": 11, "message": "Error 1", "hint": null, "code": "error-1", "severity": "error"}
{"file": "test.py", "line": 4, "column": 4, "message": "Error 2", "hint": null, "code": "error-2", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("error-1"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("error-2"));
    }

    #[test]
    fn invalid_json_line_skipped() {
        let stdout = r#"{"file": "test.py", "line": 2, "column": 11, "message": "Valid", "hint": null, "code": "code1", "severity": "error"}
this is not json at all
{"file": "test.py", "line": 3, "column": 5, "message": "Another", "hint": null, "code": "code2", "severity": "warning"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Only the two valid JSON lines should be parsed
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
    }

    #[test]
    fn hint_becomes_note() {
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Some error", "hint": "Have you tried...?", "code": "test-code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.notes[0], "Have you tried...?");
    }

    #[test]
    fn hint_null_no_notes() {
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Some error", "hint": null, "code": "test-code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.notes.len(), 0);
    }

    #[test]
    fn severity_mapping() {
        let error = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Error", "hint": null, "code": "E", "severity": "error"}"#;
        let warning = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Warning", "hint": null, "code": "W", "severity": "warning"}"#;
        let note = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Note", "hint": null, "code": "N", "severity": "note"}"#;

        let e_result = parse(error, "", root(), "mypy", "python");
        assert_eq!(e_result.diagnostics[0].severity, Severity::Error);

        let w_result = parse(warning, "", root(), "mypy", "python");
        assert_eq!(w_result.diagnostics[0].severity, Severity::Warning);

        let n_result = parse(note, "", root(), "mypy", "python");
        assert_eq!(n_result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn optional_end_coords() {
        // Diagnostic with no end_line/end_column
        let stdout = r#"{"file": "test.py", "line": 5, "column": 10, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 11); // 0-indexed 10 becomes 1-indexed 11
        assert_eq!(span.end_line, None);
        assert_eq!(span.end_column, None);
    }

    #[test]
    fn column_indexing() {
        // Mypy is 0-indexed, we should be 1-indexed
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "end_line": 1, "end_column": 5, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        // column 0 → 1, end_column 5 → 6
        assert_eq!(span.column, 1);
        assert_eq!(span.end_column, Some(6));
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_mypy_empty_input() {
        let result = parse("", "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_mypy_whitespace_only() {
        let result = parse("   \n\n\t\t\n   ", "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_mypy_huge_input() {
        // Generate 50,000 JSON line entries
        let single_line = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let mut huge = String::new();
        for _i in 0..50_000 {
            huge.push_str(single_line);
            huge.push('\n');
        }
        let result = parse(&huge, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 50_000);
        assert_eq!(result.diagnostics.len(), 50_000);
    }

    #[test]
    fn stress_mypy_binary_garbage() {
        let garbage = "{\x00\x01\x02}";
        let result = parse(garbage, "", root(), "mypy", "python");
        // Binary garbage is not valid JSON; entire input is unparseable
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_mypy_truncated_json() {
        let truncated = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Incomplete"#;
        let result = parse(truncated, "", root(), "mypy", "python");
        // Truncated JSON is not valid; entire input is unparseable
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_mypy_mixed_valid_invalid_json() {
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Valid 1", "hint": null, "code": "code1", "severity": "error"}
this is garbage
{"file": "test.py", "line": 2, "column": 5, "message": "Valid 2", "hint": null, "code": "code2", "severity": "warning"}
more garbage!!!
{"file": "test.py", "line": 3, "column": 10, "message": "Valid 3", "hint": null, "code": "code3", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.parsed_items, 3);
        // Verify we got the valid ones
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("code1"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("code2"));
        assert_eq!(result.diagnostics[2].rule.as_deref(), Some("code3"));
    }

    #[test]
    fn stress_mypy_unicode_in_paths() {
        let stdout = r#"{"file": "/project/src/文件.py", "line": 1, "column": 0, "message": "错误 🔥", "hint": "提示", "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.location, Location::File("src/文件.py".to_string()));
        assert_eq!(d.message, "错误 🔥");
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.notes[0], "提示");
    }

    #[test]
    fn stress_mypy_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let stdout = format!(
            r#"{{"file": "test.py", "line": 1, "column": 0, "message": "{}", "hint": null, "code": "code", "severity": "error"}}"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, long_msg);
    }

    #[test]
    fn stress_mypy_malformed_json_line() {
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Valid", "hint": null, "code": "code1", "severity": "error"}
{invalid json without quotes
{"file": "test.py", "line": 2, "column": 5, "message": "Another", "hint": null, "code": "code2", "severity": "warning"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
    }

    // ─── Real-world format tests ────────────────────────────────────────────

    #[test]
    fn real_mypy_json_format() {
        // Real mypy -O json output has JSON on separate lines
        let stdout = r#"{"file": "src/main.py", "line": 10, "column": 5, "end_line": 10, "end_column": 8, "message": "Incompatible return value type (got \"int\", expected \"str\")", "hint": "Revealed type is \"int\"", "code": "return-value", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
    }

    // ─── Edge case tests ────────────────────────────────────────────────────

    #[test]
    fn edge_case_missing_optional_fields() {
        // Minimal valid mypy JSON with only required fields
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Error", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.message, "Error");
        assert_eq!(d.rule, None);
        assert_eq!(d.notes.len(), 0);
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, None);
        assert_eq!(span.end_column, None);
    }

    #[test]
    fn edge_case_zero_column_indexing() {
        // Mypy column 0 should become column 1 in our output
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        assert_eq!(
            span.column, 1,
            "0-indexed column 0 must become 1-indexed column 1"
        );
    }

    #[test]
    fn edge_case_large_line_numbers() {
        let stdout = r#"{"file": "test.py", "line": 999999, "column": 50000, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 999999);
        assert_eq!(span.column, 50001);
    }

    #[test]
    fn edge_case_unknown_severity() {
        // Unknown severity should map to Info
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Unknown", "hint": null, "code": "code", "severity": "unknown-severity"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Info);
    }

    #[test]
    fn edge_case_only_blank_lines() {
        let stdout = "\n\n\n\n\n";
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn edge_case_end_column_without_end_line() {
        // end_column is present but end_line is not — Span::new drops both if end_line is None
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "end_column": 10, "message": "Error", "hint": null, "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        // Span::new requires both end_line and end_column to preserve them
        assert_eq!(span.end_line, None);
        assert_eq!(span.end_column, None);
    }

    #[test]
    fn edge_case_all_severity_types() {
        let severities = vec![
            ("error", Severity::Error),
            ("warning", Severity::Warning),
            ("note", Severity::Info),
        ];
        for (sev_str, expected_sev) in severities {
            let stdout = format!(
                r#"{{"file": "test.py", "line": 1, "column": 0, "message": "Test", "hint": null, "code": "code", "severity": "{}"}}"#,
                sev_str
            );
            let result = parse(&stdout, "", root(), "mypy", "python");
            assert_eq!(result.diagnostics[0].severity, expected_sev);
            assert_eq!(
                result.diagnostics[0].raw_severity,
                Some(sev_str.to_string())
            );
        }
    }

    #[test]
    fn edge_case_hint_with_special_chars() {
        // Hint/note with quotes, newlines, backslashes (JSON-escaped)
        let stdout = r#"{"file": "test.py", "line": 1, "column": 0, "message": "Error", "hint": "Hint with \"quotes\" and\nnewlines and \\backslash", "code": "code", "severity": "error"}"#;
        let result = parse(stdout, "", root(), "mypy", "python");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.notes.len(), 1);
        assert!(d.notes[0].contains("quotes"));
        assert!(d.notes[0].contains("newlines"));
        assert!(d.notes[0].contains("backslash"));
    }

    #[test]
    fn edge_case_many_errors_single_file() {
        // 1000 diagnostics from same file
        let mut stdout = String::new();
        for i in 0..1000 {
            stdout.push_str(&format!(
                r#"{{"file": "test.py", "line": {}, "column": 0, "message": "Error {}", "hint": null, "code": "code", "severity": "error"}}"#,
                i + 1,
                i
            ));
            stdout.push('\n');
        }
        let result = parse(&stdout, "", root(), "mypy", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1000);
        assert_eq!(result.parsed_items, 1000);
    }
}
