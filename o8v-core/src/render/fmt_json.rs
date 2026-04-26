// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! JSON rendering for FmtReport — structured for tools and CI.

use super::output::Output;
use crate::{FmtOutcome, FmtReport};

/// Render a FmtReport as JSON.
///
/// Structure:
/// ```json
/// {
///   "stacks": [
///     { "name": "rust", "status": "ok", "tool": "cargo", "timing_ms": 220 }
///   ]
/// }
/// ```
pub fn render_fmt_json(report: &FmtReport) -> Output {
    let mut stacks = Vec::new();

    for entry in &report.entries {
        let stack_name = entry.stack.to_string();
        let tool = &entry.tool;

        let (status, timing_ms) = match &entry.outcome {
            FmtOutcome::Ok { duration } => ("ok", duration.as_millis() as u64),
            FmtOutcome::Dirty { duration } => ("dirty", duration.as_millis() as u64),
            FmtOutcome::Error { .. } => ("error", 0),
            FmtOutcome::NotFound { .. } => ("not_found", 0),
        };

        stacks.push(serde_json::json!({
            "name": stack_name,
            "status": status,
            "tool": tool,
            "timing_ms": timing_ms
        }));
    }

    let root = if stacks.is_empty() {
        serde_json::json!({ "stacks": stacks, "reason": "no_stacks" })
    } else {
        serde_json::json!({ "stacks": stacks })
    };
    // Use match instead of unwrap_or_else per project rules
    let json_str = match serde_json::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
    };
    Output::new(format!("{}\n", json_str))
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
    fn json_structure() {
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

        let output = render_fmt_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(output.as_str()).unwrap();
        assert!(parsed["stacks"].is_array());

        let stacks = parsed["stacks"].as_array().unwrap();
        assert_eq!(stacks.len(), 2);
        assert_eq!(stacks[0]["name"], "rust");
        assert_eq!(stacks[0]["status"], "ok");
        assert_eq!(stacks[0]["tool"], "cargo");
        assert_eq!(stacks[0]["timing_ms"], 220);
        assert_eq!(stacks[1]["name"], "python");
        assert_eq!(stacks[1]["status"], "ok");
        assert_eq!(stacks[1]["tool"], "ruff");
        assert_eq!(stacks[1]["timing_ms"], 85);
    }

    #[test]
    fn json_dirty_status() {
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

        let output = render_fmt_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(output.as_str()).unwrap();
        assert_eq!(parsed["stacks"][0]["status"], "dirty");
    }

    #[test]
    fn json_error_status() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Python,
                project_root: dummy_root(),
                tool: "ruff".to_string(),
                outcome: FmtOutcome::Error {
                    cause: "exit code 1".to_string(),
                    stderr: String::new(),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(output.as_str()).unwrap();
        assert_eq!(parsed["stacks"][0]["status"], "error");
        assert_eq!(parsed["stacks"][0]["timing_ms"], 0);
    }

    #[test]
    fn json_not_found_status() {
        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::DotNet,
                project_root: dummy_root(),
                tool: "dotnet".to_string(),
                outcome: FmtOutcome::NotFound {
                    program: "dotnet".to_string(),
                },
            }],
            detection_errors: vec![],
        };

        let output = render_fmt_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(output.as_str()).unwrap();
        assert_eq!(parsed["stacks"][0]["status"], "not_found");
        assert_eq!(parsed["stacks"][0]["timing_ms"], 0);
    }

    #[test]
    fn json_empty_report() {
        let report = FmtReport {
            entries: vec![],
            detection_errors: vec![],
        };
        let output = render_fmt_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(output.as_str()).unwrap();
        assert!(parsed["stacks"].as_array().unwrap().is_empty());
        assert_eq!(parsed["reason"].as_str(), Some("no_stacks"));
    }
}
