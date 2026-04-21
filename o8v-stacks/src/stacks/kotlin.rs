//! Kotlin stack — ktlint checking.
//!
//! For Kotlin projects, runs `ktlint --reporter=json` which outputs
//! JSON-formatted diagnostics.

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the Kotlin stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "ktlint",
            program: "ktlint",
            args: &["--reporter=json", "--log-level=error"],
            stack: "kotlin",
            parse_fn: crate::parse::ktlint::parse,
            env: &[],
            optional: false,
        })],
        formatter: Some(FormatTool {
            program: "ktlint",
            format_args: &["--format"],
            check_args: &["--reporter=json"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "gradle",
            args: &["test"],
        }),
        build_tool: Some(BuildTool {
            program: "gradle",
            args: &["build"],
        }),
        error_extractor: None,
    }
}
