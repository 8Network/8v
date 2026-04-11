//! Ruby stack — rubocop.

use crate::stack_tools::StackTools;
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
        formatter: None,
        test_runner: None,
        build_tool: None,
    }
}
