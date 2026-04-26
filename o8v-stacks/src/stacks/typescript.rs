//! TypeScript stack — tsc, eslint, biome, oxlint (via local node_modules/.bin).

use super::node::{biome_check, oxlint_check, prettier_check, prettier_formatter, NodeToolCheck};
use crate::stack_tools::{BuildTool, ErrorExtractor, RunKind, StackTools, TestTool};

fn typescript_extract(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    kind: RunKind,
) -> Vec<o8v_core::diagnostic::Diagnostic> {
    match kind {
        // tsc writes to stdout; build always uses tsc output format.
        RunKind::Build => {
            crate::parse::tsc::parse(stdout, stderr, project_root, "tsc", "typescript").diagnostics
        }
        RunKind::Test => {
            // Inspect package.json scripts.test to decide whether tsc produced the output.
            let pkg_path = project_root.join("package.json");
            let pkg_content = match std::fs::read_to_string(&pkg_path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "typescript test runner: could not read package.json at {}: {}; \
                         falling back to raw output",
                        pkg_path.display(),
                        e
                    );
                    return Vec::new();
                }
            };
            let pkg: serde_json::Value = match serde_json::from_str(&pkg_content) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "typescript test runner: could not parse package.json at {}: {}; \
                         falling back to raw output",
                        pkg_path.display(),
                        e
                    );
                    return Vec::new();
                }
            };
            let test_script = pkg
                .get("scripts")
                .and_then(|s| s.get("test"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if test_script.contains("tsc") {
                crate::parse::tsc::parse(stdout, stderr, project_root, "tsc", "typescript")
                    .diagnostics
            } else {
                tracing::warn!(
                    "typescript test runner {:?} has no structured error extractor; \
                     falling back to raw output",
                    test_script
                );
                Vec::new()
            }
        }
    }
}

const STACK: &str = "typescript";

/// Returns all tools for the typescript stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(NodeToolCheck {
                name: "tsc",
                program: "tsc",
                args: &["--noEmit", "--pretty", "false"],
                stack: STACK,
                parser: Some(crate::parse::tsc::parse),
                skip_stderr_pattern: None,
                optional: false,
            }),
            Box::new(NodeToolCheck {
                name: "eslint",
                program: "eslint",
                args: &[
                    ".",
                    "--ext",
                    ".ts,.tsx",
                    "--format=json",
                    "--max-warnings",
                    "0",
                ],
                stack: STACK,
                parser: Some(crate::parse::eslint::parse),
                skip_stderr_pattern: Some("eslint.config"),
                optional: false,
            }),
            Box::new(prettier_check(STACK)),
            Box::new(biome_check(STACK)),
            Box::new(oxlint_check(STACK)),
        ],
        formatter: Some(prettier_formatter()),
        test_runner: Some(TestTool {
            program: "npm",
            args: &["test", "--silent"],
        }),
        build_tool: Some(BuildTool {
            program: "tsc",
            args: &[],
        }),
        error_extractor: Some(ErrorExtractor {
            extract: typescript_extract,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RunKind::Test with a jest-based package.json → no diagnostics (no tsc extractor).
    #[test]
    fn test_kind_jest_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pkg = serde_json::json!({
            "name": "my-project",
            "scripts": { "test": "jest" }
        });
        std::fs::write(dir.path().join("package.json"), pkg.to_string())
            .expect("write package.json");

        let result = typescript_extract("", "", dir.path(), RunKind::Test);
        assert!(
            result.is_empty(),
            "jest runner should yield no diagnostics; got {result:?}"
        );
    }

    /// RunKind::Test with a tsc-based package.json → delegates to tsc parser.
    #[test]
    fn test_kind_tsc_delegates() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pkg = serde_json::json!({
            "name": "my-project",
            "scripts": { "test": "tsc --noEmit" }
        });
        std::fs::write(dir.path().join("package.json"), pkg.to_string())
            .expect("write package.json");

        // Empty stdout → tsc parser returns no diagnostics, but the path (tsc::parse) is exercised.
        let result = typescript_extract("", "", dir.path(), RunKind::Test);
        assert!(result.is_empty());
    }

    /// RunKind::Test with missing package.json → no diagnostics (warn + fallback).
    #[test]
    fn test_kind_missing_pkg_json_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = typescript_extract("", "", dir.path(), RunKind::Test);
        assert!(result.is_empty());
    }
}
