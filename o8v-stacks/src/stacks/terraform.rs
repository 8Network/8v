//! Terraform stack — tflint.

use crate::stack_tools::{FormatTool, StackTools};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the terraform stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "tflint",
            program: "tflint",
            args: &["--format=json"],
            stack: "terraform",
            parse_fn: crate::parse::tflint::parse,
            env: &[],
        })],
        formatter: Some(FormatTool {
            program: "terraform",
            format_args: &["fmt", "-recursive"],
            check_args: &["fmt", "-check", "-recursive"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: None,
        build_tool: None,
    }
}
