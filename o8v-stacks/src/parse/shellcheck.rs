//! ShellCheck JSON parser — covers `shellcheck -f json`.
//!
//! ShellCheck emits a JSON array on stdout. Each element is a diagnostic object
//! with file, line, column, level, code, and message fields.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse shellcheck JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // ShellCheck outputs a bare JSON array, not an object wrapper.
    let diagnostics_raw: Vec<ShellcheckDiagnostic> = match serde_json::from_str(stdout) {
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

        // Map level string to Severity enum.
        let severity = match item.level.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" => Severity::Info,
            "style" => Severity::Hint,
            _ => {
                tracing::debug!(level = %item.level, "unknown shellcheck level");
                Severity::Warning // default to warning for unknown levels
            }
        };

        // Create rule identifier from code (e.g., "SC2154")
        let rule = format!("SC{}", item.code);

        // Build span if we have location information.
        let span = Some(Span::new(
            item.line,
            item.column,
            item.end_line,
            item.end_column,
        ));

        diagnostics.push(Diagnostic {
            location,
            span,
            rule: Some(DisplayStr::from_untrusted(rule)),
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
struct ShellcheckDiagnostic {
    file: String,
    line: u32,
    #[serde(rename = "endLine")]
    end_line: Option<u32>,
    column: u32,
    #[serde(rename = "endColumn")]
    end_column: Option<u32>,
    level: String,
    code: u32,
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
        let result = parse("[]", "", root(), "shellcheck", "shell");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json", "", root(), "shellcheck", "shell");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn single_warning() {
        let stdout = r#"[{"file":"/project/test.sh","line":2,"endLine":2,"column":6,"endColumn":10,"level":"warning","code":2154,"message":"foo is referenced but not assigned.","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.rule.as_deref(), Some("SC2154"));
        assert_eq!(d.message, "foo is referenced but not assigned.");
        assert_eq!(d.location, Location::File("test.sh".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 6);
        assert_eq!(span.end_line, Some(2));
        assert_eq!(span.end_column, Some(10));
    }

    #[test]
    fn error_level() {
        let stdout = r#"[{"file":"/project/script.sh","line":1,"column":1,"level":"error","code":1091,"message":"Unexpected EOF","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SC1091"));
    }

    #[test]
    fn info_level() {
        let stdout = r#"[{"file":"/project/script.sh","line":5,"column":3,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SC2086"));
    }

    #[test]
    fn style_level_maps_to_hint() {
        let stdout = r#"[{"file":"/project/script.sh","line":3,"column":1,"level":"style","code":2166,"message":"Prefer 'local' for loop variables in functions.","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Hint);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SC2166"));
    }

    #[test]
    fn multiple_diagnostics() {
        let stdout = r#"[{"file":"/project/a.sh","line":1,"column":1,"level":"warning","code":2154,"message":"undefined","fix":null},{"file":"/project/b.sh","line":2,"column":5,"level":"error","code":1091,"message":"unexpected EOF","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("a.sh".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("b.sh".to_string())
        );
    }

    #[test]
    fn raw_severity_preserved() {
        let stdout = r#"[{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(
            result.diagnostics[0].raw_severity.as_deref(),
            Some("warning")
        );
    }

    #[test]
    fn absolute_path_under_root() {
        let stdout = r#"[{"file":"/project/src/script.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/script.sh".to_string())
        );
    }

    #[test]
    fn relative_path() {
        let stdout = r#"[{"file":"script.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("script.sh".to_string())
        );
    }

    #[test]
    fn missing_end_line_column() {
        let stdout = r#"[{"file":"/project/test.sh","line":5,"column":3,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 3);
        assert_eq!(span.end_line, None);
        assert_eq!(span.end_column, None);
    }

    #[test]
    fn unknown_level_defaults_to_warning() {
        let stdout = r#"[{"file":"/project/test.sh","line":1,"column":1,"level":"unknown","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    // ─── Stress tests ──────────────────────────────────────────────────────

    #[test]
    fn stress_truncated_json() {
        let truncated = r#"[{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg"#;
        let result = parse(truncated, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_unicode_in_message() {
        let stdout = r#"[{"file":"/project/测试.sh","line":1,"column":1,"level":"warning","code":2154,"message":"错误 🔥","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("错误"));
    }

    #[test]
    fn stress_long_message() {
        let long_msg = "x".repeat(10000);
        let stdout = format!(
            r#"[{{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"{}","fix":null}}]"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_many_diagnostics() {
        let item = r#"{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg","fix":null}"#;
        let huge = format!(
            "[{}]",
            std::iter::repeat_n(item, 10000)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = parse(&huge, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10000);
        assert_eq!(result.parsed_items, 10000);
    }

    #[test]
    fn stress_whitespace_only() {
        let result = parse("   \n\t\n  ", "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_binary_garbage() {
        let garbage = String::from_utf8_lossy(&[0x00, 0x01, 0x02]);
        let result = parse(&garbage, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_non_array_json() {
        let stdout = r#"{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg"}"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_empty_string() {
        let result = parse("", "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_missing_required_fields() {
        let stdout = r#"[{"file":"/project/test.sh","level":"warning","code":2154}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_null_values() {
        let stdout =
            r#"[{"file":null,"line":1,"column":1,"level":"warning","code":2154,"message":"msg"}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_extremely_long_message() {
        let long_msg = "x".repeat(100000);
        let stdout = format!(
            r#"[{{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"{}","fix":null}}]"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message.len(), 100000);
    }

    #[test]
    fn stress_unicode_in_paths() {
        let stdout = r#"[{"file":"/project/тест/🔥/script.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        match &result.diagnostics[0].location {
            Location::Absolute(p) | Location::File(p) => assert!(p.contains("тест")),
            _ => panic!("unexpected Location variant"),
        }
    }

    #[test]
    fn stress_deeply_nested_invalid_json() {
        let stdout = r#"[[[[[{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":2154,"message":"msg"}]]]]]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_mixed_valid_invalid_items() {
        let stdout = r#"[{"file":"/project/a.sh","line":1,"column":1,"level":"warning","code":2154,"message":"ok"},{"invalid":"object"}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_code_formatting() {
        let stdout = r#"[{"file":"/project/test.sh","line":1,"column":1,"level":"warning","code":1,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SC1"));
    }

    #[test]
    fn stress_large_line_numbers() {
        let stdout = r#"[{"file":"/project/test.sh","line":999999,"column":999999,"level":"warning","code":2154,"message":"msg","fix":null}]"#;
        let result = parse(stdout, "", root(), "shellcheck", "shell");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 999999);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().column, 999999);
    }

    #[test]
    fn stress_all_severity_levels() {
        let severities = vec![
            ("error", Severity::Error),
            ("warning", Severity::Warning),
            ("info", Severity::Info),
            ("style", Severity::Hint),
        ];

        for (level_str, expected_severity) in severities {
            let stdout = format!(
                r#"[{{"file":"/project/test.sh","line":1,"column":1,"level":"{}","code":2154,"message":"msg","fix":null}}]"#,
                level_str
            );
            let result = parse(&stdout, "", root(), "shellcheck", "shell");
            assert_eq!(result.status, ParseStatus::Parsed);
            assert_eq!(
                result.diagnostics[0].severity, expected_severity,
                "Level '{}' should map to {:?}",
                level_str, expected_severity
            );
        }
    }
}
