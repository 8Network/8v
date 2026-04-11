//! Rust stack — cargo check, clippy, cargo fmt.

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the rust stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(EnrichedToolCheck {
                name: "cargo check",
                program: "cargo",
                args: &[
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--message-format=json",
                ],
                stack: "rust",
                parse_fn: crate::parse::cargo::parse,
                env: &[],
            }),
            Box::new(EnrichedToolCheck {
                name: "clippy",
                program: "cargo",
                args: &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--message-format=json",
                    "--",
                    "-D",
                    "warnings",
                ],
                stack: "rust",
                parse_fn: crate::parse::cargo::parse,
                env: &[],
            }),
            Box::new(EnrichedToolCheck {
                name: "cargo fmt",
                program: "cargo",
                args: &["fmt", "--all", "--check", "--", "--color=never"],
                stack: "rust",
                parse_fn: crate::parse::rustfmt::parse,
                env: &[],
            }),
        ],
        formatter: Some(FormatTool {
            program: "cargo",
            format_args: &["fmt", "--all"],
            check_args: &["fmt", "--all", "--check"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "cargo",
            args: &["test", "--workspace"],
        }),
        build_tool: Some(BuildTool {
            program: "cargo",
            args: &["build"],
        }),
    }
}
