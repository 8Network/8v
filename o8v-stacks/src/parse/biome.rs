//! `Biome` JSON parser — covers `biome ci --reporter=json`.
//!
//! `Biome` emits a JSON object with a `diagnostics` array on stdout.
//! Each diagnostic is a structured object with severity, message, category, and location.
//! The location includes a string path and optional start/end position objects with line/column.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse biome JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // Parse as a root object with diagnostics array.
    let output: BiomeOutput = match serde_json::from_str(stdout) {
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
    let mut parsed_items = 0u32;
    let mut skipped = 0u32;

    for diag in output.diagnostics {
        // Extract file path from location (path is a string, not an object).
        let file_path = match &diag.location.path {
            Some(path) => path.clone(),
            None => {
                skipped += 1;
                tracing::debug!("biome: skipping diagnostic with no file path");
                continue;
            }
        };

        let location = super::normalize_path(&file_path, project_root);

        // Map severity string to our Severity enum.
        let severity = match diag.severity.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "information" | "info" => Severity::Info,
            _ => {
                skipped += 1;
                tracing::debug!(
                    sev = %diag.severity,
                    "biome: skipping diagnostic with unknown severity"
                );
                continue;
            }
        };

        // Extract span from start/end positions (0-indexed in JSON, convert to 1-indexed for Span).
        let span = match (&diag.location.start, &diag.location.end) {
            (Some(start), Some(end)) => Some(Span {
                line: start.line + 1,
                column: start.column + 1,
                end_line: Some(end.line + 1),
                end_column: Some(end.column + 1),
            }),
            _ => None,
        };

        diagnostics.push(Diagnostic {
            location,
            span,
            rule: Some(DisplayStr::from_untrusted(diag.category)),
            severity,
            raw_severity: Some(diag.severity),
            message: DisplayStr::from_untrusted(diag.message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: diag
                .advices
                .as_ref()
                .and_then(|a| a.first().and_then(|advice| advice.source_code.clone())),
            tool: tool.to_string(),
            stack: stack.to_string(),
        });

        // Count this item as successfully parsed.
        parsed_items += 1;
    }

    if skipped > 0 {
        tracing::warn!(skipped, "biome: some diagnostics could not be parsed");
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BiomeOutput {
    diagnostics: Vec<BiomeDiagnostic>,
}

#[derive(Deserialize)]
struct BiomeDiagnostic {
    severity: String,
    message: String,
    category: String,
    location: BiomeLocation,
    #[serde(default)]
    advices: Option<Vec<BiomeAdvice>>,
}

#[derive(Deserialize)]
struct BiomeLocation {
    path: Option<String>,
    #[serde(default)]
    start: Option<BiomePosition>,
    #[serde(default)]
    end: Option<BiomePosition>,
}

#[derive(Deserialize)]
struct BiomePosition {
    line: u32,
    column: u32,
}

#[derive(Deserialize)]
struct BiomeAdvice {
    #[serde(rename = "sourceCode")]
    source_code: Option<String>,
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
        let result = parse(r#"{"diagnostics":[]}"#, "", root(), "biome", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn real_biome_output_with_line_column() {
        // Real output from biome ci --reporter=json with actual start/end positions
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"Using == may be unsafe if you are relying on type coercion.","category":"lint/suspicious/noDoubleEquals","location":{"path":"test.js","start":{"line":1,"column":7},"end":{"line":1,"column":9}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("lint/suspicious/noDoubleEquals"));
        assert_eq!(
            d.message,
            "Using == may be unsafe if you are relying on type coercion."
        );
        assert_eq!(d.location, Location::File("test.js".to_string()));
        // Verify span is converted from 0-indexed to 1-indexed
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 8);
        assert_eq!(span.end_line, Some(2));
        assert_eq!(span.end_column, Some(10));
    }

    #[test]
    fn biome_format_error() {
        // Real format error from biome (no line/column in location)
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"File content differs from formatting output","category":"format","location":{"path":"package.json","start":{"line":0,"column":0},"end":{"line":0,"column":0}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("format"));
        assert_eq!(d.location, Location::File("package.json".to_string()));
    }

    #[test]
    fn biome_with_debugger_error() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"This is an unexpected use of the debugger statement.","category":"lint/suspicious/noDebugger","location":{"path":"test.js","start":{"line":2,"column":0},"end":{"line":2,"column":9}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("lint/suspicious/noDebugger"));
        // Line 2 (0-indexed) becomes line 3 (1-indexed)
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 3);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, Some(3));
        assert_eq!(span.end_column, Some(10));
    }

    #[test]
    fn biome_warning_severity() {
        let stdout = r#"{"diagnostics":[{"severity":"warning","message":"Some warning","category":"lint/style/rule","location":{"path":"file.js","start":{"line":5,"column":10},"end":{"line":5,"column":15}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn biome_info_severity() {
        let stdout = r#"{"diagnostics":[{"severity":"information","message":"Some info","category":"lint/info/rule","location":{"path":"test.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json", "", root(), "biome", "javascript");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn missing_file_path() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"start":{"line":0,"column":0},"end":{"line":0,"column":5}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn multiple_diagnostics() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"Error 1","category":"rule1","location":{"path":"file1.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}},"advices":[]},{"severity":"warning","message":"Warning 1","category":"rule2","location":{"path":"file2.js","start":{"line":5,"column":3},"end":{"line":5,"column":7}},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn absolute_path_preserved() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"/other/file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::Absolute("/other/file.js".to_string())
        );
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_biome_empty_input() {
        let result = parse("", "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_biome_huge_input() {
        let diag = r#"{"severity":"error","message":"error","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}},"advices":[]}"#;
        let huge = format!(
            r#"{{"diagnostics":[{}]}}"#,
            std::iter::repeat_n(diag, 10_000)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = parse(&huge, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10_000);
    }

    #[test]
    fn stress_biome_whitespace_only() {
        let result = parse("   \n\n\t\t\n   ", "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_biome_truncated_json() {
        let truncated = r#"{"diagnostics":[{"severity":"error"#;
        let result = parse(truncated, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_biome_unicode_in_paths() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"错误 🔥","category":"rule","location":{"path":"src/文件.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_biome_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let stdout = format!(
            r#"{{"diagnostics":[{{"severity":"error","message":"{}","category":"rule","location":{{"path":"file.js","start":{{"line":0,"column":0}},"end":{{"line":0,"column":1}}}},"advices":[]}}]}}"#,
            long_msg
        );
        let result = parse(&stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn security_biome_injection_quote_in_message() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"error: \"quoted\" text","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "error: \"quoted\" text");
    }

    #[test]
    fn security_biome_injection_control_chars() {
        // DisplayStr::from_untrusted strips control chars at parse time — no downstream injection risk.
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"error\nwith\nnewlines","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":1,"column":5}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics[0].message.contains('\n'));
        assert!(result.diagnostics[0].message.contains("error"));
    }

    #[test]
    fn unknown_severity_skipped() {
        let stdout = r#"{"diagnostics":[{"severity":"critical","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn biome_optional_span() {
        // Some diagnostics might not have start/end (though real biome always has them)
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js"},"advices":[]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn real_biome_ci_output_with_summary() {
        // Real biome ci --reporter=json output with summary, diagnostics, and command
        let stdout = r#"{"summary":{"changed":0,"unchanged":3,"matches":0,"duration":1388459,"errors":4,"warnings":1},"diagnostics":[{"severity":"error","message":"Using == may be unsafe if you are relying on type coercion.","category":"lint/suspicious/noDoubleEquals","location":{"path":"test.js","start":{"line":4,"column":13},"end":{"line":4,"column":15}},"advices":[]}],"command":"ci"}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("lint/suspicious/noDoubleEquals"));
    }

    #[test]
    fn stress_biome_binary_garbage() {
        let garbage = "\x00\x01\x02";
        let result = parse(garbage, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_biome_malformed_json() {
        let malformed_cases = vec![
            r#"{"diagnostics":[{incomplete}]}"#,
            r#"{"diagnostics":[}]}"#,
            r#"{"diagnostics": not-a-list}"#,
            r#"{invalid json}"#,
        ];
        for input in malformed_cases {
            let result = parse(input, "", root(), "biome", "javascript");
            assert_eq!(result.status, ParseStatus::Unparsed);
            assert_eq!(result.diagnostics.len(), 0);
        }
    }

    #[test]
    fn edge_empty_diagnostics_with_extra_fields() {
        let stdout = r#"{"diagnostics":[],"summary":{"errors":0,"warnings":0},"command":"ci"}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn edge_missing_diagnostics_field() {
        let stdout = r#"{"summary":{"errors":0},"command":"ci"}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn edge_missing_location_field() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule"}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn edge_missing_severity_field() {
        let stdout = r#"{"diagnostics":[{"message":"msg","category":"rule","location":{"path":"file.js"}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn edge_diagnostic_with_missing_start_only() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":5}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn edge_diagnostic_with_missing_end_only() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js","end":{"line":5,"column":10}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn edge_zero_indexed_line_column_conversion() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":3}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, Some(1));
        assert_eq!(span.end_column, Some(4));
    }

    #[test]
    fn edge_large_line_numbers() {
        let stdout = r#"{"diagnostics":[{"severity":"warning","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":999999,"column":999999},"end":{"line":999999,"column":999999}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 1000000);
        assert_eq!(span.column, 1000000);
    }

    #[test]
    fn edge_multiple_advices_uses_first() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}},"advices":[{"sourceCode":"first advice"},{"sourceCode":"second advice"}]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].snippet.as_deref(),
            Some("first advice")
        );
    }

    #[test]
    fn edge_advice_without_source_code() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}},"advices":[{"context":"some context"}]}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].snippet.is_none());
    }

    #[test]
    fn edge_message_with_special_characters() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"Error with \"quotes\" and \\ backslash","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("quotes"));
        assert!(result.diagnostics[0].message.contains("backslash"));
    }

    #[test]
    fn stress_biome_mixed_severities_with_skipped() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"error msg","category":"rule1","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}},{"severity":"invalid-sev","message":"invalid msg","category":"rule2","location":{"path":"file.js","start":{"line":1,"column":0},"end":{"line":1,"column":1}}},{"severity":"warning","message":"warning msg","category":"rule3","location":{"path":"file.js","start":{"line":2,"column":0},"end":{"line":2,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.parsed_items, 2);
    }

    #[test]
    fn stress_biome_all_missing_paths() {
        let stdout = r#"{"diagnostics":[{"severity":"error","message":"msg1","category":"rule1","location":{"start":{"line":0,"column":0},"end":{"line":0,"column":1}}},{"severity":"warning","message":"msg2","category":"rule2","location":{"start":{"line":1,"column":0},"end":{"line":1,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_biome_info_severity_lowercase() {
        let stdout = r#"{"diagnostics":[{"severity":"info","message":"info msg","category":"rule","location":{"path":"file.js","start":{"line":0,"column":0},"end":{"line":0,"column":1}}}]}"#;
        let result = parse(stdout, "", root(), "biome", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }
}
