//! Helm lint text parser — covers `helm lint .`.
//!
//! Helm writes diagnostics to stdout in the format:
//!   `[LEVEL] location: message`
//!
//! Where LEVEL is one of ERROR, WARNING, or INFO.
//! The summary line (`Error: N chart(s) linted, N chart(s) failed`) is skipped.
//! The linting header (`==> Linting chart-name`) is skipped.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity};
use o8v_core::display_str::DisplayStr;

/// Parse helm lint output (both stdout and stderr) into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();

    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(d) = parse_line(line, project_root, tool, stack) {
            diagnostics.push(d);
        }
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: 0, // text parsers don't track parsed_items
    }
}

/// Attempt to parse a single helm lint diagnostic line.
///
/// Expected format: `[LEVEL] location: message`
/// Skips: lines starting with `==> Linting`, lines starting with `Error:`.
fn parse_line(
    line: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Option<Diagnostic> {
    let line = line.trim();

    // Skip header lines
    if line.starts_with("==> Linting") {
        return None;
    }

    // Skip summary lines like "Error: 1 chart(s) linted, 1 chart(s) failed"
    if line.starts_with("Error:") {
        return None;
    }

    // Parse `[LEVEL] location: message`
    let line = line.strip_prefix('[')?;
    let bracket_end = line.find(']')?;
    let level = &line[..bracket_end];
    let rest = line[bracket_end + 1..].trim();

    let severity = match level {
        "ERROR" => Severity::Error,
        "WARNING" => Severity::Warning,
        "INFO" => Severity::Info,
        _ => {
            tracing::debug!(level, "unknown helm lint level");
            return None;
        }
    };

    // Split on first `: ` to separate location from message
    let colon_pos = rest.find(": ")?;
    let location_str = &rest[..colon_pos];
    let message = rest[colon_pos + 2..].to_string();

    if location_str.is_empty() || message.is_empty() {
        return None;
    }

    let location = super::normalize_path(location_str, project_root);

    Some(Diagnostic {
        location,
        span: None, // helm lint provides no line/column info
        rule: None,
        severity,
        raw_severity: Some(level.to_string()),
        message: DisplayStr::from_untrusted(message),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str, stderr: &str) -> ParseResult {
        parse(stdout, stderr, Path::new(ROOT), "helm lint", "helm")
    }

    // ─── Basic behavior ────────────────────────────────────────────────────

    #[test]
    fn empty_output() {
        let result = run("", "");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn skips_linting_header() {
        let result = run("==> Linting my-chart\n", "");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn skips_summary_line() {
        let result = run("Error: 1 chart(s) linted, 1 chart(s) failed\n", "");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_error() {
        let result = run("[ERROR] Chart.yaml: name is required\n", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "name is required");
        assert_eq!(d.location, Location::File("Chart.yaml".to_string()));
        assert_eq!(d.raw_severity, Some("ERROR".to_string()));
        assert!(d.span.is_none());
    }

    #[test]
    fn single_warning() {
        let result = run("[WARNING] templates/bad.yaml: some warning\n", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.message, "some warning");
        assert_eq!(d.location, Location::File("templates/bad.yaml".to_string()));
        assert_eq!(d.raw_severity, Some("WARNING".to_string()));
    }

    #[test]
    fn single_info() {
        let result = run("[INFO] Chart.yaml: icon is recommended\n", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Info);
        assert_eq!(d.message, "icon is recommended");
        assert_eq!(d.raw_severity, Some("INFO".to_string()));
    }

    #[test]
    fn full_helm_lint_output() {
        let stdout = "\
==> Linting chart-name\n\
[INFO] Chart.yaml: icon is recommended\n\
[WARNING] templates/bad.yaml: some warning\n\
[ERROR] Chart.yaml: name is required\n\
[ERROR] templates/: validation: chart.metadata.name is required\n\
\n\
Error: 1 chart(s) linted, 1 chart(s) failed\n";
        let result = run(stdout, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 4);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
        assert_eq!(result.diagnostics[2].severity, Severity::Error);
        assert_eq!(result.diagnostics[3].severity, Severity::Error);
    }

    #[test]
    fn parses_both_stdout_and_stderr() {
        let stdout = "[ERROR] Chart.yaml: name is required\n";
        let stderr = "[WARNING] values.yaml: some stderr warning\n";
        let result = run(stdout, stderr);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn message_with_colon_preserved() {
        // The `: ` split takes only the first occurrence
        let result = run(
            "[ERROR] templates/: validation: chart.metadata.name is required\n",
            "",
        );
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.message, "validation: chart.metadata.name is required");
        assert_eq!(d.location, Location::File("templates/".to_string()));
    }

    #[test]
    fn no_span_information() {
        let result = run("[ERROR] Chart.yaml: name is required\n", "");
        assert!(result.diagnostics[0].span.is_none());
    }

    #[test]
    fn no_rule_information() {
        let result = run("[ERROR] Chart.yaml: name is required\n", "");
        assert!(result.diagnostics[0].rule.is_none());
    }

    #[test]
    fn tool_and_stack_set() {
        let result = run("[ERROR] Chart.yaml: name is required\n", "");
        assert_eq!(result.diagnostics[0].tool, "helm lint");
        assert_eq!(result.diagnostics[0].stack, "helm");
    }

    // ─── Malformed input ───────────────────────────────────────────────────

    #[test]
    fn malformed_no_bracket() {
        let result = run("just a plain line\n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn malformed_unknown_level() {
        let result = run("[DEBUG] Chart.yaml: some debug\n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn malformed_no_colon_separator() {
        // No `: ` after location
        let result = run("[ERROR] Chart.yaml name is required\n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn malformed_empty_location() {
        let result = run("[ERROR] : message here\n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn malformed_empty_message() {
        let result = run("[ERROR] Chart.yaml: \n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn malformed_unclosed_bracket() {
        let result = run("[ERROR Chart.yaml: message\n", "");
        assert_eq!(result.diagnostics.len(), 0);
    }

    // ─── Stress tests ──────────────────────────────────────────────────────

    #[test]
    fn stress_huge_number_of_diagnostics() {
        let mut lines = Vec::new();
        for i in 0..1000 {
            lines.push(format!(
                "[WARNING] templates/resource-{i}.yaml: some issue {i}"
            ));
        }
        let input = lines.join("\n");
        let result = run(&input, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1000);
    }

    #[test]
    fn stress_unicode_message() {
        let result = run("[ERROR] Chart.yaml: 错误 🔥 Unicode message\n", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Unicode"));
    }

    #[test]
    fn stress_unicode_location() {
        let result = run("[WARNING] templates/文件.yaml: unicode path\n", "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_very_long_message() {
        let long_msg = "x".repeat(100_000);
        let input = format!("[ERROR] Chart.yaml: {long_msg}\n");
        let result = run(&input, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message.len(), 100_000);
    }

    #[test]
    fn stress_mixed_valid_and_invalid_lines() {
        let input = "\
==> Linting my-chart\n\
not a valid diagnostic\n\
[ERROR] Chart.yaml: real error\n\
Error: 1 chart(s) linted, 1 chart(s) failed\n\
[INVALID] something.yaml: bad level\n\
[WARNING] templates/ok.yaml: valid warning\n";
        let result = run(input, "");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn stress_whitespace_only_lines() {
        let input = "  \n\t\n   \n";
        let result = run(input, "");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn stress_all_three_levels() {
        let input = "\
[ERROR] Chart.yaml: error message\n\
[WARNING] templates/x.yaml: warning message\n\
[INFO] Chart.yaml: info message\n";
        let result = run(input, "");
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
        assert_eq!(result.diagnostics[2].severity, Severity::Info);
    }
}
