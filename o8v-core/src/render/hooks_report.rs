// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksReport {
    pub exit_code: u8,
    pub success: bool,
}

impl super::Renderable for HooksReport {
    fn render_plain(&self) -> Output {
        if self.success {
            Output::new("ok\n".to_string())
        } else {
            Output::new(format!("failed (exit {})\n", self.exit_code))
        }
    }

    fn render_json(&self) -> Output {
        let json = serde_json::json!({
            "exit_code": self.exit_code,
            "success": self.success,
        });
        Output::new(json.to_string())
    }

    fn render_human(&self) -> Output {
        if self.success {
            Output::new("hooks: passed\n".to_string())
        } else {
            Output::new(format!("hooks: failed (exit {})\n", self.exit_code))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    #[test]
    fn render_plain_success() {
        let report = HooksReport {
            exit_code: 0,
            success: true,
        };
        assert_eq!(report.render_plain().as_str(), "ok\n");
    }

    #[test]
    fn render_plain_failure() {
        let report = HooksReport {
            exit_code: 1,
            success: false,
        };
        assert_eq!(report.render_plain().as_str(), "failed (exit 1)\n");
    }

    #[test]
    fn render_json_has_fields() {
        let report = HooksReport {
            exit_code: 0,
            success: true,
        };
        let json: serde_json::Value = serde_json::from_str(report.render_json().as_str()).unwrap();
        assert_eq!(json["exit_code"], 0);
        assert_eq!(json["success"], true);
    }

    #[test]
    fn render_human_success() {
        let report = HooksReport {
            exit_code: 0,
            success: true,
        };
        assert_eq!(report.render_human().as_str(), "hooks: passed\n");
    }

    #[test]
    fn render_human_failure() {
        let report = HooksReport {
            exit_code: 2,
            success: false,
        };
        assert_eq!(report.render_human().as_str(), "hooks: failed (exit 2)\n");
    }
}
