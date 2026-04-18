//! Go test/build output parsers.
//!
//! ## go test -json
//!
//! `go test -json` emits NDJSON (one JSON object per line). Each object has an
//! `Action` field. A test failure is recorded as two events:
//!
//! 1. `{"Action":"fail","Test":"TestFoo","Package":"pkg"}` — the final verdict.
//! 2. `{"Action":"output","Test":"TestFoo","Output":"--- FAIL: TestFoo\n..."}` — the output.
//!
//! We collect output lines per test name, then emit one `Diagnostic` per
//! `fail` event that has a non-empty `Test` field. Package-level fail events
//! (no `Test` field) are skipped — they duplicate individual test failures.
//!
//! ## go build errors
//!
//! `go build ./...` on stable toolchains emits plain text:
//! ```text
//! ./src/main.go:10:5: undefined: foo
//! ```
//! Pattern: `<path>:<line>:<col>: <message>`.
//! We parse this with a simple line scanner — no regex crate required.
//!
//! This parser is called from `go_extract` dispatching on `RunKind`.

use o8v_core::diagnostic::{Diagnostic, Location, Severity};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;
use std::collections::HashMap;

/// One NDJSON event from `go test -json`.
#[derive(Debug, Deserialize)]
struct GoTestEvent {
    #[serde(rename = "Action")]
    action: String,
    /// Present for test-level events (absent for package-level).
    #[serde(rename = "Test")]
    test: Option<String>,
    /// Captured output for `action = "output"`.
    #[serde(rename = "Output")]
    output: Option<String>,
    /// Package path.
    #[serde(rename = "Package")]
    package: Option<String>,
}

/// Parse `go test -json` NDJSON output into diagnostics.
///
/// Returns one [`Diagnostic`] per failed test. Package-level fail events
/// (no `Test` field) are silently ignored to avoid duplicates.
#[must_use]
pub fn parse_test(
    stdout: &str,
    _stderr: &str,
    _project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // Accumulate output lines per test name.
    let mut output_buf: HashMap<String, String> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(event) = serde_json::from_str::<GoTestEvent>(line) else {
            tracing::debug!(line, "skipping non-JSON line in go test output");
            continue;
        };

        match event.action.as_str() {
            "output" => {
                if let (Some(name), Some(out)) = (event.test, event.output) {
                    output_buf.entry(name).or_default().push_str(&out);
                }
            }
            "fail" => {
                // Skip package-level fail events (no Test field).
                let Some(name) = event.test else { continue };
                let raw_output = output_buf.remove(&name).unwrap_or_default();
                // Pick the most informative single line as the headline message:
                // prefer the first `file.go:N: ...` line (assertion), else fall
                // back to the first non-`=== RUN` / non-`--- FAIL` line, else
                // a plain "failed" marker.
                let headline = raw_output
                    .lines()
                    .map(str::trim)
                    .find(|l| {
                        !l.is_empty()
                            && !l.starts_with("=== RUN")
                            && !l.starts_with("--- FAIL")
                            && !l.starts_with("--- PASS")
                    })
                    .unwrap_or("failed");
                let message = format!("test `{name}` failed: {headline}");
                let notes = if raw_output.trim().is_empty() {
                    vec![]
                } else {
                    vec![raw_output.trim_end().to_string()]
                };
                // Include package in location hint if available.
                let loc_str = if let Some(pkg) = &event.package {
                    format!("{pkg}::{name}")
                } else {
                    name.clone()
                };
                diagnostics.push(Diagnostic {
                    location: Location::Absolute(loc_str),
                    span: None,
                    rule: None,
                    severity: Severity::Error,
                    raw_severity: Some("error".to_string()),
                    message: DisplayStr::from_untrusted(&message),
                    related: vec![],
                    notes,
                    suggestions: vec![],
                    snippet: None,
                    tool: tool.to_string(),
                    stack: stack.to_string(),
                });
            }
            _ => {
                tracing::debug!(
                    action = %event.action,
                    "skipping non-fail go test event"
                );
            }
        }
    }

    diagnostics
}

/// Parse `go build ./...` plain-text output into diagnostics.
///
/// Lines match `<path>:<line>:<col>: <message>`.
/// Lines that don't match the pattern are silently skipped.
#[must_use]
pub fn parse_build(
    stdout: &str,
    stderr: &str,
    _project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // go build writes errors to stderr, not stdout.
    let combined = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };

    for line in combined.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(d) = parse_build_line(line, tool, stack) {
            diagnostics.push(d);
        }
    }

    diagnostics
}

/// Parse a single `go build` error line.
///
/// Format: `<path>:<line>:<col>: <message>`
/// Returns `None` if the line doesn't match.
fn parse_build_line(line: &str, tool: &str, stack: &str) -> Option<Diagnostic> {
    // Must have at least 3 colons: path:line:col: message
    // Walk backwards: find the last colon-separated region that looks like
    // `<digits>:<digits>: ` to locate the position of the file path.
    let bytes = line.as_bytes();
    let mut i = line.len();

    while i > 0 {
        i -= 1;
        if bytes[i] != b':' {
            continue;
        }
        // Check that bytes after this colon form `<space><message>`.
        // The pattern is `path:LINE:COL: message`.
        // We search for the `COL: ` part first.
        let after = &line[i + 1..];
        let Some(col_end) = after.find(": ") else {
            continue;
        };
        let col_str = &after[..col_end];
        let Ok(col): Result<u32, _> = col_str.trim().parse() else {
            continue;
        };
        // Now find LINE: just before the `:COL:` part.
        let before_col = &line[..i];
        let Some(line_colon) = before_col.rfind(':') else {
            continue;
        };
        let line_str = &before_col[line_colon + 1..];
        let Ok(line_num): Result<u32, _> = line_str.trim().parse() else {
            continue;
        };
        let file_path = &before_col[..line_colon];
        if file_path.is_empty() {
            continue;
        }
        // Guard: file_path must look like a real file path — ends with ".go", or
        // contains a path separator. This prevents false positives when a colon
        // appears inside the error message (e.g. `foo.go:12:5: msg at a:1: hint`).
        let looks_like_path =
            file_path.ends_with(".go") || file_path.contains('/') || file_path.contains('\\');
        if !looks_like_path {
            continue;
        }
        let message = after[col_end + 2..].trim();
        if message.is_empty() {
            continue;
        }

        // Determine severity from message prefix.
        let severity = if message.starts_with("warning:") || message.starts_with("note:") {
            Severity::Warning
        } else {
            Severity::Error
        };

        let location = crate::parse::normalize_path(file_path, std::path::Path::new(""));
        return Some(Diagnostic {
            location,
            span: Some(o8v_core::diagnostic::Span::new(line_num, col, None, None)),
            rule: None,
            severity,
            raw_severity: Some(if severity == Severity::Warning {
                "warning".to_string()
            } else {
                "error".to_string()
            }),
            message: DisplayStr::from_untrusted(message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/project")
    }

    // ─── parse_test ──────────────────────────────────────────────────────────

    /// Empty stdout → no diagnostics.
    #[test]
    fn test_empty_stdout() {
        let result = parse_test("", "", root(), "go test", "go");
        assert!(result.is_empty());
    }

    /// One test failure → one diagnostic.
    #[test]
    fn test_one_failure() {
        let input = concat!(
            r#"{"Action":"run","Test":"TestFoo","Package":"example.com/pkg"}"#,
            "\n",
            r#"{"Action":"output","Test":"TestFoo","Package":"example.com/pkg","Output":"--- FAIL: TestFoo (0.00s)\n"}"#,
            "\n",
            r#"{"Action":"fail","Test":"TestFoo","Package":"example.com/pkg","Elapsed":0.01}"#,
            "\n",
        );
        let result = parse_test(input, "", root(), "go test", "go");
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.to_string().contains("TestFoo"));
    }

    /// Package-level fail event (no Test field) is ignored.
    #[test]
    fn test_package_fail_ignored() {
        let input = concat!(
            r#"{"Action":"fail","Package":"example.com/pkg","Elapsed":0.01}"#,
            "\n",
        );
        let result = parse_test(input, "", root(), "go test", "go");
        assert!(result.is_empty());
    }

    /// Multiple test failures → multiple diagnostics in order.
    #[test]
    fn test_many_failures() {
        let input = concat!(
            r#"{"Action":"fail","Test":"TestA","Package":"pkg"}"#,
            "\n",
            r#"{"Action":"fail","Test":"TestB","Package":"pkg"}"#,
            "\n",
        );
        let result = parse_test(input, "", root(), "go test", "go");
        assert_eq!(result.len(), 2);
        assert!(result[0].message.to_string().contains("TestA"));
        assert!(result[1].message.to_string().contains("TestB"));
    }

    /// Output lines are accumulated and included in the diagnostic message.
    #[test]
    fn test_output_accumulated() {
        let input = concat!(
            r#"{"Action":"output","Test":"TestFoo","Output":"line1\n"}"#,
            "\n",
            r#"{"Action":"output","Test":"TestFoo","Output":"line2\n"}"#,
            "\n",
            r#"{"Action":"fail","Test":"TestFoo","Package":"pkg"}"#,
            "\n",
        );
        let result = parse_test(input, "", root(), "go test", "go");
        assert_eq!(result.len(), 1);
        let msg = result[0].message.to_string();
        // Headline (message) is the first non-RUN/FAIL line.
        assert!(
            msg.contains("line1"),
            "message should contain headline: {msg}"
        );
        // Full output lands in notes, not the message.
        let notes_joined = result[0].notes.join("\n");
        assert!(notes_joined.contains("line1"));
        assert!(notes_joined.contains("line2"));
    }

    /// Interrupted stream: no suite-end event. Processes what it saw.
    #[test]
    fn test_interrupted_ndjson() {
        let input = concat!(
            r#"{"Action":"fail","Test":"TestSlow","Package":"pkg"}"#,
            "\n",
        );
        let result = parse_test(input, "", root(), "go test", "go");
        assert_eq!(result.len(), 1);
    }

    /// Non-JSON lines are skipped gracefully.
    #[test]
    fn test_non_json_skipped() {
        let input = "FAIL\texample.com/pkg\t0.003s\n";
        let result = parse_test(input, "", root(), "go test", "go");
        assert!(result.is_empty());
    }

    // ─── parse_build ─────────────────────────────────────────────────────────

    /// Empty stderr → no diagnostics.
    #[test]
    fn build_empty() {
        let result = parse_build("", "", root(), "go build", "go");
        assert!(result.is_empty());
    }

    /// Standard compile error → one diagnostic.
    #[test]
    fn build_compile_error() {
        let result = parse_build(
            "",
            "./src/main.go:10:5: undefined: foo",
            root(),
            "go build",
            "go",
        );
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.to_string().contains("undefined: foo"));
        assert!(matches!(&d.span, Some(s) if s.line == 10 && s.column == 5));
    }

    /// Multiple build errors → multiple diagnostics.
    #[test]
    fn build_multiple_errors() {
        let stderr = "./a.go:1:1: error one\n./b.go:2:3: error two\n";
        let result = parse_build("", stderr, root(), "go build", "go");
        assert_eq!(result.len(), 2);
    }

    /// Lines without the pattern are ignored.
    #[test]
    fn build_noise_ignored() {
        let result = parse_build("", "FAIL\tbuild failed", root(), "go build", "go");
        assert!(result.is_empty());
    }

    // ─── parse_build_line ────────────────────────────────────────────────────

    /// Normal compile error line → correct path, line, col.
    #[test]
    fn build_line_normal() {
        let d = parse_build_line("foo.go:12:5: undefined: Bar", "go build", "go");
        let d = d.expect("expected Some diagnostic");
        assert!(matches!(&d.location, o8v_core::diagnostic::Location::File(f) if f == "foo.go"));
        let span = d.span.expect("expected span");
        assert_eq!(span.line, 12);
        assert_eq!(span.column, 5);
    }

    /// Colon inside the message must not confuse the path extractor.
    ///
    /// Input: `foo.go:12:5: undefined: time.RFC3339 at a:1: use DateTime`
    /// The backward walk would naturally match `a:1: use DateTime` first, giving a
    /// file_path of `foo.go:12:5: undefined: time.RFC3339 at a` — wrong.
    /// The path-validation guard rejects non-path-like `file_path` values and
    /// continues walking until it finds the real `foo.go:12:5` prefix.
    #[test]
    fn build_line_colon_in_message() {
        let line = "foo.go:12:5: undefined: time.RFC3339 at a:1: use DateTime";
        let d = parse_build_line(line, "go build", "go")
            .expect("expected Some diagnostic for colon-in-message input");
        assert!(
            matches!(&d.location, o8v_core::diagnostic::Location::File(f) if f == "foo.go"),
            "expected file_path 'foo.go', got {:?}",
            d.location
        );
        let span = d.span.expect("expected span");
        assert_eq!(span.line, 12);
        assert_eq!(span.column, 5);
    }

    /// A line with no file-path prefix returns None.
    #[test]
    fn build_line_no_path_prefix() {
        let d = parse_build_line("random text: 1:2: nope", "go build", "go");
        assert!(
            d.is_none(),
            "expected None for line without path, got {d:?}"
        );
    }
}
