//! ktlint JSON parser — Kotlin linter.
//!
//! ktlint outputs diagnostics to stdout as a JSON array:
//! ```json
//! [
//!   {
//!     "file": "/path/to/file.kt",
//!     "errors": [
//!       {
//!         "line": 2,
//!         "column": 1,
//!         "message": "msg",
//!         "rule": "standard:rule-name"
//!       }
//!     ]
//!   }
//! ]
//! ```
//!
//! All errors are treated as Error severity (no warning distinction in ktlint).

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

#[derive(Deserialize)]
struct KtlintFile {
    file: String,
    errors: Vec<KtlintError>,
}

#[derive(Deserialize)]
struct KtlintError {
    line: u32,
    column: u32,
    message: String,
    rule: String,
}

/// Parse ktlint JSON output into diagnostics.
/// ktlint writes diagnostics to stdout.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    _tool: &str,
    _stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();

    // Empty output is clean
    if stdout.is_empty() {
        return ParseResult {
            diagnostics,
            status: ParseStatus::Parsed,
            parsed_items: 0,
        };
    }

    let files: Vec<KtlintFile> = match serde_json::from_str(stdout) {
        Ok(f) => f,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    for file in files {
        for error in file.errors {
            let location = super::normalize_path(&file.file, project_root);

            let diag = Diagnostic {
                location,
                span: Some(Span::new(error.line, error.column, None, None)),
                rule: Some(DisplayStr::from_untrusted(error.rule.clone())),
                severity: Severity::Error,
                raw_severity: Some("Error".to_string()),
                message: DisplayStr::from_untrusted(error.message),
                related: vec![],
                notes: vec![],
                suggestions: vec![],
                snippet: None,
                tool: "ktlint".to_string(),
                stack: "kotlin".to_string(),
            };

            diagnostics.push(diag);
        }
    }

    let parsed_items = diagnostics.len() as u32;
    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_output() {
        let result = parse("", "", std::path::Path::new("/project"), "ktlint", "kotlin");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn parse_single_file_single_error() {
        let stdout = r#"[{"file":"/project/test.kt","errors":[{"line":2,"column":1,"message":"msg","rule":"standard:rule-name"}]}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "msg");
        assert_eq!(diag.rule.as_deref(), Some("standard:rule-name"));
        assert_eq!(diag.tool, "ktlint");
        assert_eq!(diag.stack, "kotlin");
    }

    #[test]
    fn parse_location() {
        let stdout = r#"[{"file":"/project/src/Main.kt","errors":[{"line":5,"column":10,"message":"test","rule":"test:rule"}]}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 10);
    }

    #[test]
    fn parse_multiple_files() {
        let stdout = r#"[
{"file":"/project/file1.kt","errors":[{"line":1,"column":1,"message":"error1","rule":"rule1"}]},
{"file":"/project/file2.kt","errors":[{"line":2,"column":2,"message":"error2","rule":"rule2"}]}
]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("rule1"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("rule2"));
    }

    #[test]
    fn parse_multiple_errors_same_file() {
        let stdout = r#"[{"file":"/project/test.kt","errors":[{"line":1,"column":1,"message":"error1","rule":"rule1"},{"line":2,"column":2,"message":"error2","rule":"rule2"}]}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].message, "error1");
        assert_eq!(result.diagnostics[1].message, "error2");
    }

    #[test]
    fn parse_no_errors() {
        let stdout = "[]";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn parse_invalid_json() {
        let stdout = "invalid json";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn parse_relative_path() {
        let stdout = r#"[{"file":"src/test.kt","errors":[{"line":1,"column":1,"message":"msg","rule":"rule"}]}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "ktlint",
            "kotlin",
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert!(matches!(
            result.diagnostics[0].location,
            o8v_core::diagnostic::Location::File(_)
        ));
    }
}
