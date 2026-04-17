// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Plain text rendering for FmtReport — token-efficient, tab-separated.

use super::output::Output;
use crate::{FmtOutcome, FmtReport};

/// Render a FmtReport as plain text.
///
/// One line per entry, tab-separated:
/// `stack_name\tstatus\ttool\tduration_ms`
///
/// Status values: ok, dirty, error, not_found
pub fn render_fmt_plain(report: &FmtReport) -> Output {
    let mut buf = String::new();

    for entry in &report.entries {
        let stack_name = entry.stack.to_string();
        let tool = &entry.tool;

        let (status, duration_ms) = match &entry.outcome {
            FmtOutcome::Ok { duration } => ("ok", duration.as_millis().to_string()),
            FmtOutcome::Dirty { duration } => ("dirty", duration.as_millis().to_string()),
            FmtOutcome::Error { .. } => ("error", "0".to_string()),
            FmtOutcome::NotFound { .. } => ("not_found", "0".to_string()),
        };

        buf.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            stack_name, status, tool, duration_ms
        ));
    }

    Output::new(buf)
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
    fn plain_tab_separated() {
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

        let output = render_fmt_plain(&report);
        let text = output.as_str();
        let lines: Vec<&str> = text.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        let rust_line: Vec<&str> = lines[0].split('\t').collect();
        assert_eq!(rust_line[0], "rust");
        assert_eq!(rust_line[1], "ok");
        assert_eq!(rust_line[2], "cargo");
        assert_eq!(rust_line[3], "220");

        let python_line: Vec<&str> = lines[1].split('\t').collect();
        assert_eq!(python_line[0], "python");
        assert_eq!(python_line[1], "ok");
        assert_eq!(python_line[2], "ruff");
        assert_eq!(python_line[3], "85");
    }

    #[test]
    fn plain_dirty_status() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Go,
                project_root: dummy_root(),
                tool: "gofmt".to_string(),
                outcome: FmtOutcome::Dirty {
                    duration: Duration::from_millis(50),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_plain(&report);
        assert!(output.as_str().contains("dirty"));
        assert!(output.as_str().contains("gofmt"));
    }

    #[test]
    fn plain_not_found_status() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Python,
                project_root: dummy_root(),
                tool: "ruff".to_string(),
                outcome: FmtOutcome::NotFound {
                    program: "ruff".to_string(),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_plain(&report);
        assert!(output.as_str().contains("not_found"));
    }

    #[test]
    fn plain_mixed_outcomes() {
        let report = FmtReport {
            entries: vec![
                FmtEntry {
                    stack: Stack::Rust,
                    project_root: dummy_root(),
                    tool: "cargo".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(100),
                    },
                },
                FmtEntry {
                    stack: Stack::Python,
                    project_root: dummy_root(),
                    tool: "ruff".to_string(),
                    outcome: FmtOutcome::Dirty {
                        duration: Duration::from_millis(80),
                    },
                },
                FmtEntry {
                    stack: Stack::Go,
                    project_root: dummy_root(),
                    tool: "gofmt".to_string(),
                    outcome: FmtOutcome::Error {
                        cause: "exit code 1".to_string(),
                        stderr: String::new(),
                    },
                },
            ],
            detection_errors: vec![],
        };

        let output = render_fmt_plain(&report);
        let lines: Vec<&str> = output.as_str().trim().split('\n').collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("ok"));
        assert!(lines[1].contains("dirty"));
        assert!(lines[2].contains("error"));
    }

    #[test]
    fn plain_empty_report() {
        let report = FmtReport {
            entries: vec![],
            detection_errors: vec![],
        };
        let output = render_fmt_plain(&report);
        assert_eq!(output.as_str(), "");
    }
}
