// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! JSON rendering logic for CheckReport.
//!
//! Uses typed structs with `#[derive(Serialize)]` — the compiler guarantees
//! the output shape. No `serde_json::json!` macros. No untyped construction.

use serde::Serialize;
use std::time::Duration;

// ─── Typed output structs ─────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonOutput {
    results: Vec<JsonResult>,
    detection_errors: Vec<JsonDetectionError>,
    summary: JsonSummary,
    /// Delta compared to previous run. Omitted when no previous data is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<JsonDelta>,
}

#[derive(Serialize)]
struct JsonDelta {
    new: usize,
    fixed: usize,
    unchanged: usize,
}

#[derive(Serialize)]
struct JsonDetectionError {
    kind: &'static str,
    path: String,
    message: String,
}

#[derive(Serialize)]
struct JsonResult {
    project: String,
    stack: String,
    path: String,
    checks: Vec<JsonCheck>,
}

#[derive(Serialize)]
struct JsonCheck {
    name: String,
    outcome: String,
    ms: u64,
    parse_status: String,
    /// Always present — empty array, not absent. Schema must be predictable.
    diagnostics: Vec<crate::Diagnostic>,
    /// Raw stdout from the tool. Omitted when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    /// Raw stderr from the tool. Omitted when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    /// Error cause string. Only present for Error outcomes.
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<String>,
    /// True if stdout was truncated at the capture limit.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stdout_truncated: bool,
    /// True if stderr was truncated at the capture limit.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stderr_truncated: bool,
}

#[derive(Serialize)]
struct JsonSummary {
    success: bool,
    passed: u32,
    failed: u32,
    errors: u32,
    detection_errors: usize,
    ms: u64,
}

// ─── Public render function ───────────────────────────────────────────────

/// Render a `CheckReport` as pretty-printed JSON, returning an `Output`.
pub(in crate::render) fn render_check_json(
    report: &crate::CheckReport,
    _config: &super::RenderConfig,
) -> super::Output {
    let s = super::Summary::from_report(report);
    let summary = JsonSummary {
        success: s.success,
        passed: s.passed,
        failed: s.failed,
        errors: s.errors,
        detection_errors: s.detection_errors,
        ms: duration_ms(s.total_duration),
    };

    let results = build_results(report);
    let detection_errors = build_detection_errors(report);
    let delta = report.delta.as_ref().map(|d| JsonDelta {
        new: d.new,
        fixed: d.fixed,
        unchanged: d.unchanged,
    });

    let output = JsonOutput {
        results,
        detection_errors,
        summary,
        delta,
    };

    let mut json = serde_json::to_string_pretty(&output)
        .expect("BUG: JsonOutput contains only serializable types");
    json.push('\n');

    super::Output::new(json)
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn build_results(report: &crate::CheckReport) -> Vec<JsonResult> {
    let mut results = Vec::new();

    for result in report.results() {
        let mut checks = Vec::new();

        for entry in result.entries() {
            let ms = duration_ms(entry.duration());

            let check = match entry.outcome() {
                crate::CheckOutcome::Passed {
                    diagnostics,
                    raw_stdout,
                    raw_stderr,
                    parse_status,
                    stdout_truncated,
                    stderr_truncated,
                } => {
                    // Only include raw output when verification was weakened
                    // (truncated or unparsed). Clean passes don't need megabytes
                    // of compiler-artifact JSON in the output.
                    let noteworthy = *stdout_truncated
                        || *stderr_truncated
                        || *parse_status == crate::ParseStatus::Unparsed;
                    JsonCheck {
                        name: entry.name().to_string(),
                        outcome: "passed".to_string(),
                        ms,
                        parse_status: parse_status.to_string(),
                        diagnostics: diagnostics.clone(),
                        stdout: if noteworthy {
                            non_empty(raw_stdout)
                        } else {
                            None
                        },
                        stderr: if noteworthy {
                            non_empty(raw_stderr)
                        } else {
                            None
                        },
                        cause: None,
                        stdout_truncated: *stdout_truncated,
                        stderr_truncated: *stderr_truncated,
                    }
                }
                crate::CheckOutcome::Failed {
                    diagnostics,
                    raw_stdout,
                    raw_stderr,
                    parse_status,
                    stdout_truncated,
                    stderr_truncated,
                    ..
                } => JsonCheck {
                    name: entry.name().to_string(),
                    outcome: "failed".to_string(),
                    ms,
                    parse_status: parse_status.to_string(),
                    // Clone is intentional — same rationale as Passed arm above.
                    diagnostics: diagnostics.clone(),
                    stdout: non_empty(raw_stdout),
                    stderr: non_empty(raw_stderr),
                    cause: None,
                    stdout_truncated: *stdout_truncated,
                    stderr_truncated: *stderr_truncated,
                },
                crate::CheckOutcome::Error {
                    kind,
                    cause,
                    raw_stdout,
                    raw_stderr,
                } => {
                    let ps = match kind {
                        crate::ErrorKind::Runtime => crate::ParseStatus::None,
                        crate::ErrorKind::Verification => crate::ParseStatus::Unparsed,
                    };
                    JsonCheck {
                        name: entry.name().to_string(),
                        outcome: "error".to_string(),
                        ms,
                        parse_status: ps.to_string(),
                        diagnostics: vec![],
                        stdout: non_empty(raw_stdout),
                        stderr: non_empty(raw_stderr),
                        cause: Some(cause.clone()),
                        stdout_truncated: false,
                        stderr_truncated: false,
                    }
                }
                #[allow(unreachable_patterns)]
                other => {
                    tracing::warn!(
                        "unknown CheckOutcome variant for '{}': {other:?}",
                        entry.name()
                    );
                    JsonCheck {
                        name: entry.name().to_string(),
                        outcome: "unknown".to_string(),
                        ms,
                        parse_status: "none".to_string(),
                        diagnostics: vec![],
                        stdout: None,
                        stderr: None,
                        cause: Some(format!("Unknown outcome variant: {other:?}")),
                        stdout_truncated: false,
                        stderr_truncated: false,
                    }
                }
            };

            checks.push(check);
        }

        results.push(JsonResult {
            project: super::sanitize_for_display(result.project_name()),
            stack: result.stack().to_string(),
            path: super::sanitize_for_display(&result.project_path().to_string()),
            checks,
        });
    }

    results
}

fn build_detection_errors(report: &crate::CheckReport) -> Vec<JsonDetectionError> {
    report
        .detection_errors()
        .iter()
        .map(|e| {
            let kind = e.kind();
            let path = e.path().display().to_string();
            JsonDetectionError {
                kind,
                path: super::sanitize_for_display(&path),
                message: super::sanitize_for_display(&e.to_string()),
            }
        })
        .collect()
}

/// Return `Some(clone)` if non-empty, `None` otherwise.
fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Duration as milliseconds in `u64`, without going through `u128`.
fn duration_ms(d: Duration) -> u64 {
    d.as_secs()
        .saturating_mul(1000)
        .saturating_add(u64::from(d.subsec_millis()))
}

#[cfg(test)]
mod security_tests {
    #[test]
    fn security_json_escaping_quotes() {
        // Verify serde_json properly escapes special characters like quotes.
        // This test ensures double quotes in fields don't break JSON structure.
        let test_val = serde_json::json!({
            "message": r#"error: "quoted" value"#,
            "path": r#"C:\path\to\file"#,
        });
        let json_str = serde_json::to_string(&test_val).unwrap();
        // Should be valid JSON with escaped quotes
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn security_json_escaping_control_chars() {
        // Serde_json should properly handle/escape control characters.
        let test_val = serde_json::json!({
            "message": "line1\nline2\x00null\x1f"
        });
        let json_str = serde_json::to_string(&test_val).unwrap();
        // Should serialize successfully
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn security_json_escaping_unicode() {
        // Unicode and emoji should serialize correctly in JSON.
        let test_val = serde_json::json!({
            "message": "error 🔥 中文 עברית",
            "path": "/project/文件.rs"
        });
        let json_str = serde_json::to_string_pretty(&test_val).unwrap();
        // Must be valid UTF-8 and valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn security_json_escaping_backslash() {
        // Backslashes in strings should be properly escaped.
        let test_val = serde_json::json!({
            "path": r#"C:\Users\test\file.rs"#,
            "message": "regex: \\d+ matches digits"
        });
        let json_str = serde_json::to_string(&test_val).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn security_json_huge_message() {
        // A very large message (10MB) should serialize without panic.
        // This tests DoS resistance for memory exhaustion attacks.
        let huge_msg = "x".repeat(10_000_000);
        let test_val = serde_json::json!({
            "message": &huge_msg,
        });
        let json_str = serde_json::to_string(&test_val).unwrap();
        assert!(!json_str.is_empty());
        // Verify it parses back
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }
}
