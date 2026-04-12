// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `build` command — detect the project, find the build tool, run it.
//!
//! - `8v build [path]` — detects project at path (default: `.`), runs its build tool
//! - `8v build [path] --json` — structured JSON output
//! - `8v build [path] --timeout <secs>` — override default 300s timeout

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Timeout in seconds (default: 300)
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Maximum output lines to show per section (0 = no limit)
    #[arg(long, default_value = "30")]
    pub limit: usize,

    /// Page number for paginated output (default: 1)
    #[arg(long, default_value = "1")]
    pub page: usize,
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
use o8v_core::render::build_report::BuildReport;

use o8v_core::{exit_code_number, exit_label, validate_timeout};

pub struct BuildCommand {
    pub args: Args,
}

impl Command for BuildCommand {
    type Report = BuildReport;

    async fn execute(
        &self,
        ctx: &CommandContext,
    ) -> Result<Self::Report, CommandError> {
        validate_timeout(self.args.timeout).map_err(CommandError::Execution)?;

        let abs_path = crate::util::resolve_path(&self.args.path)
            .map_err(|e| CommandError::Execution(format!("8v: {e}")))?;

        let root = o8v_project::ProjectRoot::new(&abs_path)
            .map_err(|e| CommandError::Execution(format!("8v: invalid path: {e}")))?;

        let result = o8v_project::detect_all(&root);
        let (projects, errors) = result.into_parts();
        let detection_errors: Vec<String> = errors.iter().map(|e| e.to_string()).collect();

        if projects.is_empty() {
            let mut msg = "8v: no project detected".to_string();
            for e in &detection_errors {
                msg.push_str(&format!("\n  detection error: {e}"));
            }
            return Err(CommandError::Execution(msg));
        }

        let project = &projects[0];
        let stack_name = format!("{}", project.stack());

        let tools = o8v_stacks::tools_for(project.stack());
        let build_tool = match tools.build_tool {
            Some(t) => t,
            None => {
                return Err(CommandError::Execution(format!(
                    "8v: no build tool for {} projects",
                    project.stack()
                )))
            }
        };

        let mut cmd = std::process::Command::new(build_tool.program);
        cmd.args(build_tool.args);
        cmd.current_dir(std::path::Path::new(&project.path().to_string()));

        let config = o8v_process::ProcessConfig {
            timeout: std::time::Duration::from_secs(self.args.timeout),
            max_stdout: o8v_process::DEFAULT_MAX_OUTPUT,
            max_stderr: o8v_process::DEFAULT_MAX_OUTPUT,
            interrupted: Some(ctx.interrupted),
        };

        let proc_result = o8v_process::run(cmd, &config);
        let cmd_str = format!("{} {}", build_tool.program, build_tool.args.join(" "));

        Ok(BuildReport {
            process: o8v_core::process_report::ProcessReport {
                command: cmd_str,
                exit_code: exit_code_number(&proc_result.outcome),
                success: matches!(proc_result.outcome, o8v_process::ExitOutcome::Success),
                exit_label: exit_label(&proc_result.outcome),
                duration: proc_result.duration,
                duration_display: o8v_process::format_duration(proc_result.duration),
                stdout: proc_result.stdout,
                stderr: proc_result.stderr,
                stdout_truncated: proc_result.stdout_truncated,
                stderr_truncated: proc_result.stderr_truncated,
            },
            stack: stack_name,
            detection_errors,
            render_config: o8v_core::render::RenderConfig {
                limit: if self.args.limit == 0 {
                    None
                } else {
                    Some(self.args.limit)
                },
                verbose: false,
                color: false,
                page: self.args.page,
            },
        })
    }
}
