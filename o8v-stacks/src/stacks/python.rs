//! Python stack — ruff check and mypy static type checking.

use crate::stack_tools::{FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the python stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(EnrichedToolCheck {
                name: "ruff",
                program: "ruff",
                args: &["check", "--output-format=json"],
                stack: "python",
                parse_fn: crate::parse::ruff::parse,
                env: &[],
            }),
            Box::new(EnrichedToolCheck {
                name: "mypy",
                program: "mypy",
                args: &[".", "-O", "json", "--no-error-summary"],
                stack: "python",
                parse_fn: crate::parse::mypy::parse,
                env: &[],
            }),
        ],
        formatter: Some(FormatTool {
            program: "ruff",
            format_args: &["format", "."],
            check_args: &["format", "--check", "."],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "python3",
            args: &["-m", "pytest", "-q"],
        }),
        build_tool: None,
    }
}
