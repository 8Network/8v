//! Ruby stack — rubocop.

use crate::stack_tools::{FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the ruby stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "rubocop",
            program: "rubocop",
            args: &["--format", "json"],
            stack: "ruby",
            parse_fn: crate::parse::rubocop::parse,
            env: &[],
        })],
        formatter: Some(FormatTool {
            program: "rubocop",
            format_args: &["-a"],
            check_args: &["--format", "json"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "rake",
            args: &["test"],
        }),
        build_tool: None,
        error_extractor: None,
    }
}
