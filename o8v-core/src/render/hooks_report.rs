// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksReport {
    pub hooks: Vec<HookEntry>,
    /// Exit code from the hook execution (0 = success, non-zero = failure/block).
    /// Only meaningful when hooks is empty (i.e., this is an execution report, not a list).
    pub exit_code: u8,
    /// True if exit_code == 0.
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    pub event: String,   // e.g. "pre-check", "post-check"
    pub command: String, // the shell command
    pub enabled: bool,
}

impl super::Renderable for HooksReport {
    fn render_plain(&self) -> Output {
        let mut lines = Vec::new();
        for entry in &self.hooks {
            let enabled_str = if entry.enabled { "true" } else { "false" };
            lines.push(format!(
                "{}\t{}\t{}",
                entry.event, entry.command, enabled_str
            ));
        }
        Output::new(lines.join("\n"))
    }

    fn render_json(&self) -> Output {
        let json = serde_json::json!({
            "hooks": self.hooks
        });
        Output::new(json.to_string())
    }

    fn render_human(&self) -> Output {
        if self.hooks.is_empty() {
            return Output::new("No hooks configured".to_string());
        }

        // Calculate column widths for alignment
        let max_event_len = self.hooks.iter().map(|e| e.event.len()).max().unwrap_or(5);
        let max_command_len = self
            .hooks
            .iter()
            .map(|e| e.command.len())
            .max()
            .unwrap_or(7);

        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "{:<width_event$}  {:<width_command$}  {}\n",
            "EVENT",
            "COMMAND",
            "ENABLED",
            width_event = max_event_len.max(5),
            width_command = max_command_len.max(7)
        ));

        // Separator
        output.push_str(&format!(
            "{:-<width_event$}  {:-<width_command$}  {:-<7}\n",
            "",
            "",
            "",
            width_event = max_event_len.max(5),
            width_command = max_command_len.max(7)
        ));

        // Rows
        for entry in &self.hooks {
            let enabled_symbol = if entry.enabled { "✓" } else { "✗" };
            output.push_str(&format!(
                "{:<width_event$}  {:<width_command$}  {}\n",
                entry.event,
                entry.command,
                enabled_symbol,
                width_event = max_event_len.max(5),
                width_command = max_command_len.max(7)
            ));
        }

        Output::new(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    fn sample_report() -> HooksReport {
        HooksReport {
            hooks: vec![
                HookEntry {
                    event: "pre-check".to_string(),
                    command: "cargo fmt --check".to_string(),
                    enabled: true,
                },
                HookEntry {
                    event: "post-check".to_string(),
                    command: "cargo clippy".to_string(),
                    enabled: false,
                },
            ],
            exit_code: 0,
            success: true,
        }
    }

    #[test]
    fn test_render_plain_format() {
        let report = sample_report();
        let output = report.render_plain();
        let content = output.as_str();

        assert!(content.contains("pre-check\tcargo fmt --check\ttrue"));
        assert!(content.contains("post-check\tcargo clippy\tfalse"));
    }

    #[test]
    fn test_render_plain_line_count() {
        let report = sample_report();
        let output = report.render_plain();
        let lines: Vec<&str> = output.as_str().lines().collect();

        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_render_json_valid() {
        let report = sample_report();
        let output = report.render_json();
        let content = output.as_str();

        let parsed: serde_json::Value = serde_json::from_str(content).expect("JSON parse failed");
        assert!(parsed.get("hooks").is_some());
        assert_eq!(parsed["hooks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_render_json_contains_fields() {
        let report = sample_report();
        let output = report.render_json();
        let content = output.as_str();

        assert!(content.contains("pre-check"));
        assert!(content.contains("cargo fmt --check"));
        assert!(content.contains("true"));
        assert!(content.contains("post-check"));
        assert!(content.contains("false"));
    }

    #[test]
    fn test_render_human_has_header() {
        let report = sample_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("EVENT"));
        assert!(content.contains("COMMAND"));
        assert!(content.contains("ENABLED"));
    }

    #[test]
    fn test_render_human_has_symbols() {
        let report = sample_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("✓"));
        assert!(content.contains("✗"));
    }

    #[test]
    fn test_render_human_empty_hooks() {
        let report = HooksReport {
            hooks: vec![],
            exit_code: 0,
            success: true,
        };
        let output = report.render_human();
        let content = output.as_str();

        assert_eq!(content, "No hooks configured");
    }

    #[test]
    fn test_render_human_alignment() {
        let report = HooksReport {
            hooks: vec![
                HookEntry {
                    event: "a".to_string(),
                    command: "short".to_string(),
                    enabled: true,
                },
                HookEntry {
                    event: "very-long-event".to_string(),
                    command: "this-is-a-very-long-command-string".to_string(),
                    enabled: false,
                },
            ],
            exit_code: 0,
            success: true,
        };
        let output = report.render_human();
        let content = output.as_str();

        // Should not panic and should contain both entries
        assert!(content.contains("very-long-event"));
        assert!(content.contains("this-is-a-very-long-command-string"));
    }
}
