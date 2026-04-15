//! JavaScript stack — eslint, biome, oxlint (via local node_modules/.bin).

use super::node::{biome_check, oxlint_check, prettier_check, prettier_formatter, NodeToolCheck};
use crate::stack_tools::{StackTools, TestTool};

const STACK: &str = "javascript";

/// Returns all tools for the javascript stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(NodeToolCheck {
                name: "eslint",
                program: "eslint",
                args: &[".", "--format=json", "--max-warnings", "0"],
                stack: STACK,
                parser: Some(crate::parse::eslint::parse),
                skip_stderr_pattern: Some("eslint.config"),
                optional: false,
            }),
            Box::new(prettier_check(STACK)),
            Box::new(biome_check(STACK)),
            Box::new(oxlint_check(STACK)),
        ],
        formatter: Some(prettier_formatter()),
        test_runner: Some(TestTool {
            program: "npm",
            args: &["test", "--silent"],
        }),
        build_tool: None,
    }
}
