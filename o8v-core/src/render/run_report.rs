// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Report type for `8v run` — arbitrary command execution.

use super::output::Output;
use crate::process_report::ProcessReport;

/// Result of running an arbitrary command.
pub struct RunReport {
    pub process: ProcessReport,
}

impl super::Renderable for RunReport {
    fn render_plain(&self) -> Output {
        let p = &self.process;
        let mut buf = String::new();
        buf.push_str(&format!("$ {}\n", p.command));
        buf.push_str(&format!("exit: {}\n", p.exit_label));
        buf.push_str(&format!("duration: {}\n", p.duration_display));
        render_process_output(&mut buf, p);
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
        });
        let s = match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }

    fn render_human(&self) -> Output {
        // Human gets the same output as plain for run.
        self.render_plain()
    }
}

/// Render stdout/stderr sections matching the format_process_output convention.
pub fn render_process_output(buf: &mut String, p: &ProcessReport) {
    let stdout = p.stdout.trim();
    if !stdout.is_empty() {
        buf.push_str("\nstdout:\n");
        buf.push_str(stdout);
        buf.push('\n');
        if p.stdout_truncated {
            buf.push_str(o8v_process::TRUNCATION_MARKER);
            buf.push('\n');
        }
    }

    let stderr = p.stderr.trim();
    if !stderr.is_empty() {
        buf.push_str("\nstderr:\n");
        buf.push_str(stderr);
        buf.push('\n');
        if p.stderr_truncated {
            buf.push_str(o8v_process::TRUNCATION_MARKER);
            buf.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;
    use std::time::Duration;

    fn sample() -> RunReport {
        RunReport {
            process: ProcessReport {
                command: "echo hello".to_string(),
                exit_code: 0,
                success: true,
                exit_label: "0 (success)".to_string(),
                duration: Duration::from_millis(42),
                duration_display: "42ms".to_string(),
                stdout: "hello".to_string(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
        }
    }

    #[test]
    fn plain_matches_to_string_format() {
        let out = sample().render_plain();
        let text = out.as_str();
        assert!(text.starts_with("$ echo hello\n"));
        assert!(text.contains("exit: 0 (success)\n"));
        assert!(text.contains("duration: 42ms\n"));
        assert!(text.contains("\nstdout:\nhello\n"));
    }

    #[test]
    fn plain_omits_empty_stderr() {
        let out = sample().render_plain();
        assert!(!out.as_str().contains("stderr:"));
    }

    #[test]
    fn json_matches_to_string_format() {
        let out = sample().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["command"], "echo hello");
        assert_eq!(v["exit_code"], 0);
        assert_eq!(v["duration_ms"], 42);
        assert_eq!(v["truncated"]["stdout"], false);
        assert_eq!(v["truncated"]["stderr"], false);
    }

    #[test]
    fn plain_with_stderr() {
        let mut r = sample();
        r.process.stderr = "warning: something".to_string();
        let out = r.render_plain();
        assert!(out.as_str().contains("\nstderr:\nwarning: something\n"));
    }

    #[test]
    fn plain_with_truncation_marker() {
        let mut r = sample();
        r.process.stdout_truncated = true;
        let out = r.render_plain();
        assert!(out.as_str().contains(o8v_process::TRUNCATION_MARKER));
    }
}
