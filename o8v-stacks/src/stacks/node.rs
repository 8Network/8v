//! Node-specific tool check — resolves from local `node_modules/.bin`.
//!
//! Does not fall back to global PATH — a global tool may be the wrong
//! version, stale, or fake. False positives from ambient state violate
//! 8v's reliability promise.

use crate::enrich::{enrich, ParseFn};
use crate::tool_resolution::find_node_bin;
use o8v_core::diagnostic::ParseStatus;
use o8v_core::{Check, CheckContext, CheckOutcome, ErrorKind};
use std::process::Command;

/// A check for Node-based tools.
///
/// Resolves from `./node_modules/.bin` ONLY.
/// If the tool is not installed locally, reports Error with install instructions.
pub struct NodeToolCheck {
    pub name: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    /// Stack name for enrichment (e.g. "typescript", "javascript").
    pub stack: &'static str,
    /// Optional parser — if set, enriches the outcome with structured diagnostics.
    /// Uses `self.name` and `self.stack` for attribution — no duplication.
    pub parser: Option<ParseFn>,
    /// If set, when the tool fails and stderr contains this pattern,
    /// downgrade to Passed (tool not configured for this project).
    pub skip_stderr_pattern: Option<&'static str>,
    /// If true, missing tool → Passed (tool not used by this project).
    /// If false, missing tool → Error (tool should be installed).
    pub optional: bool,
}

impl Check for NodeToolCheck {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&self, project_dir: &o8v_fs::ContainmentRoot, ctx: &CheckContext) -> CheckOutcome {
        // Walk up from project_dir to find the tool in node_modules/.bin
        let Some(bin_path) = find_node_bin(project_dir, self.program) else {
            if self.optional {
                // Tool is optional — not installed means not used by this project
                return CheckOutcome::passed(
                    String::new(),
                    String::new(),
                    ParseStatus::Parsed,
                    false,
                    false,
                );
            }
            return CheckOutcome::error(
                ErrorKind::Runtime,
                format!(
                    "'{}' not installed locally — run npm install in your project",
                    self.name
                ),
            );
        };

        let mut cmd = Command::new(&bin_path);
        cmd.args(self.args).current_dir(project_dir.as_path());

        // Run through o8v-process directly to inspect SpawnError kind
        // before it gets flattened into a string by run_tool.
        let config = o8v_process::ProcessConfig {
            timeout: ctx.timeout,
            interrupted: Some(ctx.interrupted),
            ..o8v_process::ProcessConfig::default()
        };
        let result = o8v_process::run(cmd, &config);

        // SpawnError::NotFound means the binary exists but is broken (e.g., stale symlink).
        // Keep as a safety net in case the binary disappeared between the walk-up check
        // and the actual execution.
        if let o8v_process::ExitOutcome::SpawnError {
            kind: std::io::ErrorKind::NotFound,
            ..
        } = &result.outcome
        {
            return CheckOutcome::error(
                ErrorKind::Runtime,
                format!(
                    "'{}' not installed locally — run npm install in your project",
                    self.name
                ),
            );
        }

        // Check if tool should skip gracefully when not configured for this project.
        if let Some(pattern) = self.skip_stderr_pattern {
            if let o8v_process::ExitOutcome::Failed { .. } = &result.outcome {
                if result.stderr.contains(pattern) {
                    // Tool not configured — skip gracefully
                    tracing::warn!("{}: skipped (not configured)", self.name);
                    return CheckOutcome::passed(
                        result.stdout.clone(),
                        result.stderr.clone(),
                        ParseStatus::Parsed,
                        result.stdout_truncated,
                        result.stderr_truncated,
                    );
                }
            }
        }

        // For all other outcomes, use the standard run_tool conversion path.
        let outcome = crate::runner::process_result_to_outcome(result, self.name);

        // Enrich with parser if provided.
        if let Some(parse_fn) = self.parser {
            return enrich(outcome, project_dir, self.name, self.stack, parse_fn);
        }

        outcome
    }
}

// ─── Shared web tool builders ────────────────────────────────────────────────
//
// prettier, biome, and oxlint are invoked identically in every web stack
// (TypeScript, JavaScript). The only difference is the `stack` attribution.
// Define them once here; stacks call the builder with their stack name.
//
// The formatter is identical across all web stacks — no stack attribution needed.

use crate::stack_tools::FormatTool;

pub(super) fn prettier_check(stack: &'static str) -> NodeToolCheck {
    NodeToolCheck {
        name: "prettier",
        program: "prettier",
        args: &["--list-different", "."],
        stack,
        parser: Some(crate::parse::prettier::parse),
        skip_stderr_pattern: None,
        optional: true,
    }
}

pub(super) fn biome_check(stack: &'static str) -> NodeToolCheck {
    NodeToolCheck {
        name: "biome",
        program: "biome",
        args: &["ci", ".", "--reporter=json"],
        stack,
        parser: Some(crate::parse::biome::parse),
        skip_stderr_pattern: None,
        optional: true,
    }
}

pub(super) fn oxlint_check(stack: &'static str) -> NodeToolCheck {
    NodeToolCheck {
        name: "oxlint",
        program: "oxlint",
        args: &[".", "--format=json"],
        stack,
        parser: Some(crate::parse::oxlint::parse),
        skip_stderr_pattern: None,
        optional: true,
    }
}

pub(super) fn prettier_formatter() -> FormatTool {
    FormatTool {
        program: "prettier",
        format_args: &["--write", "."],
        check_args: &["--check", "."],
        check_dirty_on_stdout: false,
        needs_node_resolution: true,
    }
}
