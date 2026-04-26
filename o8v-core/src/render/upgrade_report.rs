// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeReport {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub upgraded: bool,
    pub error: Option<String>,
}

impl super::Renderable for UpgradeReport {
    fn render_plain(&self) -> Output {
        let mut lines = Vec::new();
        lines.push(format!("current: {}", self.current_version));

        if let Some(ref latest) = self.latest_version {
            lines.push(format!("latest: {}", latest));
        } else {
            lines.push("latest: unknown".to_string());
        }

        let status = if let Some(ref err) = self.error {
            format!("error: {}", err)
        } else if self.latest_version.as_deref() == Some(self.current_version.as_str()) {
            "status: up-to-date".to_string()
        } else if self.latest_version.is_some() {
            "status: upgrade-available".to_string()
        } else {
            "status: up-to-date".to_string()
        };
        lines.push(status);

        Output::new(lines.join("\n"))
    }

    fn render_json(&self) -> Output {
        let json = serde_json::json!({
            "current_version": self.current_version,
            "latest_version": self.latest_version,
            "upgraded": self.upgraded,
            "error": self.error
        });
        Output::new(json.to_string())
    }

    fn render_human(&self) -> Output {
        let mut output = String::new();

        output.push_str("Upgrade Status\n");
        output.push_str("==============\n\n");

        output.push_str(&format!("Current version: {}\n", self.current_version));

        if let Some(ref latest) = self.latest_version {
            output.push_str(&format!("Latest version:  {}\n", latest));
        } else {
            output.push_str("Latest version:  unknown\n");
        }

        output.push('\n');

        if let Some(ref err) = self.error {
            output.push_str(&format!("Status: ERROR - {}\n", err));
        } else if self.latest_version.as_deref() == Some(self.current_version.as_str()) {
            output.push_str("Status: UP-TO-DATE ✓\n");
        } else if self.latest_version.is_some() {
            output.push_str("Status: UPGRADE AVAILABLE\n");
        } else {
            output.push_str("Status: UP-TO-DATE ✓\n");
        }

        Output::new(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    fn sample_up_to_date_report() -> UpgradeReport {
        UpgradeReport {
            current_version: "1.0.0".to_string(),
            latest_version: Some("1.0.0".to_string()),
            upgraded: true,
            error: None,
        }
    }

    fn sample_upgrade_available_report() -> UpgradeReport {
        UpgradeReport {
            current_version: "1.0.0".to_string(),
            latest_version: Some("1.1.0".to_string()),
            upgraded: false,
            error: None,
        }
    }

    fn sample_error_report() -> UpgradeReport {
        UpgradeReport {
            current_version: "1.0.0".to_string(),
            latest_version: None,
            upgraded: false,
            error: Some("Failed to fetch latest version".to_string()),
        }
    }

    #[test]
    fn test_render_plain_up_to_date() {
        let report = sample_up_to_date_report();
        let output = report.render_plain();
        let content = output.as_str();

        assert!(content.contains("current: 1.0.0"));
        assert!(content.contains("latest: 1.0.0"));
        assert!(content.contains("status: up-to-date"));
    }

    #[test]
    fn test_render_plain_upgrade_available() {
        let report = sample_upgrade_available_report();
        let output = report.render_plain();
        let content = output.as_str();

        assert!(content.contains("current: 1.0.0"));
        assert!(content.contains("latest: 1.1.0"));
        assert!(content.contains("status: upgrade-available"));
    }

    #[test]
    fn test_render_plain_error() {
        let report = sample_error_report();
        let output = report.render_plain();
        let content = output.as_str();

        assert!(content.contains("current: 1.0.0"));
        assert!(content.contains("latest: unknown"));
        assert!(content.contains("error: Failed to fetch latest version"));
    }

    #[test]
    fn test_render_json_valid() {
        let report = sample_up_to_date_report();
        let output = report.render_json();
        let content = output.as_str();

        let parsed: serde_json::Value = serde_json::from_str(content).expect("JSON parse failed");
        assert_eq!(parsed["current_version"].as_str().unwrap(), "1.0.0");
        assert_eq!(parsed["latest_version"].as_str().unwrap(), "1.0.0");
        assert!(parsed["upgraded"].as_bool().unwrap());
        assert!(parsed["error"].is_null());
    }

    #[test]
    fn test_render_json_with_error() {
        let report = sample_error_report();
        let output = report.render_json();
        let content = output.as_str();

        let parsed: serde_json::Value = serde_json::from_str(content).expect("JSON parse failed");
        assert_eq!(parsed["current_version"].as_str().unwrap(), "1.0.0");
        assert!(parsed["latest_version"].is_null());
        assert!(!parsed["upgraded"].as_bool().unwrap());
        assert_eq!(
            parsed["error"].as_str().unwrap(),
            "Failed to fetch latest version"
        );
    }

    #[test]
    fn test_render_human_up_to_date() {
        let report = sample_up_to_date_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("Current version: 1.0.0"));
        assert!(content.contains("Latest version:  1.0.0"));
        assert!(content.contains("Status: UP-TO-DATE ✓"));
    }

    #[test]
    fn test_render_human_upgrade_available() {
        let report = sample_upgrade_available_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("Current version: 1.0.0"));
        assert!(content.contains("Latest version:  1.1.0"));
        assert!(content.contains("Status: UPGRADE AVAILABLE"));
    }

    #[test]
    fn test_render_human_error() {
        let report = sample_error_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("Current version: 1.0.0"));
        assert!(content.contains("Latest version:  unknown"));
        assert!(content.contains("Status: ERROR - Failed to fetch latest version"));
    }

    #[test]
    fn test_render_human_has_header() {
        let report = sample_up_to_date_report();
        let output = report.render_human();
        let content = output.as_str();

        assert!(content.contains("Upgrade Status"));
    }

    #[test]
    fn test_render_json_line_count() {
        let report = sample_up_to_date_report();
        let output = report.render_json();
        let content = output.as_str();
        let lines: Vec<&str> = content.lines().collect();

        // JSON should be single line when compacted
        assert_eq!(lines.len(), 1);
    }
    /// Real "already at latest" path: upgraded=false but versions match.
    /// This is what upgrade.rs returns when remote == current.
    fn sample_already_at_latest_report() -> UpgradeReport {
        UpgradeReport {
            current_version: "1.0.0".to_string(),
            latest_version: Some("1.0.0".to_string()),
            upgraded: false,
            error: None,
        }
    }

    #[test]
    fn test_render_human_already_at_latest_no_upgrade_available() {
        // upgraded=false + versions match => must NOT say "UPGRADE AVAILABLE"
        let report = sample_already_at_latest_report();
        let output = report.render_human();
        let content = output.as_str();
        assert!(
            !content.contains("UPGRADE AVAILABLE"),
            "render_human must not say UPGRADE AVAILABLE when already on latest version
content: {content}"
        );
        assert!(
            content.to_lowercase().contains("up to date")
                || content.to_lowercase().contains("up-to-date"),
            "render_human must say 'up to date' when already on latest version
content: {content}"
        );
    }

    #[test]
    fn test_render_plain_already_at_latest_no_upgrade_available() {
        // upgraded=false + versions match => must NOT say "upgrade-available"
        let report = sample_already_at_latest_report();
        let output = report.render_plain();
        let content = output.as_str();
        assert!(
            !content.contains("upgrade-available"),
            "render_plain must not say upgrade-available when already on latest version
content: {content}"
        );
        assert!(
            content.contains("up-to-date"),
            "render_plain must say up-to-date when already on latest version
content: {content}"
        );
    }
}
