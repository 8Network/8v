//! `Oxlint` JSON parser — covers `oxlint --format=json`.
//!
//! Oxlint emits a single JSON object on stdout with a `diagnostics` array.
//! Each diagnostic contains message, code, severity, filename, and optional labels.
//! Labels have nested `span: {offset, length, line, column}`.
//! Severity values are strings: "warning", "error", etc.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse oxlint JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // Parse as single JSON object with diagnostics array.
    // We only care about diagnostics; ignore metadata like number_of_files, threads_count, etc.
    let obj: serde_json::Value = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    let diagnostics_array = match obj.get("diagnostics") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    let mut diagnostics = Vec::new();
    let mut parsed_items = 0u32;

    for diag_val in diagnostics_array {
        let diag: OxlintDiagnostic = match serde_json::from_value(diag_val.clone()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let location = super::normalize_path(&diag.filename, project_root);

        // Extract span from first label if available, otherwise None.
        let span = diag
            .labels
            .first()
            .and_then(|label| label.span.as_ref())
            .map(|span| Span::new(span.line, span.column, None, None));

        // Map severity string to Severity enum.
        let severity = match diag.severity.as_str() {
            "warning" => Severity::Warning,
            "error" => Severity::Error,
            other => {
                tracing::debug!(
                    severity = other,
                    "oxlint: unknown severity, defaulting to Error"
                );
                Severity::Error
            }
        };

        // Extract rule name: "eslint(no-debugger)" → "no-debugger", or keep as-is.
        let rule = if diag.code.contains('(') && diag.code.contains(')') {
            // Format: "eslint(rule-name)" or similar — extract the rule name.
            diag.code
                .split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
                .map(DisplayStr::from_untrusted)
                .or_else(|| Some(DisplayStr::from_untrusted(&diag.code)))
        } else {
            Some(DisplayStr::from_untrusted(&diag.code))
        };

        // Include help text in notes if present.
        let notes = if let Some(help) = &diag.help {
            vec![help.clone()]
        } else {
            vec![]
        };

        diagnostics.push(Diagnostic {
            location,
            span,
            rule,
            severity,
            raw_severity: Some(diag.severity.clone()),
            message: DisplayStr::from_untrusted(diag.message.clone()),
            related: vec![],
            notes,
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });

        // Count this item as successfully parsed.
        parsed_items += 1;
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OxlintDiagnostic {
    message: String,
    code: String,
    severity: String,
    filename: String,
    #[serde(default)]
    help: Option<String>,
    #[serde(default)]
    labels: Vec<OxlintLabel>,
}

#[derive(Deserialize)]
struct OxlintLabel {
    #[serde(default)]
    span: Option<OxlintSpan>,
}

#[derive(Deserialize)]
struct OxlintSpan {
    line: u32,
    column: u32,
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
    fn empty_diagnostics() {
        let stdout = r#"{"diagnostics":[]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn real_oxlint_output_no_debugger() {
        // Real oxlint output for a debugger statement (captured from oxlint . --format=json)
        let stdout = r#"{"diagnostics":[{"message":"`debugger` statement is not allowed","code":"eslint(no-debugger)","severity":"warning","causes":[],"url":"https://oxc.rs/docs/guide/usage/linter/rules/eslint/no-debugger.html","help":"Remove the debugger statement","filename":"test.js","labels":[{"span":{"offset":43,"length":9,"line":3,"column":1}}],"related":[]}],"number_of_files":1,"number_of_rules":93,"threads_count":10,"start_time":0.008049459}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.rule.as_deref(), Some("no-debugger"));
        assert_eq!(d.message, "`debugger` statement is not allowed");
        assert_eq!(d.location, Location::File("test.js".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 3);
        assert_eq!(span.column, 1);
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.notes[0], "Remove the debugger statement");
    }

    #[test]
    fn real_oxlint_output_eval() {
        // Real oxlint output for eval() call
        let stdout = r#"{"diagnostics":[{"message":"eval can be harmful.","code":"eslint(no-eval)","severity":"warning","causes":[],"url":"https://oxc.rs/docs/guide/usage/linter/rules/eslint/no-eval.html","help":"Avoid eval(). For JSON parsing use JSON.parse(); for dynamic property access use bracket notation (obj[key]); for other cases refactor to avoid evaluating strings as code.","filename":"test.js","labels":[{"span":{"offset":53,"length":4,"line":4,"column":1}}],"related":[]}],"number_of_files":1,"number_of_rules":93,"threads_count":10,"start_time":0.004167125}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.rule.as_deref(), Some("no-eval"));
        assert_eq!(d.message, "eval can be harmful.");
        assert_eq!(d.location, Location::File("test.js".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 4);
        assert_eq!(span.column, 1);
        assert!(d.notes[0].contains("JSON.parse"));
    }

    #[test]
    fn nested_span_structure() {
        // Test that we correctly parse span nested inside labels
        let stdout = r#"{"diagnostics":[{"message":"test","code":"rule","severity":"warning","causes":[],"filename":"file.js","labels":[{"span":{"offset":100,"length":5,"line":10,"column":20}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 10);
        assert_eq!(span.column, 20);
    }

    #[test]
    fn multiple_diagnostics() {
        let stdout = r#"{"diagnostics":[{"message":"msg1","code":"eslint(rule1)","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]},{"message":"msg2","code":"eslint(rule2)","severity":"error","causes":[],"filename":"src/app.js","labels":[{"span":{"offset":0,"length":1,"line":2,"column":2}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("rule1"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("rule2"));
    }

    #[test]
    fn no_labels() {
        let stdout = r#"{"diagnostics":[{"message":"Warning without label.","code":"eslint(rule)","severity":"warning","causes":[],"filename":"test.js","labels":[],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert!(d.span.is_none());
        assert_eq!(d.message, "Warning without label.");
    }

    #[test]
    fn rule_extraction_with_parens() {
        // Code like "eslint(no-debugger)" should extract "no-debugger"
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"eslint(no-debugger)","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("no-debugger"));
    }

    #[test]
    fn rule_without_parens() {
        // Code without parens should be kept as-is
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"custom-rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("custom-rule"));
    }

    #[test]
    fn absolute_filename() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"/project/src/test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/test.js".to_string())
        );
    }

    #[test]
    fn optional_help_field() {
        let stdout = r#"{"diagnostics":[{"message":"test msg","code":"rule","severity":"warning","help":"This is helpful text","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.notes[0], "This is helpful text");
    }

    #[test]
    fn missing_optional_fields() {
        // Minimal diagnostic with only required fields
        let stdout = r#"{"diagnostics":[{"message":"error","code":"rule","severity":"error","filename":"test.js"}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.message, "error");
        assert!(d.span.is_none());
        assert!(d.notes.is_empty());
    }

    #[test]
    fn unknown_severity_defaults_to_error() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"unknown","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn invalid_json() {
        let stdout = "not json at all";
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_oxlint_empty_input() {
        let result = parse("", "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_oxlint_huge_diagnostics() {
        // Create a large number of diagnostics with real structure
        let diag_entry = r#"{"message":"msg","code":"rule","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}"#;
        let huge = format!(
            r#"{{"diagnostics":[{}]}}"#,
            std::iter::repeat_n(diag_entry, 1_000)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = parse(&huge, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1_000);
        assert_eq!(result.parsed_items, 1_000);
    }

    #[test]
    fn stress_oxlint_unicode_in_paths() {
        let stdout = r#"{"diagnostics":[{"message":"错误 🔥","code":"rule","severity":"error","causes":[],"filename":"src/文件.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_oxlint_unicode_in_message() {
        let stdout = r#"{"diagnostics":[{"message":"Error: 日本語 RTL עברית","code":"rule","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("日本語"));
    }

    #[test]
    fn stress_oxlint_truncated_json() {
        let truncated = r#"{"diagnostics":[{"message":"msg","code":"rule"#;
        let result = parse(truncated, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_oxlint_whitespace_only() {
        let result = parse("   \n\n\t\t\n   ", "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_oxlint_binary_garbage() {
        let garbage = "{\x00\x01\x02}";
        let result = parse(garbage, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn security_oxlint_injection_quote_in_message() {
        let stdout = r#"{"diagnostics":[{"message":"error: \"quoted\" text","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "error: \"quoted\" text");
    }

    #[test]
    fn security_oxlint_injection_control_chars() {
        let stdout = r#"{"diagnostics":[{"message":"error\nwith\nnewlines","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // DisplayStr::from_untrusted strips newlines at parse time — no downstream injection risk.
        assert!(!result.diagnostics[0].message.contains('\n'));
        assert!(result.diagnostics[0].message.contains("error"));
    }

    #[test]
    fn security_oxlint_injection_backslash() {
        let stdout = r#"{"diagnostics":[{"message":"path: C:\\Users\\test","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("C:"));
    }

    #[test]
    fn stress_oxlint_extremely_long_message() {
        let long_msg = "a".repeat(10_000);
        let stdout = format!(
            r#"{{"diagnostics":[{{"message":"{}","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{{"span":{{"offset":0,"length":1,"line":1,"column":1}}}}],"related":[]}}]}}"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message.len(), 10_000);
    }

    #[test]
    fn stress_oxlint_malformed_array() {
        let stdout = r#"{"diagnostics":{"message":"error"}}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_oxlint_missing_diagnostics_key() {
        let stdout = r#"{"data":[],"metadata":{}}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_oxlint_partial_diagnostic_skipped() {
        let stdout = r#"{"diagnostics":[{"message":"msg1","code":"rule1","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]},{"message":"msg2","severity":"warning","causes":[],"filename":"test.js"}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
    }

    #[test]
    fn edge_case_missing_span_in_label() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn edge_case_empty_labels_array() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn edge_case_multiple_labels_uses_first() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}},{"span":{"offset":10,"length":2,"line":2,"column":3}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn edge_case_severity_warning() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn edge_case_severity_error() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn edge_case_code_format_variants() {
        let stdout1 = r#"{"diagnostics":[{"message":"m","code":"simple","severity":"warning","filename":"test.js"}]}"#;
        let result1 = parse(stdout1, "", root(), "oxlint", "javascript");
        assert_eq!(result1.diagnostics.len(), 1);
        assert_eq!(result1.diagnostics[0].rule.as_deref(), Some("simple"));

        let stdout2 = r#"{"diagnostics":[{"message":"m","code":"eslint(rule)","severity":"warning","filename":"test.js"}]}"#;
        let result2 = parse(stdout2, "", root(), "oxlint", "javascript");
        assert_eq!(result2.diagnostics.len(), 1);
        assert_eq!(result2.diagnostics[0].rule.as_deref(), Some("rule"));

        let stdout3 = r#"{"diagnostics":[{"message":"m","code":"plugin(nested)","severity":"warning","filename":"test.js"}]}"#;
        let result3 = parse(stdout3, "", root(), "oxlint", "javascript");
        assert_eq!(result3.diagnostics.len(), 1);
        assert_eq!(result3.diagnostics[0].rule.as_deref(), Some("nested"));
    }

    #[test]
    fn edge_case_raw_severity_preserved() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"custom-severity","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(
            result.diagnostics[0].raw_severity.as_deref(),
            Some("custom-severity")
        );
    }

    #[test]
    fn edge_case_no_help_creates_empty_notes() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(result.diagnostics[0].notes.len(), 0);
    }

    #[test]
    fn edge_case_special_characters_in_rule() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"eslint(no-unused-vars)","severity":"warning","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":1,"column":1}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        assert_eq!(
            result.diagnostics[0].rule.as_deref(),
            Some("no-unused-vars")
        );
    }

    #[test]
    fn edge_case_line_and_column_zero() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":0,"line":0,"column":0}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
    }

    #[test]
    fn edge_case_very_large_line_column_numbers() {
        let stdout = r#"{"diagnostics":[{"message":"msg","code":"rule","severity":"error","causes":[],"filename":"test.js","labels":[{"span":{"offset":0,"length":1,"line":999999,"column":999999}}],"related":[]}]}"#;
        let result = parse(stdout, "", root(), "oxlint", "javascript");
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 999999);
        assert_eq!(span.column, 999999);
    }
}
