// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! JSON check renderer — structured output for tools, CI, and storage.

use crate::{CheckOutcome, CheckReport};
use serde::Serialize;
use std::time::Duration;

pub struct Json;

#[derive(Serialize)]
struct JsonOutput {
    results: Vec<JsonResult>,
    detection_errors: Vec<JsonDetectionError>,
    summary: JsonSummary,
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
    diagnostics: Vec<crate::Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stdout_truncated: bool,
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

fn build_results(report: &CheckReport) -> Vec<JsonResult> {
    let mut results = Vec::new();

    for result in report.results() {
        let mut checks = Vec::new();

        for entry in result.entries() {
            let ms = duration_ms(entry.duration());

            let check = match entry.outcome() {
                CheckOutcome::Passed {
                    diagnostics,
                    raw_stdout,
                    raw_stderr,
                    parse_status,
                    stdout_truncated,
                    stderr_truncated,
                } => {
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
                CheckOutcome::Failed {
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
                    diagnostics: diagnostics.clone(),
                    stdout: non_empty(raw_stdout),
                    stderr: non_empty(raw_stderr),
                    cause: None,
                    stdout_truncated: *stdout_truncated,
                    stderr_truncated: *stderr_truncated,
                },
                CheckOutcome::Error {
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

fn build_detection_errors(report: &CheckReport) -> Vec<JsonDetectionError> {
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

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn duration_ms(d: Duration) -> u64 {
    d.as_secs()
        .saturating_mul(1000)
        .saturating_add(u64::from(d.subsec_millis()))
}
