// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use crate::diagnostic::Diagnostic;
use crate::process_report::ProcessReport;
use crate::render::RenderConfig;

pub struct BuildReport {
    pub process: ProcessReport,
    pub stack: String,
    /// Short project name (directory basename).
    pub name: String,
    pub detection_errors: Vec<String>,
    pub render_config: RenderConfig,
    /// Structured errors extracted from build output (errors-first rendering).
    pub errors: Vec<Diagnostic>,
}

impl super::Renderable for BuildReport {
    fn render_plain(&self) -> Output {
        let p = &self.process;
        let mut buf = String::new();

        for e in &self.detection_errors {
            buf.push_str(&format!("warning: {e}\n"));
        }

        // Progressive header: "{name} {stack}"
        buf.push_str(&format!("{} {}\n", self.name, self.stack));

        if p.success {
            buf.push_str(&format!("build success {}\n", p.duration_display));
        } else {
            buf.push_str(&format!("build failed {}\n", p.duration_display));

            // Show structured diagnostics when available.
            if !self.errors.is_empty() {
                const MAX_ERRORS: usize = 10;
                for diag in self.errors.iter().take(MAX_ERRORS) {
                    let file = match &diag.location {
                        crate::diagnostic::Location::File(f)
                        | crate::diagnostic::Location::Absolute(f) => f.as_str(),
                    };
                    let location = match diag.span.as_ref().map(|s| s.line) {
                        Some(l) => format!("{}:{}: ", file, l),
                        None => format!("{}: ", file),
                    };
                    buf.push_str(&format!("  {}{}\n", location, diag.message));
                }
                if self.errors.len() > MAX_ERRORS {
                    let remaining = self.errors.len() - MAX_ERRORS;
                    buf.push_str(&format!("  ... and {} more\n", remaining));
                }
            } else if !p.stderr.trim().is_empty() {
                // Fallback: show raw stderr when no structured diagnostics.
                buf.push_str(&format!("{}\n", p.stderr.trim()));
            }
        }

        Output::new(buf)
    }

    fn render_json(&self) -> Output {
        let p = &self.process;
        let errors_json = serde_json::to_value(&self.errors)
            .expect("BUG: Vec<Diagnostic> is always serializable");
        let json = serde_json::json!({
            "name": self.name,
            "stack": self.stack,
            "success": p.success,
            "exit_code": p.exit_code,
            "duration_ms": p.duration.as_millis() as u64,
            "command": p.command,
            "errors": errors_json,
            "detection_errors": self.detection_errors,
            "stdout": p.stdout,
            "stderr": p.stderr,
            "truncated": {
                "stdout": p.stdout_truncated,
                "stderr": p.stderr_truncated,
            },
        });
        let s = match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;
    use std::time::Duration;

    fn sample() -> BuildReport {
        BuildReport {
            process: ProcessReport {
                command: "cargo build".to_string(),
                exit_code: 0,
                success: true,
                exit_label: "0 (success)".to_string(),
                duration: Duration::from_millis(162),
                duration_display: "162ms".to_string(),
                stdout: String::new(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            stack: "rust".to_string(),
            name: "myproject".to_string(),
            detection_errors: vec![],
            render_config: RenderConfig::default(),
            errors: vec![],
        }
    }

    fn sample_failure() -> BuildReport {
        BuildReport {
            process: ProcessReport {
                command: "cargo build".to_string(),
                exit_code: 1,
                success: false,
                exit_label: "1".to_string(),
                duration: Duration::from_millis(500),
                duration_display: "500ms".to_string(),
                stdout: String::new(),
                stderr: "error[E0308]: type mismatch".to_string(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            stack: "rust".to_string(),
            name: "myproject".to_string(),
            detection_errors: vec![],
            render_config: RenderConfig::default(),
            errors: vec![],
        }
    }

    /// Regression test: pre-fix code returned "$ cargo build\n..." — this must fail on pre-fix.
    #[test]
    fn plain_success_progressive_format() {
        let out = sample().render_plain();
        let text = out.as_str();
        // New format: "{name} {stack}\nbuild success {duration}\n"
        assert!(
            text.starts_with("myproject rust\n"),
            "expected progressive header 'myproject rust\\n', got: {text:?}"
        );
        assert!(
            text.contains("build success 162ms"),
            "expected 'build success 162ms', got: {text:?}"
        );
        // Must NOT contain old verbose format markers.
        assert!(
            !text.contains("$ cargo build"),
            "must not contain raw command line: {text:?}"
        );
        assert!(
            !text.contains("exit:"),
            "must not contain 'exit:' in progressive output: {text:?}"
        );
    }

    /// Regression test: failure must show "build failed {ms}" not verbose exit label.
    #[test]
    fn plain_failure_progressive_format() {
        let out = sample_failure().render_plain();
        let text = out.as_str();
        assert!(
            text.starts_with("myproject rust\n"),
            "expected 'myproject rust\\n', got: {text:?}"
        );
        assert!(
            text.contains("build failed 500ms"),
            "expected 'build failed 500ms', got: {text:?}"
        );
        // Falls back to raw stderr when no structured diagnostics.
        assert!(
            text.contains("error[E0308]: type mismatch"),
            "expected stderr fallback, got: {text:?}"
        );
    }

    /// Regression test: failure with structured diagnostics shows file:line + message.
    #[test]
    fn plain_failure_with_diagnostics() {
        use crate::diagnostic::{Location, Severity, Span};
        use crate::display_str::DisplayStr;
        let mut r = sample_failure();
        r.errors = vec![Diagnostic {
            location: Location::File("src/main.rs".to_string()),
            span: Some(Span::new(42, 1, None, None)),
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_untrusted("type mismatch"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "cargo".to_string(),
            stack: "rust".to_string(),
        }];
        let out = r.render_plain();
        let text = out.as_str();
        assert!(
            text.contains("src/main.rs:42: type mismatch"),
            "expected 'src/main.rs:42: type mismatch', got: {text:?}"
        );
    }

    #[test]
    fn plain_with_detection_errors() {
        let mut r = sample();
        r.detection_errors = vec!["multiple roots".to_string()];
        let out = r.render_plain();
        assert!(out.as_str().starts_with("warning: multiple roots\n"));
    }

    #[test]
    fn json_has_name_and_stack() {
        let out = sample().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["name"], "myproject");
        assert_eq!(v["stack"], "rust");
        assert_eq!(v["success"], true);
        assert_eq!(v["exit_code"], 0);
        // stdout and stderr must be present as strings (even if empty).
        assert!(
            v.get("stdout").and_then(|s| s.as_str()).is_some(),
            "stdout must be a string in JSON; got: {v}"
        );
        assert!(
            v.get("stderr").and_then(|s| s.as_str()).is_some(),
            "stderr must be a string in JSON; got: {v}"
        );
    }
}
