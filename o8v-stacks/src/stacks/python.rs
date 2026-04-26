//! Python stack — ruff check and mypy static type checking.

use crate::stack_tools::{ErrorExtractor, FormatTool, RunKind, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

fn python_extract(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    kind: RunKind,
) -> Vec<o8v_core::diagnostic::Diagnostic> {
    match kind {
        RunKind::Test => {
            crate::parse::pytest_text::parse(stdout, stderr, project_root, "pytest", "python")
        }
        // `build_tool: None` means this arm is unreachable in practice.
        RunKind::Build => {
            tracing::warn!("python stack has no build step; error extraction skipped");
            Vec::new()
        }
    }
}

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
                optional: false,
            }),
            Box::new(EnrichedToolCheck {
                name: "mypy",
                program: "mypy",
                args: &[".", "-O", "json", "--no-error-summary"],
                stack: "python",
                parse_fn: crate::parse::mypy::parse,
                env: &[],
                optional: false,
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
        error_extractor: Some(ErrorExtractor {
            extract: python_extract,
        }),
    }
}
