// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `run` command — execute a command with containment, timeout, and structured output.
//!
//! - `8v run "echo hello"` — run a command
//! - `8v run "cargo build" --json` — structured JSON output
//! - `8v run "long-task" --timeout 300` — override default 120s timeout

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Command to run (e.g. "echo hello", "cargo build --release")
    pub command: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Timeout in seconds (default: 120)
    #[arg(long, default_value = "120")]
    pub timeout: u64,
}

impl Args {
    pub fn audience(&self) -> o8v_core::render::Audience {
        if self.json {
            o8v_core::render::Audience::Machine
        } else {
            o8v_core::render::Audience::Human
        }
    }
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::event_channel::EventChannel;
use o8v_core::events::run::RunEvent;
use o8v_core::render::run_report::RunReport;

use o8v_core::{exit_code_number, exit_label, validate_timeout};

pub struct RunCommand {
    pub args: Args,
}

impl Command for RunCommand {
    type Report = RunReport;
    type Event = RunEvent;

    async fn execute(
        &self,
        ctx: &CommandContext,
        _events: EventChannel<Self::Event>,
    ) -> Result<Self::Report, CommandError> {
        validate_timeout(self.args.timeout).map_err(CommandError::Execution)?;

        let parts = match shlex::split(&self.args.command) {
            Some(p) if p.is_empty() => {
                return Err(CommandError::Execution("8v: empty command".to_string()))
            }
            Some(p) => p,
            None => {
                return Err(CommandError::Execution(
                    "8v: invalid command — unbalanced quotes".to_string(),
                ))
            }
        };

        let program = &parts[0];
        let cmd_args = &parts[1..];

        let cwd = std::env::current_dir().map_err(|e| {
            CommandError::Execution(format!("8v: cannot determine working directory: {e}"))
        })?;

        let mut cmd = std::process::Command::new(program);
        cmd.args(cmd_args);
        cmd.current_dir(&cwd);

        let config = o8v_process::ProcessConfig {
            timeout: std::time::Duration::from_secs(self.args.timeout),
            max_stdout: o8v_process::DEFAULT_MAX_OUTPUT,
            max_stderr: o8v_process::DEFAULT_MAX_OUTPUT,
            interrupted: Some(ctx.interrupted),
        };

        let result = o8v_process::run(cmd, &config);

        Ok(RunReport {
            process: o8v_core::process_report::ProcessReport {
                command: self.args.command.clone(),
                exit_code: exit_code_number(&result.outcome),
                success: matches!(result.outcome, o8v_process::ExitOutcome::Success),
                exit_label: exit_label(&result.outcome),
                duration: result.duration,
                duration_display: o8v_process::format_duration(result.duration),
                stdout: result.stdout,
                stderr: result.stderr,
                stdout_truncated: result.stdout_truncated,
                stderr_truncated: result.stderr_truncated,
            },
        })
    }
}
