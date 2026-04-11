// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! High-level E2E assertions for integration testing.

use crate::binary::run_bin;
use crate::expected::Expected;
use crate::fixture::Fixture;
use crate::json::{JsonDiagnostic, JsonOutput};
use crate::query::find_entry;
use o8v_core::{CheckOutcome, CheckReport, CheckResult, Diagnostic, Location};

/// Assert all expected checks and diagnostics appear in the report.
///
/// For each `[[check]]` in the Expected:
/// - If it has diagnostics: asserts each diagnostic appears in the tool's output.
///   Extra diagnostics from the tool are allowed (tools evolve).
/// - If it has no diagnostics: asserts the tool produced `Passed`.
///
/// Also asserts every listed tool actually ran (exists in the report).
///
/// # Panics
/// Panics with detailed context: what was expected, what was found.
pub fn assert_expected(report: &CheckReport, expected: &Expected) {
    // Verify the expected stack was detected.
    let stack_found = report
        .results()
        .iter()
        .any(|r| r.stack().label() == expected.stack);
    assert!(
        stack_found,
        "expected stack '{}' not found in results (found: {:?})",
        expected.stack,
        report
            .results()
            .iter()
            .map(|r| r.stack().to_string())
            .collect::<Vec<_>>()
    );

    assert!(
        !expected.checks.is_empty(),
        "EXPECTED.toml has no [[check]] entries — an empty expectation proves nothing"
    );

    let result = find_result_by_label(report, &expected.stack);

    for check in &expected.checks {
        let entry = find_entry(result, &check.tool);

        if check.diagnostics.is_empty() {
            // No expected diagnostics — skip this check entirely.
            continue;
        } else {
            // Tool with expected diagnostics — collect and match.
            let diagnostics = match entry.outcome() {
                CheckOutcome::Failed { diagnostics, .. } => diagnostics,
                CheckOutcome::Passed { diagnostics, .. } => diagnostics,
                CheckOutcome::Error { cause, .. } => {
                    panic!("'{}' returned Error: {cause}", check.tool);
                }
                #[allow(unreachable_patterns)]
                other => panic!("'{}' unexpected outcome: {other:?}", check.tool),
            };

            assert!(
                !diagnostics.is_empty(),
                "'{}' produced no diagnostics — tool may not have run or parser returned empty",
                check.tool
            );

            for exp in &check.diagnostics {
                let expected_severity = exp.severity();

                let found = diagnostics.iter().any(|d| {
                    let rule_match = match &exp.rule {
                        Some(expected_rule) => d.rule.as_deref() == Some(expected_rule.as_str()),
                        None => true,
                    };
                    let file_match = match &d.location {
                        Location::File(f) | Location::Absolute(f) => f == &exp.file,
                        #[allow(unreachable_patterns)]
                        _ => false,
                    };
                    let severity_match = d.severity == expected_severity;
                    let message_match = match &exp.message_contains {
                        Some(substring) => d.message.contains(substring.as_str()),
                        None => true,
                    };
                    rule_match && file_match && severity_match && message_match
                });

                assert!(
                    found,
                    "expected diagnostic not found in '{}' output:\n\
                     \n  rule:             {:?}\
                     \n  file:             {}\
                     \n  severity:         {}\
                     \n  message_contains: {:?}\
                     \n\
                     \nactual diagnostics ({}):\n{}\
                     \n\nraw stdout: {}\nraw stderr: {}",
                    check.tool,
                    exp.rule,
                    exp.file,
                    exp.severity,
                    exp.message_contains,
                    diagnostics.len(),
                    format_diagnostics(&diagnostics.iter().collect::<Vec<_>>()),
                    match entry.outcome() {
                        CheckOutcome::Failed { raw_stdout, .. } =>
                            &raw_stdout[..raw_stdout.len().min(500)],
                        _ => "",
                    },
                    match entry.outcome() {
                        CheckOutcome::Failed { raw_stderr, .. } =>
                            &raw_stderr[..raw_stderr.len().min(500)],
                        _ => "",
                    },
                );
            }
        }
    }
}

/// Find a result by stack label (string). Panics if not found.
fn find_result_by_label<'a>(report: &'a CheckReport, stack: &str) -> &'a CheckResult {
    report
        .results()
        .iter()
        .find(|r| r.stack().label() == stack)
        .unwrap_or_else(|| {
            let found: Vec<_> = report.results().iter().map(|r| r.stack().label()).collect();
            panic!("stack '{stack}' not found in report (found: {found:?})")
        })
}

fn format_diagnostics(diagnostics: &[&Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| {
            let file = match &d.location {
                Location::File(f) | Location::Absolute(f) => f.as_str(),
                #[allow(unreachable_patterns)]
                other => panic!("unexpected Location variant in format_diagnostics: {other:?}"),
            };
            format!(
                "  rule={:?} file={} severity={} msg={:.80}",
                d.rule, file, d.severity, d.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Assert a report has no detection errors.
///
/// # Panics
/// Panics with the detection errors if any exist.
pub fn assert_no_detection_errors(report: &CheckReport) {
    assert!(
        report.detection_errors().is_empty(),
        "detection errors: {:?}",
        report.detection_errors()
    );
}

/// Assert a report detected exactly `count` projects.
///
/// # Panics
/// Panics with the actual count if it doesn't match.
pub fn assert_project_count(report: &CheckReport, count: usize) {
    assert_eq!(
        report.results().len(),
        count,
        "expected {count} projects, found {}",
        report.results().len()
    );
}

/// Assert the E2E test matches expectations end-to-end.
///
/// High-level wrapper that:
/// 1. Runs the 8v binary in JSON, plain, and human formats
/// 2. Checks exit codes (1 = violations found)
/// 3. Verifies expected diagnostics appear in output
/// 4. Bidirectional coverage: all tools in output must be in EXPECTED.toml
///
/// # Panics
/// Panics on any mismatch with detailed diagnostics.
pub fn assert_e2e(fixture: &Fixture, expected: &Expected) {
    // ── JSON format ─────────────────────────────────────────────
    let json_out = run_bin(fixture, &["--json"]);
    assert_eq!(
        json_out.status.code(),
        Some(1),
        "expected exit code 1 (violations), got {:?}\nstderr: {}",
        json_out.status.code(),
        String::from_utf8_lossy(&json_out.stderr)
    );

    let json_stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(!json_stdout.is_empty(), "JSON output should not be empty");

    // Deserialize into typed structs
    let output: JsonOutput =
        serde_json::from_str(&json_stdout).expect("failed to deserialize JSON output");
    assert!(
        !output.results.is_empty(),
        "JSON results should not be empty"
    );

    // Match expected checks against JSON output
    for check in &expected.checks {
        if check.diagnostics.is_empty() {
            // Tool with no expected diagnostics — verify it ran and passed
            let all_tools: Vec<&str> = output
                .results
                .iter()
                .flat_map(|r| r.checks.iter())
                .map(|c| c.name.as_str())
                .collect();
            let found = output
                .results
                .iter()
                .flat_map(|r| r.checks.iter())
                .find(|c| c.name == check.tool);
            assert!(
                found.is_some(),
                "JSON: tool '{}' not found in output. Available tools: {:?}",
                check.tool,
                all_tools
            );
            let c = found.expect("tool already validated above");
            assert_eq!(
                c.outcome, "passed",
                "JSON: tool '{}' expected passed, got {}",
                check.tool, c.outcome
            );
            continue;
        }

        // Find diagnostics for this tool in JSON
        let found_diagnostics: Vec<&JsonDiagnostic> = output
            .results
            .iter()
            .flat_map(|r| r.checks.iter())
            .filter(|c| c.name == check.tool)
            .flat_map(|c| c.diagnostics.iter())
            .collect();

        assert!(
            !found_diagnostics.is_empty(),
            "JSON: tool '{}' produced no diagnostics",
            check.tool
        );

        for exp in &check.diagnostics {
            let matched = found_diagnostics.iter().any(|d| {
                let rule_ok = match &exp.rule {
                    Some(r) => d.rule.as_deref() == Some(r.as_str()),
                    None => true,
                };
                let file_ok = d.location.path == exp.file;
                let sev_ok = d.severity == exp.severity;
                let msg_ok = match &exp.message_contains {
                    Some(m) => d.message.contains(m.as_str()),
                    None => true,
                };
                rule_ok && file_ok && sev_ok && msg_ok
            });
            assert!(
                matched,
                "JSON: expected diagnostic not found for '{}':\n  rule={:?} file={} severity={}\nJSON diagnostics: {:#?}",
                check.tool, exp.rule, exp.file, exp.severity, found_diagnostics
            );
        }
    }

    // ── Bidirectional tool coverage ───────────────────────────────
    // Verify ALL tools in the output are listed in EXPECTED.toml.
    // Without this, a new tool added to a stack silently passes untested.
    let expected_tool_names: Vec<&str> = expected.checks.iter().map(|c| c.tool.as_str()).collect();
    for result in &output.results {
        for check in &result.checks {
            assert!(
                expected_tool_names.contains(&check.name.as_str()),
                "JSON: tool '{}' ran but is not listed in EXPECTED.toml.\n\
                 Expected tools: {:?}\n\
                 Output tools: {:?}\n\
                 Add a [[check]] section for '{}' in EXPECTED.toml.",
                check.name,
                expected_tool_names,
                result.checks.iter().map(|c| &c.name).collect::<Vec<_>>(),
                check.name,
            );
        }
    }

    // ── Plain format ────────────────────────────────────────────
    let plain_out = run_bin(fixture, &["--plain"]);
    assert_eq!(
        plain_out.status.code(),
        Some(1),
        "plain: expected exit code 1"
    );
    let plain_stdout = String::from_utf8_lossy(&plain_out.stdout);
    assert!(
        !plain_stdout.contains("\x1b["),
        "plain output must not contain ANSI escapes"
    );
    // Verify diagnostic file paths appear in plain output
    for check in &expected.checks {
        for exp in &check.diagnostics {
            assert!(
                plain_stdout.contains(&exp.file),
                "plain: expected file '{}' not found in output:\n{}",
                exp.file,
                &plain_stdout[..plain_stdout.len().min(1000)]
            );
        }
    }

    // ── Human format (NO_COLOR) ─────────────────────────────────
    let human_out = run_bin(fixture, &[]);
    assert_eq!(
        human_out.status.code(),
        Some(1),
        "human: expected exit code 1"
    );
    let human_stderr = String::from_utf8_lossy(&human_out.stderr);
    assert!(
        !human_stderr.contains("\x1b["),
        "human output with NO_COLOR must not contain ANSI escapes"
    );
    // Human format writes to stderr — verify diagnostic info present
    for check in &expected.checks {
        for exp in &check.diagnostics {
            assert!(
                human_stderr.contains(&exp.file),
                "human: expected file '{}' not found in stderr:\n{}",
                exp.file,
                &human_stderr[..human_stderr.len().min(1000)]
            );
        }
    }
}
