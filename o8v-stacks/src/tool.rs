//! Generic tool checks — run external programs as Check implementations.

use crate::enrich::{enrich, ParseFn};
use crate::runner::run_tool;
use o8v_core::diagnostic::ParseStatus;
use o8v_core::{Check, CheckContext, CheckOutcome};
use std::process::Command;

/// A check that shells out to an external tool (no enrichment).
pub struct ToolCheck {
    name: &'static str,
    program: String,
    args: Vec<String>,
}

impl ToolCheck {
    /// Create a new tool check.
    #[must_use]
    pub fn new(name: &'static str, program: &str, args: &[&str]) -> Self {
        Self {
            name,
            program: program.to_string(),
            args: args.iter().map(ToString::to_string).collect(),
        }
    }
}

impl Check for ToolCheck {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&self, project_dir: &o8v_fs::ContainmentRoot, ctx: &CheckContext) -> CheckOutcome {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args).current_dir(project_dir.as_path());
        run_tool(cmd, self.name, ctx.timeout, ctx.interrupted)
    }
}

/// A check that runs an external tool and enriches the output with a parser.
///
/// Eliminates the repeated `Command::new → run_tool → enrich` pattern across
/// stack modules. Stacks become pure declarations.
pub struct EnrichedToolCheck {
    /// Check name (e.g. "go vet", "ruff", "dotnet build").
    pub name: &'static str,
    /// Program binary (e.g. "go", "ruff", "dotnet", "deno", "cargo").
    pub program: &'static str,
    /// Arguments to pass to the program.
    pub args: &'static [&'static str],
    /// Stack name for enrichment (e.g. "go", "python", "dotnet").
    pub stack: &'static str,
    /// Parser function for enriching tool output.
    pub parse_fn: ParseFn,
    /// Environment variables to set (e.g. `&[("NO_COLOR", "1")]`).
    pub env: &'static [(&'static str, &'static str)],
    /// If true, missing tool → Passed (tool not installed on this machine).
    /// If false (default), missing tool → Error.
    pub optional: bool,
}

impl Check for EnrichedToolCheck {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&self, project_dir: &o8v_fs::ContainmentRoot, ctx: &CheckContext) -> CheckOutcome {
        let mut cmd = Command::new(self.program);
        cmd.args(self.args).current_dir(project_dir.as_path());
        for &(key, val) in self.env {
            cmd.env(key, val);
        }

        if self.optional {
            let config = o8v_process::ProcessConfig {
                timeout: ctx.timeout,
                interrupted: Some(ctx.interrupted),
                ..o8v_process::ProcessConfig::default()
            };
            let result = o8v_process::run(cmd, &config);
            if let o8v_process::ExitOutcome::SpawnError {
                kind: std::io::ErrorKind::NotFound,
                ..
            } = &result.outcome
            {
                return CheckOutcome::passed(
                    String::new(),
                    String::new(),
                    ParseStatus::Parsed,
                    false,
                    false,
                );
            }
            let outcome = crate::runner::process_result_to_outcome(result, self.name);
            return enrich(outcome, project_dir, self.name, self.stack, self.parse_fn);
        }

        let outcome = run_tool(cmd, self.name, ctx.timeout, ctx.interrupted);
        enrich(outcome, project_dir, self.name, self.stack, self.parse_fn)
    }
}
