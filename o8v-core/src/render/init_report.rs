// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitReport {
    pub success: bool,
}

impl super::Renderable for InitReport {
    fn render_plain(&self) -> Output {
        if self.success {
            Output::new("ok".to_string())
        } else {
            Output::new("failed".to_string())
        }
    }

    fn render_json(&self) -> Output {
        let json = serde_json::json!({
            "success": self.success,
        });
        Output::new(json.to_string())
    }

    fn render_human(&self) -> Output {
        if self.success {
            Output::new("init: complete\n".to_string())
        } else {
            Output::new("init: failed".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    #[test]
    fn render_plain_success() {
        let report = InitReport { success: true };
        assert_eq!(report.render_plain().as_str(), "ok");
    }

    #[test]
    fn render_plain_failure() {
        let report = InitReport { success: false };
        assert_eq!(report.render_plain().as_str(), "failed");
    }

    #[test]
    fn render_json_has_fields() {
        let report = InitReport { success: true };
        let json: serde_json::Value = serde_json::from_str(report.render_json().as_str()).unwrap();
        assert_eq!(json["success"], true);
    }

    #[test]
    fn render_json_failure() {
        let report = InitReport { success: false };
        let json: serde_json::Value = serde_json::from_str(report.render_json().as_str()).unwrap();
        assert_eq!(json["success"], false);
    }

    #[test]
    fn render_human_success() {
        let report = InitReport { success: true };
        assert_eq!(report.render_human().as_str(), "init: complete\n");
    }

    #[test]
    fn render_human_failure() {
        let report = InitReport { success: false };
        assert_eq!(report.render_human().as_str(), "init: failed");
    }
}
