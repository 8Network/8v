//! Deno stack — deno check (uses deno-specific parser for stderr output).

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the deno stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "deno check",
            program: "deno",
            args: &["check"],
            stack: "deno",
            parse_fn: crate::parse::deno::parse,
            env: &[("NO_COLOR", "1")],
            optional: false,
        })],
        formatter: Some(FormatTool {
            program: "deno",
            format_args: &["fmt"],
            check_args: &["fmt", "--check"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "deno",
            args: &["test"],
        }),
        build_tool: Some(BuildTool {
            program: "deno",
            args: &["compile"],
        }),
        error_extractor: None,
    }
}
