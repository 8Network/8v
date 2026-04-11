//! kustomize build stderr parser — covers `kustomize build .`.
//!
//! kustomize outputs YAML manifests to stdout on success (ignored — no diagnostics).
//! On failure, stderr contains lines starting with `Error: message`.
//! Each such line becomes one diagnostic at project root severity Error.

use o8v_core::diagnostic::{Diagnostic, Location, ParseResult, ParseStatus, Severity};
use o8v_core::display_str::DisplayStr;

/// Parse kustomize build output into diagnostics.
///
/// - stdout is ignored (YAML manifests on success, empty on failure)
/// - stderr lines starting with `Error: ` become Error-severity diagnostics
/// - Location is always project root (kustomize gives no file-specific info)
#[must_use]
pub fn parse(
    _stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let root_location = Location::Absolute(project_root.to_string_lossy().into_owned());

    let mut diagnostics = Vec::new();

    for line in stderr.lines() {
        let Some(message) = line.strip_prefix("Error: ") else {
            continue;
        };

        diagnostics.push(Diagnostic {
            location: root_location.clone(),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: Some("error".to_string()),
            message: DisplayStr::from_untrusted(message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    ParseResult {
        status: ParseStatus::Parsed,
        parsed_items: diagnostics.len() as u32,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stderr: &str) -> ParseResult {
        parse("", stderr, Path::new(ROOT), "kustomize build", "kustomize")
    }

    fn root_location() -> Location {
        Location::Absolute(ROOT.to_string())
    }

    #[test]
    fn empty_stderr() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn success_stdout_ignored() {
        // On success, kustomize outputs YAML to stdout — we ignore it
        let result = parse(
            "apiVersion: v1\nkind: ConfigMap\n",
            "",
            Path::new(ROOT),
            "kustomize build",
            "kustomize",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn single_error_line() {
        let result = run("Error: accumulating resources: accumulation err='...'");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.parsed_items, 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.raw_severity.as_deref(), Some("error"));
        assert_eq!(d.message, "accumulating resources: accumulation err='...'");
        assert_eq!(d.location, root_location());
        assert!(d.span.is_none());
        assert!(d.rule.is_none());
        assert_eq!(d.tool, "kustomize build");
        assert_eq!(d.stack, "kustomize");
    }

    #[test]
    fn invalid_kustomization_error() {
        let result = run(
            "Error: invalid Kustomization: yaml: mapping values are not allowed in this context",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].message,
            "invalid Kustomization: yaml: mapping values are not allowed in this context"
        );
    }

    #[test]
    fn multiple_error_lines() {
        let stderr = "Error: first error\nError: second error\nError: third error";
        let result = run(stderr);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 3);
        assert_eq!(result.parsed_items, 3);
        assert_eq!(result.diagnostics[0].message, "first error");
        assert_eq!(result.diagnostics[1].message, "second error");
        assert_eq!(result.diagnostics[2].message, "third error");
    }

    #[test]
    fn non_error_lines_ignored() {
        let stderr = "some info line\nError: real error\nanother line\nWarning: not matched";
        let result = run(stderr);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "real error");
    }

    #[test]
    fn malformed_no_error_prefix() {
        // Lines without "Error: " prefix are ignored
        let result = run("accumulation err='something bad'\nfailed to build");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn huge_stderr_many_errors() {
        let mut lines = Vec::new();
        for i in 0..10_000 {
            lines.push(format!("Error: error number {i}"));
        }
        let stderr = lines.join("\n");
        let result = run(&stderr);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10_000);
        assert_eq!(result.parsed_items, 10_000);
        // All severities are Error
        for d in &result.diagnostics {
            assert_eq!(d.severity, Severity::Error);
        }
    }

    #[test]
    fn unicode_error_message() {
        let result = run("Error: 无效的 kustomization 文件: 测试错误 🔥");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].message,
            "无效的 kustomization 文件: 测试错误 🔥"
        );
    }

    #[test]
    fn unicode_non_error_lines_ignored() {
        let stderr = "信息: 这不是错误\nError: 真正的错误\n提示: 检查配置";
        let result = run(stderr);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "真正的错误");
    }

    #[test]
    fn error_colon_no_space_not_matched() {
        // "Error:" without trailing space should not match
        let result = run("Error:missing space after colon");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn empty_message_after_error_prefix() {
        // "Error: " with empty message after → still produces a diagnostic with empty message
        let result = run("Error: ");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "");
    }

    #[test]
    fn location_is_project_root() {
        let result = run("Error: some build failure");
        assert_eq!(result.diagnostics[0].location, root_location());
    }

    #[test]
    fn whitespace_only_stderr() {
        let result = run("   \n\n\t  ");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
