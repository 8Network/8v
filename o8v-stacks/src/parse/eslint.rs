//! `ESLint` JSON parser — covers `eslint --format=json`.
//!
//! `ESLint` emits a single JSON array on stdout. Each element is a per-file object
//! with a `messages` array of violations. Severity is numeric: 1=warning, 2=error.

use o8v_core::diagnostic::{
    Applicability, Diagnostic, ParseResult, ParseStatus, Severity, Span, Suggestion,
};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse eslint JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // Parse as array of Value first, then each element individually.
    // One bad file object doesn't kill all diagnostics.
    let array: Vec<serde_json::Value> = match serde_json::from_str(stdout) {
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
    let mut skipped = 0u32;
    let mut parsed_files = 0u32;

    for item in array {
        let file: EslintFile = match serde_json::from_value(item) {
            Ok(f) => f,
            Err(e) => {
                skipped += 1;
                tracing::debug!(error = %e, "skipping malformed eslint file entry");
                continue;
            }
        };
        parsed_files += 1;
        let location = super::normalize_path(&file.file_path, project_root);

        for msg in file.messages {
            let span = Some(Span::new(
                msg.line,
                msg.column,
                msg.end_line,
                msg.end_column,
            ));

            // ESLint severity: 0=off, 1=warning, 2=error.
            // severity=0 means the rule is disabled — skip it entirely.
            let severity = match msg.severity {
                0 => continue,
                1 => Severity::Warning,
                _ => Severity::Error,
            };

            // ESLint suggestions carry byte-offset fix ranges, not line/column.
            // Converting requires source text we don't have. Keep the message,
            // omit the edit — an unusable span is worse than no span.
            let suggestions = msg
                .suggestions
                .into_iter()
                .map(|s| Suggestion {
                    message: s.desc,
                    applicability: Applicability::Unspecified,
                    edits: vec![],
                })
                .collect();

            diagnostics.push(Diagnostic {
                location: location.clone(),
                span,
                rule: msg.rule_id.map(DisplayStr::from_untrusted),
                severity,
                raw_severity: Some(match msg.severity {
                    1 => "warning".to_string(),
                    _ => "error".to_string(),
                }),
                message: DisplayStr::from_untrusted(msg.message),
                related: vec![],
                notes: vec![],
                suggestions,
                snippet: None,
                tool: tool.to_string(),
                stack: stack.to_string(),
            });
        }
    }

    if skipped > 0 {
        tracing::warn!(skipped, "eslint: some file entries could not be parsed");
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: parsed_files,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EslintFile {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<EslintMessage>,
}

#[derive(Deserialize)]
struct EslintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: u32,
    column: u32,
    #[serde(rename = "endLine")]
    end_line: Option<u32>,
    #[serde(rename = "endColumn")]
    end_column: Option<u32>,
    #[serde(default)]
    suggestions: Vec<EslintSuggestion>,
}

#[derive(Deserialize)]
struct EslintSuggestion {
    desc: String,
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
        let result = parse("[]", "", root(), "eslint", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json", "", root(), "eslint", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn single_error() {
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"'x' is defined but never used.","line":1,"column":5,"endLine":1,"endColumn":6}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("no-unused-vars"));
        assert_eq!(d.message, "'x' is defined but never used.");
        assert_eq!(d.location, Location::File("src/app.js".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 5);
        assert_eq!(span.end_line, Some(1));
        assert_eq!(span.end_column, Some(6));
    }

    #[test]
    fn warning_message() {
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"no-console","severity":1,"message":"Unexpected console statement.","line":3,"column":1,"endLine":3,"endColumn":12}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn severity_zero_skipped() {
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"some-rule","severity":0,"message":"off rule","line":1,"column":1}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn suggestion_message() {
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"no-unsafe-negation","severity":2,"message":"Unexpected negation.","line":1,"column":1,"endLine":1,"endColumn":10,"suggestions":[{"desc":"Wrap the negation in parentheses."}]}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.suggestions.len(), 1);
        assert_eq!(
            d.suggestions[0].message,
            "Wrap the negation in parentheses."
        );
        assert!(matches!(
            d.suggestions[0].applicability,
            Applicability::Unspecified
        ));
        assert!(d.suggestions[0].edits.is_empty());
    }

    #[test]
    fn malformed_file_entry() {
        // First entry is valid, second is malformed (missing filePath)
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"unused","line":1,"column":1}]},{"bad":"data"}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].rule.as_deref(),
            Some("no-unused-vars")
        );
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_eslint_empty_input() {
        let result = parse("", "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_eslint_huge_input() {
        let file_entry = r#"{"filePath":"/project/src/file.js","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"unused","line":1,"column":1,"endLine":1,"endColumn":5}]}"#;
        let huge = format!(
            "[{}]",
            std::iter::repeat_n(file_entry, 100_000)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = parse(&huge, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_eslint_binary_garbage() {
        let garbage = "[\x00\x01\x02\x00]";
        let result = parse(garbage, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_eslint_whitespace_only() {
        let result = parse("   \n\n\t\t\n   ", "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_eslint_truncated_json() {
        let truncated = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"rule"#;
        let result = parse(truncated, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_eslint_unicode_in_paths() {
        let stdout = r#"[{"filePath":"/project/src/文件.js","messages":[{"ruleId":"error-rule","severity":2,"message":"错误 🔥 RTL: עברית","line":1,"column":1,"endLine":1,"endColumn":5}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_eslint_extremely_long_line() {
        let long_msg = "x".repeat(1_000_000);
        let stdout = format!(
            r#"[{{"filePath":"/project/src/app.js","messages":[{{"ruleId":"rule","severity":2,"message":"{}","line":1,"column":1}}]}}]"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn stress_eslint_deeply_nested_messages() {
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"rule1","severity":2,"message":"msg1","line":1,"column":1},{"ruleId":"rule2","severity":2,"message":"msg2","line":2,"column":2},{"ruleId":"rule3","severity":2,"message":"msg3","line":3,"column":3},{"ruleId":"rule4","severity":2,"message":"msg4","line":4,"column":4},{"ruleId":"rule5","severity":2,"message":"msg5","line":5,"column":5},{"ruleId":"rule6","severity":2,"message":"msg6","line":6,"column":6},{"ruleId":"rule7","severity":2,"message":"msg7","line":7,"column":7},{"ruleId":"rule8","severity":2,"message":"msg8","line":8,"column":8},{"ruleId":"rule9","severity":2,"message":"msg9","line":9,"column":9},{"ruleId":"rule10","severity":2,"message":"msg10","line":10,"column":10}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10);
    }

    // ─── Security Tests ────────────────────────────────────────────────────

    #[test]
    fn security_eslint_dos_deeply_nested_json() {
        // Deeply nested JSON could cause stack exhaustion in some parsers.
        // serde_json has recursion limits, but we verify robustness.
        // Create 1000 levels of nesting: [[[[[...]]]]]
        let mut nested = String::from("[");
        for _ in 0..1000 {
            nested.push('[');
        }
        nested.push_str(r#"{"filePath":"/project/src/app.js","messages":[]}"#);
        for _ in 0..1000 {
            nested.push(']');
        }
        nested.push(']');

        // serde_json should handle or reject this gracefully
        let result = parse(&nested, "", root(), "eslint", "javascript");
        // Either it parses (unlikely) or returns unparsed status (expected)
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
    }

    #[test]
    fn security_eslint_dos_huge_single_message() {
        // Single message field with 10MB of text
        let huge_msg = "x".repeat(10_000_000);
        let stdout = format!(
            r#"[{{"filePath":"/project/src/app.js","messages":[{{"ruleId":"rule","severity":2,"message":"{}","line":1,"column":1}}]}}]"#,
            huge_msg
        );
        let result = parse(&stdout, "", root(), "eslint", "javascript");
        // Should parse successfully (memory is allowed for legitimate large outputs)
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // Message should be preserved (up to parser limits)
        assert!(!result.diagnostics[0].message.is_empty());
    }

    #[test]
    fn security_eslint_dos_many_files() {
        // 100,000 file objects each with 1 message
        let file_entry = r#"{"filePath":"/project/src/file.js","messages":[{"ruleId":"rule","severity":2,"message":"error","line":1,"column":1}]}"#;
        let huge = format!(
            "[{}]",
            std::iter::repeat_n(file_entry, 100_000)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = parse(&huge, "", root(), "eslint", "javascript");
        // Should parse and count correctly
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 100_000);
        assert_eq!(result.parsed_items, 100_000);
    }

    #[test]
    fn security_eslint_injection_quote_in_message() {
        // Quote character in message should not break JSON parsing
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"rule","severity":2,"message":"error: \"quoted\" text","line":1,"column":1}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "error: \"quoted\" text");
    }

    #[test]
    fn security_eslint_injection_control_chars_in_message() {
        // Control characters in message (escaped in JSON) should parse correctly
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"rule","severity":2,"message":"error\nwith\nnewlines","line":1,"column":1}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // DisplayStr::from_untrusted strips newlines at parse time — no downstream injection risk.
        assert!(!result.diagnostics[0].message.contains('\n'));
        assert!(result.diagnostics[0].message.contains("error"));
    }

    #[test]
    fn security_eslint_injection_backslash_in_message() {
        // Backslashes in message should parse correctly
        let stdout = r#"[{"filePath":"/project/src/app.js","messages":[{"ruleId":"rule","severity":2,"message":"path: C:\\Users\\test","line":1,"column":1}]}]"#;
        let result = parse(stdout, "", root(), "eslint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("C:"));
    }
}
