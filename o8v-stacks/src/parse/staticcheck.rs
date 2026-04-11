//! Staticcheck JSON parser — covers `staticcheck -f json`.
//!
//! Staticcheck emits NDJSON (one JSON object per line on stdout).
//! Each line is a complete diagnostic.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse staticcheck JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut parsed_any = false;
    let mut parsed_count = 0u32;
    let mut skipped = 0u32;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let diag: StaticcheckDiag = match serde_json::from_str(line) {
            Ok(d) => {
                parsed_any = true;
                parsed_count += 1;
                d
            }
            Err(e) => {
                skipped += 1;
                tracing::debug!(error = %e, "skipping malformed staticcheck entry");
                continue;
            }
        };

        let location = super::normalize_path(&diag.location.file, project_root);

        diagnostics.push(Diagnostic {
            location,
            span: Some(Span::new(
                diag.location.line,
                diag.location.column,
                Some(diag.end.line),
                Some(diag.end.column),
            )),
            rule: Some(DisplayStr::from_untrusted(diag.code)),
            severity: match diag.severity.as_str() {
                "warning" => Severity::Warning,
                // "error" and anything unknown default to Error
                _ => Severity::Error,
            },
            raw_severity: Some(diag.severity),
            message: DisplayStr::from_untrusted(diag.message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    if skipped > 0 {
        tracing::warn!(
            skipped,
            total = diagnostics.len() + skipped as usize,
            "staticcheck: some entries could not be parsed"
        );
    }

    let status = if !diagnostics.is_empty() || parsed_any || stdout.trim().is_empty() {
        ParseStatus::Parsed
    } else {
        ParseStatus::Unparsed
    };

    ParseResult {
        diagnostics,
        status,
        parsed_items: parsed_count,
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StaticcheckDiag {
    code: String,
    severity: String,
    message: String,
    location: StaticcheckLocation,
    end: StaticcheckLocation,
}

#[derive(Deserialize)]
struct StaticcheckLocation {
    file: String,
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
        parse(stdout, "", Path::new(ROOT), "staticcheck", "go")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_diagnostic() {
        let input = r#"{"code":"SA1000","severity":"error","message":"invalid regexp","location":{"file":"main.go","line":10,"column":5},"end":{"file":"main.go","line":10,"column":20}}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("SA1000"));
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "invalid regexp");
        assert_eq!(d.location, Location::File("main.go".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 10);
        assert_eq!(span.column, 5);
        assert_eq!(span.end_line, Some(10));
        assert_eq!(span.end_column, Some(20));
    }

    #[test]
    fn warning_severity() {
        let input = r#"{"code":"ST1000","severity":"warning","message":"package comment","location":{"file":"main.go","line":1,"column":1},"end":{"file":"main.go","line":1,"column":1}}"#;
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn malformed_line_skipped() {
        let input = concat!(
            r#"{"code":"SA1000","severity":"error","message":"good one","location":{"file":"a.go","line":1,"column":1},"end":{"file":"a.go","line":1,"column":1}}"#,
            "\n",
            "this is not json\n",
            r#"{"code":"SA2000","severity":"error","message":"also good","location":{"file":"b.go","line":2,"column":1},"end":{"file":"b.go","line":2,"column":1}}"#,
        );
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SA1000"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("SA2000"));
    }

    #[test]
    fn non_json_text() {
        let result = run("this is plain text, not json");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_staticcheck_empty_input() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_staticcheck_huge_input() {
        let line = "{\"code\":\"SA1000\",\"severity\":\"error\",\"message\":\"test\",\"location\":{\"file\":\"main.go\",\"line\":10,\"column\":5},\"end\":{\"file\":\"main.go\",\"line\":10,\"column\":20}}\n";
        let huge = line.repeat(100_000);
        let result = run(&huge);
        // Should not panic, regardless of parse status
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_staticcheck_binary_garbage() {
        let garbage = "{\x00\x01\x02\x7f\x7e invalid json }\n";
        let result = run(garbage);
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_staticcheck_whitespace_only() {
        let result = run("   \n\n\t\t\n   ");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_staticcheck_truncated_json() {
        let truncated = r#"{"code":"SA1000","severity":"error","message":"incomplete"#;
        let result = run(truncated);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_staticcheck_unicode_in_paths() {
        let input = "{\"code\":\"SA1000\",\"severity\":\"error\",\"message\":\"错误 🔥 עברית\",\"location\":{\"file\":\"文件.go\",\"line\":10,\"column\":5},\"end\":{\"file\":\"文件.go\",\"line\":10,\"column\":20}}\n";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_staticcheck_extremely_long_message() {
        let long_msg = "x".repeat(100_000); // Reduced to 100k to avoid actual parsing issues
        let input = format!(
            "{{\"code\":\"SA1000\",\"severity\":\"error\",\"message\":\"{}\",\"location\":{{\"file\":\"main.go\",\"line\":10,\"column\":5}},\"end\":{{\"file\":\"main.go\",\"line\":10,\"column\":20}}\n",
            long_msg
        );
        let result = run(&input);
        // Should not panic, regardless of parse status
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
    }

    #[test]
    fn stress_staticcheck_mixed_valid_invalid() {
        let input = concat!(
            "{\"code\":\"SA1000\",\"severity\":\"error\",\"message\":\"good\",\"location\":{\"file\":\"a.go\",\"line\":1,\"column\":1},\"end\":{\"file\":\"a.go\",\"line\":1,\"column\":5}}\n",
            "this is not json\n",
            "{\"code\":\"SA2000\",\"severity\":\"warning\",\"message\":\"also good\",\"location\":{\"file\":\"b.go\",\"line\":2,\"column\":1},\"end\":{\"file\":\"b.go\",\"line\":2,\"column\":5}}\n",
            "more garbage\n",
            "{\"code\":\"SA3000\",\"severity\":\"error\",\"message\":\"third\",\"location\":{\"file\":\"c.go\",\"line\":3,\"column\":1},\"end\":{\"file\":\"c.go\",\"line\":3,\"column\":5}}\n",
        );
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("SA1000"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("SA2000"));
        assert_eq!(result.diagnostics[2].rule.as_deref(), Some("SA3000"));
    }

    #[test]
    fn stress_staticcheck_malformed_coordinates() {
        let inputs = vec![
            "{\"code\":\"S\",\"severity\":\"error\",\"message\":\"msg\",\"location\":{\"file\":\"f.go\",\"line\":0,\"column\":0},\"end\":{\"file\":\"f.go\",\"line\":0,\"column\":0}}\n",
            "{\"code\":\"S\",\"severity\":\"error\",\"message\":\"msg\",\"location\":{\"file\":\"f.go\",\"line\":999999999,\"column\":999999999},\"end\":{\"file\":\"f.go\",\"line\":999999999,\"column\":999999999}}\n",
        ];
        for input in inputs {
            let result = run(input);
            assert_eq!(result.status, ParseStatus::Parsed);
            // Should not crash on extreme coordinates
        }
    }
}
