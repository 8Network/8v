//! Helm stack — helm lint.

use crate::stack_tools::StackTools;
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the helm stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "helm lint",
            program: "helm",
            args: &["lint", "."],
            stack: "helm",
            parse_fn: crate::parse::helm::parse,
            env: &[],
        })],
        formatter: None,
        test_runner: None,
        build_tool: None,
        error_extractor: None,
    }
}
