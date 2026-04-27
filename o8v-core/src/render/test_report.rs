// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use crate::diagnostic::Diagnostic;
use crate::process_report::ProcessReport;
use crate::render::RenderConfig;

pub struct TestReport {
    pub process: ProcessReport,
    pub stack: String,
    /// Short project name (directory basename).
    pub name: String,
    pub detection_errors: Vec<String>,
    pub render_config: RenderConfig,
    /// Structured errors extracted from test output (errors-first rendering).
    pub errors: Vec<Diagnostic>,
}

/// Parsed summary from cargo test JSON output.
struct SuiteSummary {
    passed: u64,
    failed: u64,
    ignored: u64,
}

/// Parse cargo test output, preferring nightly NDJSON and falling back to
/// stable libtest's plain-text `test result: ...` summary lines.
fn parse_suite_summary(stdout: &str) -> Option<SuiteSummary> {
    parse_suite_summary_json(stdout).or_else(|| parse_suite_summary_text(stdout))
}

/// Parse the last `{"type":"suite","event":"ok"|"failed",...}` line from cargo's JSON output.
fn parse_suite_summary_json(stdout: &str) -> Option<SuiteSummary> {
    let mut total: Option<SuiteSummary> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("suite") {
            let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
            if event == "ok" || event == "failed" {
                let passed = v.get("passed").and_then(|x| x.as_u64()).unwrap_or(0);
                let failed = v.get("failed").and_then(|x| x.as_u64()).unwrap_or(0);
                let ignored = v.get("ignored").and_then(|x| x.as_u64()).unwrap_or(0);
                match &mut total {
                    None => {
                        total = Some(SuiteSummary {
                            passed,
                            failed,
                            ignored,
                        });
                    }
                    Some(acc) => {
                        acc.passed += passed;
                        acc.failed += failed;
                        acc.ignored += ignored;
                    }
                }
            }
        }
    }
    total
}

/// Stable libtest fallback: parse `test result: ok. N passed; M failed; X ignored; ...`
/// lines (one per test binary). Multiple summaries are summed.
fn parse_suite_summary_text(stdout: &str) -> Option<SuiteSummary> {
    let mut total: Option<SuiteSummary> = None;
    for line in stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("test result: ") else {
            continue;
        };
        // rest is e.g. "ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        let after_dot = rest.split_once(". ").map(|(_, r)| r).unwrap_or(rest);
        let mut passed = 0u64;
        let mut failed = 0u64;
        let mut ignored = 0u64;
        let mut saw_passed = false;
        for part in after_dot.split(';') {
            let mut tokens = part.split_whitespace();
            let (Some(num), Some(label)) = (tokens.next(), tokens.next()) else {
                continue;
            };
            let Ok(n) = num.parse::<u64>() else {
                continue;
            };
            match label {
                "passed" => {
                    passed = n;
                    saw_passed = true;
                }
                "failed" => failed = n,
                "ignored" => ignored = n,
                _ => {}
            }
        }
        if !saw_passed {
            continue;
        }
        match &mut total {
            None => {
                total = Some(SuiteSummary {
                    passed,
                    failed,
                    ignored,
                });
            }
            Some(acc) => {
                acc.passed += passed;
                acc.failed += failed;
                acc.ignored += ignored;
            }
        }
    }
    total
}

impl super::Renderable for TestReport {
    fn render_plain(&self) -> Output {
        let p = &self.process;
        let mut buf = String::new();

        for e in &self.detection_errors {
            buf.push_str(&format!("warning: {e}\n"));
        }

        // Progressive header: "{name} {stack}"
        buf.push_str(&format!("{} {}\n", self.name, self.stack));

        if p.success {
            // Parse cargo JSON to get counts.
            if let Some(s) = parse_suite_summary(&p.stdout) {
                buf.push_str(&format!(
                    "tests {} passed {} failed {} ignored {}\n",
                    s.passed, s.failed, s.ignored, p.duration_display
                ));
            } else {
                buf.push_str(&format!("tests passed {}\n", p.duration_display));
            }
        } else {
            // Show summary line.
            if let Some(s) = parse_suite_summary(&p.stdout) {
                buf.push_str(&format!(
                    "tests {} passed {} failed {} ignored {}\n",
                    s.passed, s.failed, s.ignored, p.duration_display
                ));
            } else {
                buf.push_str(&format!("tests failed {}\n", p.duration_display));
            }

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
            } else {
                // Parse failed test names from JSON output.
                let mut showed_failure = false;
                for line in p.stdout.lines() {
                    let line = line.trim();
                    if !line.starts_with('{') {
                        continue;
                    }
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                        continue;
                    };
                    if v.get("type").and_then(|t| t.as_str()) == Some("test")
                        && v.get("event").and_then(|e| e.as_str()) == Some("failed")
                    {
                        if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                            buf.push_str(&format!("  FAILED {}\n", name));
                            showed_failure = true;
                        }
                    }
                }
                if !showed_failure && !p.stderr.trim().is_empty() {
                    buf.push_str(&format!("{}\n", p.stderr.trim()));
                }
            }
        }

        Output::new(buf)
    }

    fn render_json(&self) -> Output {
        let p = &self.process;
        let errors_json = serde_json::to_value(&self.errors)
            .expect("BUG: Vec<Diagnostic> is always serializable");

        // Parse suite summary for structured counts.
        let suite = parse_suite_summary(&p.stdout);
        let json = serde_json::json!({
            "name": self.name,
            "stack": self.stack,
            "success": p.success,
            "exit_code": p.exit_code,
            "duration_ms": p.duration.as_millis() as u64,
            "command": p.command,
            "passed": suite.as_ref().map(|s| s.passed),
            "failed": suite.as_ref().map(|s| s.failed),
            "ignored": suite.as_ref().map(|s| s.ignored),
            "errors": errors_json,
            "detection_errors": self.detection_errors,
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
    #[test]
    fn parse_text_summary_single_suite() {
        let stdout = "running 2 tests
test tests::a ... ok
test tests::b ... FAILED

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
";
        let s = parse_suite_summary(stdout).expect("text summary should parse");
        assert_eq!(s.passed, 1);
        assert_eq!(s.failed, 1);
        assert_eq!(s.ignored, 0);
    }

    #[test]
    fn parse_text_summary_sums_across_suites() {
        let stdout = "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let s = parse_suite_summary(stdout).expect("multiple suites should sum");
        assert_eq!(s.passed, 5);
        assert_eq!(s.failed, 1);
        assert_eq!(s.ignored, 1);
    }

    #[test]
    fn parse_text_summary_returns_none_on_no_result_line() {
        assert!(parse_suite_summary("error: test failed, to rerun pass --lib").is_none());
    }

    use std::time::Duration;

    fn sample_stdout_json() -> String {
        // Minimal cargo --format=json output for a passing suite with 2 tests.
        r#"{"type":"suite","event":"started","test_count":2}
{"type":"test","event":"ok","name":"foo","exec_time":0.1}
{"type":"test","event":"ok","name":"bar","exec_time":0.1}
{"type":"suite","event":"ok","passed":2,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.539}"#
            .to_string()
    }

    fn sample() -> TestReport {
        TestReport {
            process: ProcessReport {
                command: "cargo test --format=json --report-time".to_string(),
                exit_code: 0,
                success: true,
                exit_label: "0 (success)".to_string(),
                duration: Duration::from_millis(539),
                duration_display: "539ms".to_string(),
                stdout: sample_stdout_json(),
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

    fn sample_failure() -> TestReport {
        let stdout = r#"{"type":"suite","event":"started","test_count":1}
{"type":"test","event":"failed","name":"my_test","exec_time":0.01}
{"type":"suite","event":"failed","passed":0,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.5}"#
            .to_string();
        TestReport {
            process: ProcessReport {
                command: "cargo test --format=json --report-time".to_string(),
                exit_code: 1,
                success: false,
                exit_label: "1".to_string(),
                duration: Duration::from_millis(500),
                duration_display: "500ms".to_string(),
                stdout,
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

    /// Regression test: must FAIL on old code that starts with "$ cargo test\n".
    #[test]
    fn plain_success_progressive_format() {
        let out = sample().render_plain();
        let text = out.as_str();
        assert!(
            text.starts_with("myproject rust\n"),
            "expected progressive header 'myproject rust\\n', got: {text:?}"
        );
        assert!(
            text.contains("tests 2 passed 0 failed 0 ignored 539ms"),
            "expected test counts and duration, got: {text:?}"
        );
        // Must NOT contain old verbose format markers.
        assert!(
            !text.contains("$ cargo test"),
            "must not contain raw command line: {text:?}"
        );
        assert!(
            !text.contains("exit:"),
            "must not contain 'exit:' in progressive output: {text:?}"
        );
    }

    /// Regression test: failure shows counts + failed test names.
    #[test]
    fn plain_failure_progressive_format() {
        let out = sample_failure().render_plain();
        let text = out.as_str();
        assert!(
            text.starts_with("myproject rust\n"),
            "expected 'myproject rust\\n', got: {text:?}"
        );
        assert!(
            text.contains("0 passed 1 failed"),
            "expected failure counts, got: {text:?}"
        );
        assert!(
            text.contains("FAILED my_test"),
            "expected failed test name, got: {text:?}"
        );
    }

    #[test]
    fn plain_with_detection_errors() {
        let mut r = sample();
        r.detection_errors = vec!["cargo metadata failed".to_string()];
        let out = r.render_plain();
        assert!(out.as_str().starts_with("warning: cargo metadata failed\n"));
    }

    #[test]
    fn json_has_name_stack_counts() {
        let out = sample().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["name"], "myproject");
        assert_eq!(v["stack"], "rust");
        assert_eq!(v["success"], true);
        assert_eq!(v["passed"], 2);
        assert_eq!(v["failed"], 0);
        // Must NOT contain raw stdout/stderr.
        assert!(v.get("stdout").is_none(), "raw stdout must not be in JSON");
    }

    /// BUG TEST-1 regression: two suite lines (unit + doc-test) must be accumulated, not last-wins.
    #[test]
    fn multi_suite_counts_are_accumulated() {
        // Simulate cargo output with two suite-final lines:
        // - unit tests: 1 passed, 0 failed
        // - doc-tests:  0 passed, 0 failed
        // Total expected: 1 passed, 0 failed, 0 ignored.
        let stdout = r#"{"type":"suite","event":"started","test_count":1}
{"type":"test","event":"ok","name":"my_test","exec_time":0.05}
{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.1}
{"type":"suite","event":"started","test_count":0}
{"type":"suite","event":"ok","passed":0,"failed":0,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.0}"#.to_string();
        let report = TestReport {
            process: ProcessReport {
                command: "cargo test --format=json --report-time".to_string(),
                exit_code: 0,
                success: true,
                exit_label: "0 (success)".to_string(),
                duration: std::time::Duration::from_millis(100),
                duration_display: "100ms".to_string(),
                stdout,
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
            stack: "rust".to_string(),
            name: "myproject".to_string(),
            detection_errors: vec![],
            render_config: RenderConfig::default(),
            errors: vec![],
        };
        let text = report.render_plain();
        assert!(
            text.as_str().contains("tests 1 passed 0 failed 0 ignored"),
            "expected accumulated counts '1 passed', got: {:?}",
            text.as_str()
        );
    }
}
