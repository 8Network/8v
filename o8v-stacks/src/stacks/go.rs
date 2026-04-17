//! Go stack — go vet, staticcheck.

use crate::stack_tools::{BuildTool, ErrorExtractor, FormatTool, RunKind, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

fn go_extract(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    kind: RunKind,
) -> Vec<o8v_core::diagnostic::Diagnostic> {
    match kind {
        RunKind::Test => {
            crate::parse::go_test_json::parse_test(stdout, stderr, project_root, "go test", "go")
        }
        RunKind::Build => {
            crate::parse::go_test_json::parse_build(stdout, stderr, project_root, "go build", "go")
        }
    }
}

/// Returns all tools for the go stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(EnrichedToolCheck {
                name: "go vet",
                program: "go",
                args: &["vet", "-json", "./..."],
                stack: "go",
                parse_fn: crate::parse::govet::parse,
                env: &[],
            }),
            Box::new(EnrichedToolCheck {
                name: "staticcheck",
                program: "staticcheck",
                args: &["-f", "json", "./..."],
                stack: "go",
                parse_fn: crate::parse::staticcheck::parse,
                env: &[],
            }),
        ],
        formatter: Some(FormatTool {
            program: "gofmt",
            format_args: &["-w", "."],
            check_args: &["-l", "."],
            check_dirty_on_stdout: true,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "go",
            args: &["test", "./..."],
        }),
        build_tool: Some(BuildTool {
            program: "go",
            args: &["build", "./..."],
        }),
        error_extractor: Some(ErrorExtractor {
            extract: go_extract,
        }),
    }
}
