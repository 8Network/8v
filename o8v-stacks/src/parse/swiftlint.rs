//! swiftlint JSON parser — Swift linter.
//!
//! swiftlint outputs diagnostics to stdout as a JSON array:
//! ```json
//! [
//!   {
//!     "character": 13,
//!     "file": "/path/file.swift",
//!     "line": 2,
//!     "reason": "msg",
//!     "rule_id": "rule_name",
//!     "severity": "Warning",
//!     "type": "Trailing Whitespace"
//!   }
//! ]
//! ```
//!
//! Severity is capitalized ("Warning", "Error").
//! Character maps to column. All fields except character are present.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

#[derive(Deserialize)]
struct SwiftlintDiagnostic {
    character: Option<u32>,
    file: String,
    line: u32,
    reason: String,
    rule_id: String,
    severity: String,
    #[serde(default)]
    #[allow(dead_code)]
    type_: String,
}

/// Parse swiftlint JSON output into diagnostics.
/// swiftlint writes diagnostics to stdout.
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

    let diags: Vec<SwiftlintDiagnostic> = match serde_json::from_str(stdout) {
        Ok(d) => d,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    for d in diags {
        let location = super::normalize_path(&d.file, project_root);

        // Map severity: "Warning" → Warning, "Error" → Error
        let (severity, raw_severity) = match d.severity.as_str() {
            "Warning" => (Severity::Warning, "Warning"),
            "Error" => (Severity::Error, "Error"),
            _ => (Severity::Error, d.severity.as_str()),
        };

        let column = d.character.unwrap_or(1);

        let diag = Diagnostic {
            location,
            span: Some(Span::new(d.line, column, None, None)),
            rule: Some(DisplayStr::from_untrusted(d.rule_id)),
            severity,
            raw_severity: Some(raw_severity.to_string()),
            message: DisplayStr::from_untrusted(d.reason),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "swiftlint".to_string(),
            stack: "swift".to_string(),
        };

        diagnostics.push(diag);
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
        let result = parse(
            "",
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn parse_single_diagnostic_warning() {
        let stdout = r#"[{"character":13,"file":"/project/test.swift","line":2,"reason":"msg","rule_id":"rule_name","severity":"Warning","type":"Trailing Whitespace"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "msg");
        assert_eq!(diag.rule.as_deref(), Some("rule_name"));
        assert_eq!(diag.tool, "swiftlint");
        assert_eq!(diag.stack, "swift");
    }

    #[test]
    fn parse_single_diagnostic_error() {
        let stdout = r#"[{"character":5,"file":"/project/main.swift","line":10,"reason":"syntax error","rule_id":"syntax","severity":"Error","type":"SyntaxError"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "syntax error");
    }

    #[test]
    fn parse_location() {
        let stdout = r#"[{"character":13,"file":"/project/src/Main.swift","line":5,"reason":"test","rule_id":"test_rule","severity":"Warning","type":"Test"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 13);
    }

    #[test]
    fn parse_no_character_defaults_to_one() {
        let stdout = r#"[{"file":"/project/test.swift","line":2,"reason":"msg","rule_id":"rule","severity":"Error","type":"Test"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.column, 1);
    }

    #[test]
    fn parse_multiple_diagnostics() {
        let stdout = r#"[
{"character":1,"file":"/project/file1.swift","line":1,"reason":"error1","rule_id":"rule1","severity":"Error","type":"Type1"},
{"character":2,"file":"/project/file2.swift","line":2,"reason":"error2","rule_id":"rule2","severity":"Warning","type":"Type2"}
]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn parse_no_diagnostics() {
        let stdout = "[]";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
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
            "swiftlint",
            "swift",
        );

        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn parse_relative_path() {
        let stdout = r#"[{"character":5,"file":"src/main.swift","line":1,"reason":"msg","rule_id":"rule","severity":"Error","type":"Type"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert!(matches!(
            result.diagnostics[0].location,
            o8v_core::diagnostic::Location::File(_)
        ));
    }

    #[test]
    fn parse_absolute_path_outside_project() {
        let stdout = r#"[{"character":1,"file":"/other/file.swift","line":1,"reason":"msg","rule_id":"rule","severity":"Error","type":"Type"}]"#;
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "swiftlint",
            "swift",
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert!(matches!(
            result.diagnostics[0].location,
            o8v_core::diagnostic::Location::Absolute(_)
        ));
    }
}
