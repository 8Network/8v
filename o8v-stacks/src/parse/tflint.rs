//! tflint JSON parser — covers `tflint --format=json`.
//!
//! tflint emits JSON with top-level `issues` and `errors` arrays.
//! Each issue includes rule name, severity, location, and message.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse tflint JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let output: TflintOutput = match serde_json::from_str(stdout) {
        Ok(o) => o,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    let mut diagnostics = Vec::new();
    let parsed_count = output.issues.len() as u32;

    for issue in output.issues {
        let location = super::normalize_path(&issue.range.filename, project_root);

        let severity = match issue.rule.severity.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "notice" => Severity::Info,
            _ => {
                tracing::debug!(severity = %issue.rule.severity, "unknown tflint severity");
                Severity::Warning
            }
        };

        let span = Span::new(
            issue.range.start.line,
            issue.range.start.column,
            Some(issue.range.end.line),
            Some(issue.range.end.column),
        );

        diagnostics.push(Diagnostic {
            location,
            span: Some(span),
            rule: Some(DisplayStr::from_untrusted(issue.rule.name.clone())),
            severity,
            raw_severity: Some(issue.rule.severity.clone()),
            message: DisplayStr::from_untrusted(issue.message),
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
        parsed_items: parsed_count,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TflintOutput {
    issues: Vec<TflintIssue>,
    #[serde(default)]
    #[allow(dead_code)]
    errors: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct TflintIssue {
    rule: TflintRule,
    message: String,
    range: TflintRange,
}

#[derive(Deserialize)]
struct TflintRule {
    name: String,
    severity: String,
}

#[derive(Deserialize)]
struct TflintRange {
    filename: String,
    start: TflintPosition,
    end: TflintPosition,
}

#[derive(Deserialize)]
struct TflintPosition {
    line: u32,
    column: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "tflint", "terraform")
    }

    #[test]
    fn empty_output() {
        let result = run(r#"{"issues":[],"errors":[]}"#);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn single_issue() {
        let input = r#"{
  "issues": [
    {
      "rule": {
        "name": "terraform_unused_required_providers",
        "severity": "warning",
        "link": "https://github.com/terraform-linters/tflint/blob/master/docs/rules/terraform_unused_required_providers.md"
      },
      "message": "provider \"aws\" is not used by the configuration",
      "range": {
        "filename": "main.tf",
        "start": {
          "line": 1,
          "column": 1
        },
        "end": {
          "line": 1,
          "column": 34
        }
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
        let d = &result.diagnostics[0];
        assert_eq!(
            d.rule.as_deref(),
            Some("terraform_unused_required_providers")
        );
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(
            d.message,
            "provider \"aws\" is not used by the configuration"
        );
        assert_eq!(d.location, Location::File("main.tf".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, Some(1));
        assert_eq!(span.end_column, Some(34));
    }

    #[test]
    fn error_severity() {
        let input = r#"{
  "issues": [
    {
      "rule": {
        "name": "terraform_required_version",
        "severity": "error"
      },
      "message": "terraform version is not set",
      "range": {
        "filename": "main.tf",
        "start": {"line": 5, "column": 1},
        "end": {"line": 5, "column": 20}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn notice_severity() {
        let input = r#"{
  "issues": [
    {
      "rule": {
        "name": "terraform_naming_convention",
        "severity": "notice"
      },
      "message": "variable name should follow snake_case",
      "range": {
        "filename": "variables.tf",
        "start": {"line": 10, "column": 1},
        "end": {"line": 10, "column": 30}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn multiple_issues() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "rule1", "severity": "error"},
      "message": "issue 1",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    },
    {
      "rule": {"name": "rule2", "severity": "warning"},
      "message": "issue 2",
      "range": {
        "filename": "variables.tf",
        "start": {"line": 5, "column": 2},
        "end": {"line": 5, "column": 15}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
    }

    #[test]
    fn invalid_json() {
        let result = run("not json");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn multiline_message() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "terraform_required_providers", "severity": "error"},
      "message": "Terraform block must have a required_providers block.\nSee https://example.com for details.",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 20}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("required_providers"));
    }

    #[test]
    fn stress_huge_number_of_issues() {
        let mut issues = Vec::new();
        for i in 0..1000 {
            issues.push(format!(
                r#"{{
      "rule": {{"name": "rule_{}", "severity": "warning"}},
      "message": "issue {}",
      "range": {{
        "filename": "main.tf",
        "start": {{"line": {}, "column": 1}},
        "end": {{"line": {}, "column": 10}}
      }}
    }}"#,
                i,
                i,
                i + 1,
                i + 1
            ));
        }
        let input = format!(r#"{{"issues": [{}], "errors": []}}"#, issues.join(","));
        let result = run(&input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1000);
        assert_eq!(result.parsed_items, 1000);
    }

    #[test]
    fn stress_unicode_filenames() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "warning"},
      "message": "Unicode path 测试 🔥",
      "range": {
        "filename": "文件.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_very_large_line_numbers() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "warning"},
      "message": "Large line number",
      "range": {
        "filename": "main.tf",
        "start": {"line": 999999999, "column": 1},
        "end": {"line": 999999999, "column": 100}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_whitespace_only_output() {
        let result = run("   \n\n  ");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_empty_issues_array() {
        let result = run(r#"{"issues": [], "errors": []}"#);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_nested_paths() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "warning"},
      "message": "Nested path",
      "range": {
        "filename": "modules/vpc/main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 20}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("modules/vpc/main.tf".to_string())
        );
    }

    #[test]
    fn stress_tflint_empty_input() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_tflint_huge_input() {
        let mut issues = Vec::new();
        for i in 0..50_000 {
            issues.push(format!(
                r#"{{
      "rule": {{"name": "rule_{}", "severity": "warning"}},
      "message": "issue {}",
      "range": {{
        "filename": "main.tf",
        "start": {{"line": {}, "column": 1}},
        "end": {{"line": {}, "column": 10}}
      }}
    }}"#,
                i,
                i,
                i + 1,
                i + 1
            ));
        }
        let input = format!(r#"{{"issues": [{}], "errors": []}}"#, issues.join(","));
        let result = run(&input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 50_000);
        assert_eq!(result.parsed_items, 50_000);
    }

    #[test]
    fn stress_tflint_binary_garbage() {
        let garbage = r#"{"issues": [{"rule": {"name": "test\x00\x01\x02", "severity": "error"}, "message": "msg", "range": {"filename": "f.tf", "start": {"line": 1, "column": 1}, "end": {"line": 1, "column": 10}}}], "errors": []}"#;
        let result = run(garbage);
        // Binary bytes in JSON string values should parse OK; serde_json handles escape sequences
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_tflint_truncated_json() {
        let truncated = r#"{"issues": [{"rule": {"name": "rule1", "severity": "error"}, "message": "incomplete"#;
        let result = run(truncated);
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_tflint_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let input = format!(
            r#"{{
  "issues": [
    {{
      "rule": {{"name": "test_rule", "severity": "warning"}},
      "message": "{}",
      "range": {{
        "filename": "main.tf",
        "start": {{"line": 1, "column": 1}},
        "end": {{"line": 1, "column": 10}}
      }}
    }}
  ],
  "errors": []
}}"#,
            long_msg
        );
        let result = run(&input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
    }

    #[test]
    fn stress_tflint_malformed_json() {
        let inputs = vec![
            "{broken json",
            "{ \"issues\": [invalid] }",
            "{ \"issues\": [{\"rule\": null}] }",
            "null",
            "\"string\"",
            "123",
            "[]",
        ];
        for input in inputs {
            let result = run(input);
            assert_eq!(result.status, ParseStatus::Unparsed);
            assert_eq!(result.diagnostics.len(), 0);
        }
    }

    #[test]
    fn stress_tflint_missing_issues_field() {
        let input = r#"{"errors": []}"#;
        let result = run(input);
        // issues field does NOT have #[serde(default)], so missing it fails deserialization
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_tflint_missing_errors_field() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "warning"},
      "message": "test",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    }
  ]
}"#;
        let result = run(input);
        // errors field has #[serde(default)] so missing is OK
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_tflint_null_fields() {
        let inputs = vec![
            // null message
            r#"{"issues": [{"rule": {"name": "rule1", "severity": "warning"}, "message": null, "range": {"filename": "f.tf", "start": {"line": 1, "column": 1}, "end": {"line": 1, "column": 10}}}], "errors": []}"#,
            // null rule name
            r#"{"issues": [{"rule": {"name": null, "severity": "warning"}, "message": "msg", "range": {"filename": "f.tf", "start": {"line": 1, "column": 1}, "end": {"line": 1, "column": 10}}}], "errors": []}"#,
            // null severity
            r#"{"issues": [{"rule": {"name": "rule1", "severity": null}, "message": "msg", "range": {"filename": "f.tf", "start": {"line": 1, "column": 1}, "end": {"line": 1, "column": 10}}}], "errors": []}"#,
        ];
        for input in inputs {
            let result = run(input);
            // null in required string fields should fail deserialization
            assert_eq!(result.status, ParseStatus::Unparsed);
        }
    }

    #[test]
    fn stress_tflint_missing_rule_fields() {
        let input = r#"{
  "issues": [
    {
      "rule": {"severity": "warning"},
      "message": "missing rule name",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        // Missing required field "name" should fail deserialization
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_tflint_missing_range_fields() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "rule1", "severity": "warning"},
      "message": "test",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        // Missing required field "end" in range should fail deserialization
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_tflint_unknown_severity() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "unknown_severity"},
      "message": "test",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        // Unknown severity should fall back to Warning
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].raw_severity,
            Some("unknown_severity".to_string())
        );
    }

    #[test]
    fn stress_tflint_zero_line_column() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "test_rule", "severity": "warning"},
      "message": "test",
      "range": {
        "filename": "main.tf",
        "start": {"line": 0, "column": 0},
        "end": {"line": 0, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        // Zero line/column is clamped to 1 by Span normalizer
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 1);
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().column, 1);
    }

    #[test]
    fn stress_tflint_empty_string_fields() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "", "severity": ""},
      "message": "",
      "range": {
        "filename": "",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        // Empty strings are valid, should parse
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning); // unknown → Warning
    }

    #[test]
    fn stress_tflint_mixed_severity_types() {
        let input = r#"{
  "issues": [
    {
      "rule": {"name": "error_rule", "severity": "error"},
      "message": "error",
      "range": {
        "filename": "main.tf",
        "start": {"line": 1, "column": 1},
        "end": {"line": 1, "column": 10}
      }
    },
    {
      "rule": {"name": "warning_rule", "severity": "warning"},
      "message": "warning",
      "range": {
        "filename": "main.tf",
        "start": {"line": 2, "column": 1},
        "end": {"line": 2, "column": 10}
      }
    },
    {
      "rule": {"name": "notice_rule", "severity": "notice"},
      "message": "notice",
      "range": {
        "filename": "main.tf",
        "start": {"line": 3, "column": 1},
        "end": {"line": 3, "column": 10}
      }
    }
  ],
  "errors": []
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
        assert_eq!(result.diagnostics[2].severity, Severity::Info);
    }
}
