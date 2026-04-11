//! Go stack — go vet, staticcheck.

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

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
    }
}
