// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Human-friendly rendering for FmtReport — colored, aligned, terminal output.

use super::output::Output;
use super::RenderConfig;
use crate::{FmtOutcome, FmtReport};

// ANSI codes
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

/// Render a FmtReport for terminal display.
pub fn render_fmt_human(report: &FmtReport, config: &RenderConfig) -> Output {
    let c = config.color;
    let mut buf = String::new();

    buf.push('\n');
    buf.push_str(&format!("  {}8v fmt{}\n", color(c, BOLD), color(c, RESET)));
    buf.push('\n');

    // Compute display data for alignment
    let mut lines = Vec::new();

    for entry in &report.entries {
        let stack_name = entry.stack.to_string();
        let (symbol, detail) = match &entry.outcome {
            FmtOutcome::Ok { duration } => (
                format!("{}✓{}", color(c, GREEN), color(c, RESET)),
                format!("{}ms", duration.as_millis()),
            ),
            FmtOutcome::Dirty { duration } => (
                format!("{}✗{}", color(c, RED), color(c, RESET)),
                format!("{}ms", duration.as_millis()),
            ),
            FmtOutcome::Error { cause, .. } => (
                format!("{}✗{}", color(c, RED), color(c, RESET)),
                super::sanitize_for_display(cause),
            ),
            FmtOutcome::NotFound { program } => (
                format!("{}✗{}", color(c, RED), color(c, RESET)),
                format!("{} not found", program),
            ),
        };

        lines.push((stack_name, symbol, detail));
    }

    // Detection errors
    for error in &report.detection_errors {
        let msg = super::sanitize_for_display(&error.to_string());
        buf.push_str(&format!(
            "  {}error:{} {msg}\n",
            color(c, RED),
            color(c, RESET)
        ));
    }

    // Aligned entries
    let max_width = lines.iter().map(|(s, _, _)| s.len()).max().unwrap_or(0);
    for (stack_name, symbol, detail) in &lines {
        let padding = " ".repeat(max_width - stack_name.len());
        buf.push_str(&format!(
            "    {}{} {}  {}\n",
            stack_name, padding, symbol, detail
        ));
    }

    buf.push('\n');

    // Summary
    let ok_count = report.entries.iter().filter(|e| e.outcome.is_ok()).count();
    let total_count = report.entries.len();
    let dirty_count = total_count - ok_count;

    if dirty_count == 0 && report.detection_errors.is_empty() {
        buf.push_str(&format!(
            "    {}✓{} {} stacks formatted\n",
            color(c, GREEN),
            color(c, RESET),
            total_count
        ));
    } else {
        let error_count = report.detection_errors.len();
        let total_issues = dirty_count + error_count;
        buf.push_str(&format!(
            "    {}✗{} {} error{}\n",
            color(c, RED),
            color(c, RESET),
            total_issues,
            if total_issues == 1 { "" } else { "s" }
        ));
    }

    buf.push('\n');

    Output::new(buf)
}

fn color(enabled: bool, code: &str) -> &str {
    if enabled {
        code
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ProjectRoot, Stack};
    use crate::FmtEntry;
    use std::time::Duration;

    fn dummy_root() -> ProjectRoot {
        let dir = tempfile::tempdir().unwrap();
        ProjectRoot::new(dir.path()).unwrap()
    }

    #[test]
    fn human_ok_entries() {
        let report = FmtReport {
            entries: vec![
                FmtEntry {
                    stack: Stack::Rust,
                    project_root: dummy_root(),
                    tool: "cargo".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(220),
                    },
                },
                FmtEntry {
                    stack: Stack::Python,
                    project_root: dummy_root(),
                    tool: "ruff".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(85),
                    },
                },
            ],
            detection_errors: vec![],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        let text = output.as_str();
        assert!(text.contains("rust"));
        assert!(text.contains("python"));
        assert!(text.contains("220ms"));
        assert!(text.contains("85ms"));
        assert!(text.contains("2 stacks formatted"));
    }

    #[test]
    fn human_dirty_entry() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Python,
                project_root: dummy_root(),
                tool: "ruff".to_string(),
                outcome: FmtOutcome::Dirty {
                    duration: Duration::from_millis(100),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        assert!(output.as_str().contains("1 error"));
    }

    #[test]
    fn human_error_entry() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Rust,
                project_root: dummy_root(),
                tool: "cargo".to_string(),
                outcome: FmtOutcome::Error {
                    cause: "permission denied".to_string(),
                    stderr: String::new(),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        let text = output.as_str();
        assert!(text.contains("permission denied"));
        assert!(text.contains("rust"));
    }

    #[test]
    fn human_not_found_entry() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Go,
                project_root: dummy_root(),
                tool: "gofmt".to_string(),
                outcome: FmtOutcome::NotFound {
                    program: "gofmt".to_string(),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        assert!(output.as_str().contains("gofmt not found"));
    }

    #[test]
    fn human_detection_errors() {
        let report = FmtReport {
            entries: vec![],
            detection_errors: vec![crate::project::DetectError::ManifestInvalid {
                path: std::path::PathBuf::from("/some/path"),
                cause: "malformed JSON".into(),
            }],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        let text = output.as_str();
        assert!(text.contains("error"));
        assert!(text.contains("malformed JSON"));
    }

    #[test]
    fn human_multiple_errors_plural() {
        let report = FmtReport {
            entries: vec![
                FmtEntry {
                    stack: Stack::Rust,
                    project_root: dummy_root(),
                    tool: "cargo".to_string(),
                    outcome: FmtOutcome::Dirty {
                        duration: Duration::from_millis(100),
                    },
                },
                FmtEntry {
                    stack: Stack::Python,
                    project_root: dummy_root(),
                    tool: "ruff".to_string(),
                    outcome: FmtOutcome::Error {
                        cause: "failed".to_string(),
                        stderr: String::new(),
                    },
                },
            ],
            detection_errors: vec![],
        };

        let output = render_fmt_human(&report, &RenderConfig::default());
        assert!(output.as_str().contains("2 errors"));
    }
}
