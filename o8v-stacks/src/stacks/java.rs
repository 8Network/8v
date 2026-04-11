//! Java stack — Maven compilation checking.
//!
//! For Maven projects, runs `mvn compile -q` which compiles and outputs
//! javac-style diagnostics to stdout (in Maven's wrapped format).
//! The javac parser handles both stdout (Maven) and stderr (javac/Gradle).

use crate::stack_tools::{BuildTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the Java stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(EnrichedToolCheck {
            name: "mvn compile",
            program: "mvn",
            args: &["compile", "-q"],
            stack: "java",
            parse_fn: crate::parse::javac::parse,
            env: &[],
        })],
        formatter: None,
        test_runner: Some(TestTool {
            program: "mvn",
            args: &["test"],
        }),
        build_tool: Some(BuildTool {
            program: "mvn",
            args: &["package", "-q"],
        }),
    }
}
