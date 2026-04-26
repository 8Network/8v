//! hadolint JSON parser — covers `hadolint --format json`.
//!
//! hadolint emits a bare JSON array of diagnostic objects.
//! Each object includes file, line, column, code, level, and message fields.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse hadolint JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // hadolint outputs a bare JSON array
    let diagnostics_raw: Vec<HadolintDiagnostic> = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    let mut diagnostics = Vec::new();
    let len = diagnostics_raw.len();

    for item in diagnostics_raw {
        let location = super::normalize_path(&item.file, project_root);

        // Map level string to Severity enum
        let severity = match item.level.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" => Severity::Info,
            "style" => Severity::Hint,
            _ => {
                tracing::debug!(level = %item.level, "unknown hadolint level");
                Severity::Warning // default to warning for unknown levels
            }
        };

        let span = Span::new(item.line, item.column, None, None);

        diagnostics.push(Diagnostic {
            location,
            span: Some(span),
            rule: Some(DisplayStr::from_untrusted(item.code.clone())),
            severity,
            raw_severity: Some(item.level.clone()),
            message: DisplayStr::from_untrusted(item.message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: len as u32,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HadolintDiagnostic {
    code: String,
    column: u32,
    file: String,
    level: String,
    line: u32,
    message: String,
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
    fn empty_array() {
        let result = parse("[]", "", root(), "hadolint", "dockerfile");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json", "", root(), "hadolint", "dockerfile");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn single_warning() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"Using latest is prone to errors if the image will ever update"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.rule.as_deref(), Some("DL3007"));
        assert_eq!(
            d.message,
            "Using latest is prone to errors if the image will ever update"
        );
        assert_eq!(d.location, Location::File("Dockerfile".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn error_level() {
        let stdout = r#"[{"code":"DL1001","column":1,"file":"Dockerfile","level":"error","line":5,"message":"Invalid Dockerfile format"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn style_level() {
        let stdout = r#"[{"code":"DL4000","column":1,"file":"Dockerfile","level":"style","line":10,"message":"Consider using COPY instead of ADD"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].severity, Severity::Hint);
    }

    #[test]
    fn info_level() {
        let stdout = r#"[{"code":"DL2000","column":1,"file":"Dockerfile","level":"info","line":3,"message":"Info message"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn multiple_issues() {
        let stdout = r#"[
{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"Using latest"},
{"code":"DL3008","column":1,"file":"Dockerfile","level":"warning","line":2,"message":"Pin versions"},
{"code":"DL3009","column":1,"file":"Dockerfile","level":"warning","line":3,"message":"Delete apt cache"}
]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.parsed_items, 3);
    }

    #[test]
    fn relative_path_normalization() {
        let stdout = r#"[{"code":"DL3007","column":5,"file":"./Dockerfile","level":"warning","line":1,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("./Dockerfile".to_string())
        );
    }

    #[test]
    fn absolute_path_outside_project() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"/other/Dockerfile","level":"warning","line":1,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(
            result.diagnostics[0].location,
            Location::Absolute("/other/Dockerfile".to_string())
        );
    }

    #[test]
    fn column_information() {
        let stdout = r#"[{"code":"DL3007","column":15,"file":"Dockerfile","level":"warning","line":1,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 15);
    }

    #[test]
    fn nested_dockerfile() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"docker/web/Dockerfile","level":"warning","line":1,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("docker/web/Dockerfile".to_string())
        );
    }

    #[test]
    fn stress_many_issues() {
        let mut issues = Vec::new();
        for i in 0..500 {
            issues.push(format!(
                r#"{{"code":"DL{}","column":1,"file":"Dockerfile","level":"warning","line":{},"message":"issue {}"}}"#,
                3000 + i, i + 1, i
            ));
        }
        let input = format!("[{}]", issues.join(","));
        let result = parse(&input, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 500);
        assert_eq!(result.parsed_items, 500);
    }

    #[test]
    fn stress_unicode_message() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"错误 🔥 Unicode message"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Unicode"));
    }

    #[test]
    fn stress_very_large_line_numbers() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":999999999,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 999999999);
    }

    #[test]
    fn stress_empty_array() {
        let result = parse("[]", "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_whitespace_handling() {
        let stdout = "  [  ]  ";
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_long_message() {
        let long_msg = "x".repeat(10000);
        let input = format!(
            r#"[{{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"{}"}}]"#,
            long_msg
        );
        let result = parse(&input, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_hadolint_binary_garbage() {
        let garbage = "[{\"code\":\"DL\x00\x01\x02007\",\"column\":1,\"file\":\"Dockerfile\",\"level\":\"warning\",\"line\":1,\"message\":\"test\"}]";
        let result = parse(garbage, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_truncated_json() {
        let truncated = "[{\"code\":\"DL3007\",\"column\":1,\"file\":\"Dockerfile\",\"level\":\"warning\",\"line\":1,\"message\":\"truncated";
        let result = parse(truncated, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_null_json() {
        let result = parse("null", "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_non_array_json() {
        let result = parse(
            "{\"error\":\"not an array\"}",
            "",
            root(),
            "hadolint",
            "dockerfile",
        );
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_code_field() {
        let stdout =
            r#"[{"column":1,"file":"Dockerfile","level":"warning","line":1,"message":"no code"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_level_field() {
        let stdout =
            r#"[{"code":"DL3007","column":1,"file":"Dockerfile","line":1,"message":"no level"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_line_field() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","message":"no line"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_message_field() {
        let stdout =
            r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_file_field() {
        let stdout =
            r#"[{"code":"DL3007","column":1,"level":"warning","line":1,"message":"no file"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_missing_column_field() {
        let stdout = r#"[{"code":"DL3007","file":"Dockerfile","level":"warning","line":1,"message":"no column"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_hadolint_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let input = format!(
            r#"[{{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"{}"}}]"#,
            long_msg
        );
        let result = parse(&input, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_hadolint_unknown_level() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"unknown","line":1,"message":"unknown level"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn stress_hadolint_all_level_types() {
        let levels = vec![
            ("error", Severity::Error),
            ("warning", Severity::Warning),
            ("info", Severity::Info),
            ("style", Severity::Hint),
        ];
        for (level_str, expected_severity) in levels {
            let stdout = format!(
                r#"[{{"code":"DL3007","column":1,"file":"Dockerfile","level":"{}","line":1,"message":"test"}}]"#,
                level_str
            );
            let result = parse(&stdout, "", root(), "hadolint", "dockerfile");
            assert_eq!(result.status, ParseStatus::Parsed);
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].severity, expected_severity);
        }
    }

    #[test]
    fn stress_hadolint_unicode_in_all_fields() {
        let stdout = r#"[{"code":"DL🔥","column":1,"file":"Dockerfile_文件","level":"warning","line":1,"message":"错误 🎯 message"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert!(d.message.contains("message"));
    }

    #[test]
    fn stress_hadolint_zero_line_and_column() {
        let stdout = r#"[{"code":"DL3007","column":0,"file":"Dockerfile","level":"warning","line":0,"message":"zero values"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn stress_hadolint_very_large_column_number() {
        let stdout = r#"[{"code":"DL3007","column":999999999,"file":"Dockerfile","level":"warning","line":1,"message":"large column"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.column, 999999999);
    }

    #[test]
    fn stress_hadolint_extra_fields_ignored() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"msg","extra":"field","another":123}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
    }

    #[test]
    fn stress_hadolint_empty_code() {
        let stdout = r#"[{"code":"","column":1,"file":"Dockerfile","level":"warning","line":1,"message":"empty code"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some(""));
    }

    #[test]
    fn stress_hadolint_empty_message() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"Dockerfile","level":"warning","line":1,"message":""}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "");
    }

    #[test]
    fn stress_hadolint_empty_file() {
        let stdout = r#"[{"code":"DL3007","column":1,"file":"","level":"warning","line":1,"message":"empty file"}]"#;
        let result = parse(stdout, "", root(), "hadolint", "dockerfile");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::Absolute("".to_string())
        );
    }
}
