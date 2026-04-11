//! Kustomize stack — kustomize build.

use crate::stack_tools::StackTools;
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the kustomize stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "kustomize build",
            program: "kustomize",
            args: &["build", "."],
            stack: "kustomize",
            parse_fn: crate::parse::kustomize::parse,
            env: &[],
        })],
        formatter: None,
        test_runner: None,
        build_tool: None,
    }
}
