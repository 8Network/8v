//! Dockerfile stack — hadolint.

use crate::stack_tools::StackTools;
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the dockerfile stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "hadolint",
            program: "hadolint",
            args: &["--format", "json", "Dockerfile"],
            stack: "dockerfile",
            parse_fn: crate::parse::hadolint::parse,
            env: &[],
            optional: false,
        })],
        formatter: None,
        test_runner: None,
        build_tool: None,
        error_extractor: None,
    }
}
