//! go vet JSON parser — covers `go vet -json ./...`.
//!
//! go vet emits pretty-printed multi-line JSON objects (one per package).
//! Each object is keyed by `package -> analyzer -> [diagnostic]`.
//! Uses `serde_json::StreamDeserializer` for multi-object stream.
//!
//! IMPORTANT: `go vet -json` exits 0 even with findings.
//! Pass/fail must be determined by diagnostic count, not exit code.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;
use std::collections::HashMap;

/// Parse go vet JSON output into diagnostics.
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

    // go vet emits a stream of JSON objects (one per package), each pretty-printed.
    let stream = serde_json::Deserializer::from_str(stdout).into_iter::<GoVetPackage>();

    for result in stream {
        let pkg = match result {
            Ok(p) => {
                parsed_any = true;
                parsed_count += 1;
                p
            }
            Err(e) => {
                skipped += 1;
                tracing::debug!(error = %e, "skipping malformed go vet object");
                continue;
            }
        };

        // pkg is keyed by package path -> analyzer -> [finding]
        for analyzers in pkg.0.values() {
            for (analyzer_name, findings) in analyzers {
                for finding in findings {
                    let posn = parse_posn(&finding.posn);
                    let location = super::normalize_path(&posn.file, project_root);

                    let end_span = finding.end.as_deref().and_then(|e| {
                        let end_posn = parse_posn(e);
                        if end_posn.line > 0 {
                            Some((end_posn.line, end_posn.col))
                        } else {
                            None
                        }
                    });

                    diagnostics.push(Diagnostic {
                        location,
                        span: if posn.line > 0 {
                            Some(Span::new(
                                posn.line,
                                posn.col,
                                end_span.map(|(l, _)| l),
                                end_span.map(|(_, c)| c),
                            ))
                        } else {
                            None
                        },
                        rule: Some(DisplayStr::from_untrusted(analyzer_name.clone())),
                        severity: Severity::Error, // go vet has no severity field
                        raw_severity: None,
                        message: DisplayStr::from_untrusted(finding.message.clone()),
                        related: vec![],
                        notes: vec![],
                        suggestions: vec![],
                        snippet: None,
                        tool: tool.to_string(),
                        stack: stack.to_string(),
                    });
                }
            }
        }
    }

    if skipped > 0 {
        tracing::warn!(
            skipped,
            total = diagnostics.len() + skipped as usize,
            "go vet: some objects could not be parsed"
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

/// Parsed position from go vet's posn string.
struct Position {
    file: String,
    line: u32,
    col: u32,
}

/// Parse go vet's `posn` string: "file.go:line:col" or "file.go:line"
///
/// Uses `rsplitn(3, ':')` to split from the right. This correctly handles
/// Windows drive letters (C:\path:10:5) because rsplitn limits to 3 parts:
/// the remainder includes the drive letter colon.
fn parse_posn(posn: &str) -> Position {
    let parts: Vec<&str> = posn.rsplitn(3, ':').collect();
    match parts.len() {
        3 => {
            let col = parts[0].parse().map_or(1, |n| n);
            let line = parts[1].parse().map_or(0, |n| n);
            let file = parts[2].to_string();
            Position { file, line, col }
        }
        2 => {
            let line = parts[0].parse().map_or(0, |n| n);
            let file = parts[1].to_string();
            Position { file, line, col: 1 }
        }
        // Can't parse position — use empty filename (becomes Location::Absolute)
        // instead of stuffing the raw posn string as a filename.
        _ => Position {
            file: String::new(),
            line: 0,
            col: 0,
        },
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

/// go vet JSON: one object per package, keyed by package path -> analyzer -> findings
#[derive(Deserialize)]
struct GoVetPackage(HashMap<String, HashMap<String, Vec<GoVetFinding>>>);

#[derive(Deserialize)]
struct GoVetFinding {
    posn: String,
    end: Option<String>,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "go-vet", "go")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_finding() {
        let input = r#"{
  "example.com/pkg": {
    "printf": [
      {
        "posn": "main.go:15:2",
        "message": "fmt.Sprintf format %d has arg of wrong type"
      }
    ]
  }
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("printf"));
        assert_eq!(d.message, "fmt.Sprintf format %d has arg of wrong type");
        assert_eq!(d.location, Location::File("main.go".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 15);
        assert_eq!(span.column, 2);
    }

    #[test]
    fn posn_line_col() {
        let posn = parse_posn("file.go:10:5");
        assert_eq!(posn.file, "file.go");
        assert_eq!(posn.line, 10);
        assert_eq!(posn.col, 5);
    }

    #[test]
    fn posn_line_only() {
        let posn = parse_posn("file.go:10");
        assert_eq!(posn.file, "file.go");
        assert_eq!(posn.line, 10);
        assert_eq!(posn.col, 1);
    }

    #[test]
    fn posn_invalid() {
        let posn = parse_posn("garbage");
        assert_eq!(posn.file, "");
        assert_eq!(posn.line, 0);
        assert_eq!(posn.col, 0);
    }

    #[test]
    fn malformed_json() {
        let result = run("this is not json at all");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn finding_with_end() {
        let input = r#"{
  "example.com/pkg": {
    "printf": [
      {
        "posn": "main.go:15:2",
        "end": "main.go:15:20",
        "message": "some message"
      }
    ]
  }
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 15);
        assert_eq!(span.column, 2);
        assert_eq!(span.end_line, Some(15));
        assert_eq!(span.end_column, Some(20));
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_govet_empty_input() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_govet_huge_input() {
        let obj =
            r#"{"example.com/pkg": {"printf": [{"posn": "main.go:15:2", "message": "msg"}]}}"#;
        let mut huge = String::new();
        for _ in 0..10_000 {
            huge.push_str(obj);
            huge.push('\n');
        }
        let result = run(&huge);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_govet_binary_garbage() {
        let garbage = "{\x00\x01\x02 invalid json }";
        let result = run(garbage);
        // StreamDeserializer should skip or error gracefully
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
    }

    #[test]
    fn stress_govet_whitespace_only() {
        let result = run("   \n\n\t\t\n   ");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_govet_truncated_json() {
        let truncated = r#"{"example.com/pkg": {"printf": [{"posn"#;
        let result = run(truncated);
        // Truncated JSON should fail to deserialize
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn stress_govet_unicode_in_paths() {
        let input = r#"{
  "example.com/pkg": {
    "printf": [
      {
        "posn": "文件.go:15:2",
        "message": "错误 🔥 עברית"
      }
    ]
  }
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should handle Unicode in paths and messages
    }

    #[test]
    fn stress_govet_extremely_long_message() {
        let long_msg = "x".repeat(1_000_000);
        let input = format!(
            r#"{{
  "example.com/pkg": {{
    "printf": [
      {{
        "posn": "main.go:15:2",
        "message": "{}"
      }}
    ]
  }}
}}"#,
            long_msg
        );
        let result = run(&input);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn stress_govet_deeply_nested_packages() {
        let input = r#"{
  "pkg1": {"analyzer1": [{"posn": "file1.go:1:1", "message": "msg1"}]},
  "pkg2": {"analyzer2": [{"posn": "file2.go:2:2", "message": "msg2"}]},
  "pkg3": {"analyzer3": [{"posn": "file3.go:3:3", "message": "msg3"}]},
  "pkg4": {"analyzer4": [{"posn": "file4.go:4:4", "message": "msg4"}]},
  "pkg5": {"analyzer5": [{"posn": "file5.go:5:5", "message": "msg5"}]}
}"#;
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 5);
    }

    #[test]
    fn stress_govet_malformed_posn() {
        let malformed = vec![
            r#"{"pkg": {"a": [{"posn": "", "message": "empty posn"}]}}"#,
            r#"{"pkg": {"a": [{"posn": "invalid", "message": "no colons"}]}}"#,
            r#"{"pkg": {"a": [{"posn": "file.go:", "message": "empty line"}]}}"#,
            r#"{"pkg": {"a": [{"posn": "file.go:999999999:888888888", "message": "huge"}]}}"#,
        ];
        for input in malformed {
            let result = run(input);
            assert_eq!(result.status, ParseStatus::Parsed);
            // Malformed posn should be handled gracefully
        }
    }
}
