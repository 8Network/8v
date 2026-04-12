// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use crate::process_report::ProcessReport;
use crate::render::RenderConfig;

pub struct TestReport {
    pub process: ProcessReport,
    pub stack: String,
    pub detection_errors: Vec<String>,
    pub render_config: RenderConfig,
}

impl super::Renderable for TestReport {
    fn render_plain(&self) -> Output {
        let p = &self.process;
        let mut buf = String::new();

        for e in &self.detection_errors {
            buf.push_str(&format!("warning: {e}\n"));
        }

        buf.push_str(&format!("$ {}\n", p.command));
        buf.push_str(&format!("exit: {}\n", p.exit_label));
        buf.push_str(&format!("duration: {}\n", p.duration_display));
        super::run_report::render_process_output(&mut buf, p, &self.render_config);
        Output::new(buf)
    }

    fn render_json(&self) -> Output {
        let p = &self.process;
        let json = serde_json::json!({
            "command": p.command,
            "exit_code": p.exit_code,
            "stdout": p.stdout.trim(),
            "stderr": p.stderr.trim(),
            "duration_ms": p.duration.as_millis() as u64,
            "truncated": {
                "stdout": p.stdout_truncated,
                "stderr": p.stderr_truncated,
            },
            "stack": self.stack,
            "detection_errors": self.detection_errors,
        });
        let s = match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }

    fn render_human(&self) -> Output {
        self.render_plain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;
    use std::time::Duration;

    fn sample() -> TestReport {
        TestReport {
            process: ProcessReport {
                command: "cargo test".to_string(),
                exit_code: 0,
                success: true,
                exit_label: "0 (success)".to_string(),
                duration: Duration::from_millis(1234),
                duration_display: "1.23s".to_string(),
                stdout: "test result: ok. 5 passed".to_string(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            stack: "rust".to_string(),
            detection_errors: vec![],
            render_config: RenderConfig::default(),
        }
    }

    #[test]
    fn plain_format() {
        let out = sample().render_plain();
        let text = out.as_str();
        assert!(text.starts_with("$ cargo test\n"));
        assert!(text.contains("exit: 0 (success)\n"));
        assert!(text.contains("duration: 1.23s\n"));
    }

    #[test]
    fn plain_with_detection_errors() {
        let mut r = sample();
        r.detection_errors = vec!["cargo metadata failed".to_string()];
        let out = r.render_plain();
        assert!(out.as_str().starts_with("warning: cargo metadata failed\n"));
    }

    #[test]
    fn json_valid() {
        let out = sample().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["command"], "cargo test");
        assert_eq!(v["exit_code"], 0);
    }
}
