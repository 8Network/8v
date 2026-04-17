//! libtest JSON parser — covers `cargo test -- -Z unstable-options --format=json`.
//!
//! Nightly libtest emits NDJSON (one JSON object per line) when invoked with
//! `--format=json`. Filter for `{type:"test",event:"failed"}` to extract
//! test failures. Suite/ok/ignored/started events are silently skipped.
//!
//! This parser is called from `rust_extract` for `RunKind::Test`. On stable
//! toolchains the output is plain text (not JSON), so every line fails
//! `from_str` and is skipped via `tracing::debug!`, returning an empty vec.

use o8v_core::diagnostic::{Diagnostic, Location, ParseResult, ParseStatus, Severity};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// A single libtest NDJSON event.
#[derive(Debug, Deserialize)]
struct LibtestEvent {
    /// `"suite"` | `"test"`
    #[serde(rename = "type")]
    kind: String,
    /// `"started"` | `"ok"` | `"failed"` | `"ignored"`
    event: String,
    /// Present for `type:"test"` events.
    name: Option<String>,
    /// stdout captured during the test. Present on failed test events.
    stdout: Option<String>,
}

/// Parse libtest NDJSON output (nightly `--format=json`) into diagnostics.
///
/// Returns a `ParseResult`:
/// - `Parsed` when at least one valid JSON line was decoded (or stdout is empty).
/// - `Unparsed` when no valid JSON was found at all.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    _project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut parsed_any = false;
    let mut parsed_count = 0u32;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(event) = serde_json::from_str::<LibtestEvent>(line) else {
            tracing::debug!(line, "skipping non-JSON line in libtest output");
            continue;
        };
        parsed_any = true;
        parsed_count += 1;

        // Only process failed test events.
        if event.kind != "test" || event.event != "failed" {
            tracing::debug!(
                kind = %event.kind,
                event = %event.event,
                "skipping non-failed libtest event"
            );
            continue;
        }

        let name = event.name.unwrap_or_else(|| "<unknown>".to_string());
        let raw_stdout = event.stdout.unwrap_or_default();

        // Build the message: first non-empty line of the test's stdout, or
        // the full trimmed stdout. Fall back to a generic message if empty.
        let message = if raw_stdout.trim().is_empty() {
            format!("test `{name}` failed")
        } else {
            // Use the full stdout as the message — renderers handle multi-line.
            format!("test `{name}` failed\n{}", raw_stdout.trim_end())
        };

        diagnostics.push(Diagnostic {
            // No file path for libtest events — use the test name as identifier.
            location: Location::Absolute(name.clone()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: Some("error".to_string()),
            message: DisplayStr::from_untrusted(&message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    // Empty stdout is valid: no tests ran, or all passed.
    if stdout.trim().is_empty() {
        return ParseResult {
            diagnostics,
            status: ParseStatus::Parsed,
            parsed_items: 0,
        };
    }

    ParseResult {
        diagnostics,
        status: if parsed_any {
            ParseStatus::Parsed
        } else {
            ParseStatus::Unparsed
        },
        parsed_items: parsed_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn root() -> &'static Path {
        Path::new(ROOT)
    }

    /// Empty stdout → Parsed, zero diagnostics.
    #[test]
    fn zero_events_empty_stdout() {
        let result = parse("", "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
    }

    /// One failed test event → one diagnostic.
    #[test]
    fn one_failed_event() {
        let input = r#"{"type":"test","event":"failed","name":"my_module::my_test","stdout":"thread 'my_module::my_test' panicked at 'assertion failed', src/lib.rs:10:5\n"}"#;
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(matches!(&d.location, Location::Absolute(n) if n == "my_module::my_test"));
        assert!(d.message.to_string().contains("my_module::my_test"));
    }

    /// Multiple failures → multiple diagnostics in order.
    #[test]
    fn many_failures() {
        let input = concat!(
            r#"{"type":"test","event":"started","name":"a"}"#,
            "\n",
            r#"{"type":"test","event":"failed","name":"a","stdout":"panic at a\n"}"#,
            "\n",
            r#"{"type":"test","event":"started","name":"b"}"#,
            "\n",
            r#"{"type":"test","event":"failed","name":"b","stdout":"panic at b\n"}"#,
            "\n",
            r#"{"type":"suite","event":"failed","passed":0,"failed":2}"#,
        );
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert!(matches!(&result.diagnostics[0].location, Location::Absolute(n) if n == "a"));
        assert!(matches!(&result.diagnostics[1].location, Location::Absolute(n) if n == "b"));
    }

    /// Standard assert!() failure format in stdout is preserved in message.
    #[test]
    fn assert_panic_format() {
        // Use a plain string without JSON escape sequences so the round-trip is exact.
        let stdout_content = "thread 'my_test' panicked at assertion failed, src/lib.rs:5:5";
        let input = format!(
            r#"{{"type":"test","event":"failed","name":"my_test","stdout":"{stdout_content}"}}"#
        );
        let result = parse(&input, "", root(), "cargo test", "rust");
        assert_eq!(result.diagnostics.len(), 1);
        let msg = result.diagnostics[0].message.to_string();
        assert!(msg.contains("my_test"));
        assert!(msg.contains(stdout_content));
    }

    /// Ignored/ok/started/suite events are skipped; no diagnostics emitted.
    #[test]
    fn ignored_events_filtered() {
        let input = concat!(
            r#"{"type":"suite","event":"started","test_count":3}"#,
            "\n",
            r#"{"type":"test","event":"started","name":"a"}"#,
            "\n",
            r#"{"type":"test","event":"ok","name":"a"}"#,
            "\n",
            r#"{"type":"test","event":"ignored","name":"b"}"#,
            "\n",
            r#"{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":1}"#,
        );
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
        // All 5 lines parsed as valid JSON.
        assert_eq!(result.parsed_items, 5);
    }

    /// Interrupted NDJSON (no suite-finished line) still processes what it saw.
    #[test]
    fn interrupted_ndjson() {
        let input = concat!(
            r#"{"type":"test","event":"failed","name":"slow_test","stdout":"timed out"}"#,
            "\n",
            // Truncated mid-stream — no suite event.
        );
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    /// Plain-text output (stable toolchain) → Unparsed, no diagnostics.
    #[test]
    fn plain_text_stable_output_is_unparsed() {
        let input = "running 2 tests\ntest foo ... ok\ntest bar ... FAILED\n";
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert!(result.diagnostics.is_empty());
    }

    /// Plain-text output with only passing tests → Unparsed, no diagnostics, no panic.
    #[test]
    fn plain_text_ok_only_is_unparsed() {
        let input = "running 2 tests\ntest foo ... ok\n";
        let result = parse(input, "", root(), "cargo test", "rust");
        assert_eq!(result.status, ParseStatus::Unparsed);
        assert!(result.diagnostics.is_empty());
    }
}
