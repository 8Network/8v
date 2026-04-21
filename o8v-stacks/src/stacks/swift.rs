//! Swift stack — swiftlint checking.
//!
//! For Swift projects, runs `swiftlint lint --reporter json` which outputs
//! JSON-formatted diagnostics.

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the Swift stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "swiftlint",
            program: "swiftlint",
            args: &["lint", "--reporter", "json"],
            stack: "swift",
            parse_fn: crate::parse::swiftlint::parse,
            env: &[],
            optional: false,
        })],
        formatter: Some(FormatTool {
            program: "swiftformat",
            format_args: &["--exclude", ".build", "."],
            check_args: &["--lint", "--exclude", ".build", "."],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "swift",
            args: &["test"],
        }),
        build_tool: Some(BuildTool {
            program: "swift",
            args: &["build"],
        }),
        error_extractor: None,
    }
}
