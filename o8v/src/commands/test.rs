// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `test` command — detect the project, find the test runner, run it.
//!
//! - `8v test [path]` — detects project at path (default: `.`), runs its tests
//! - `8v test [path] --json` — structured JSON output
//! - `8v test [path] --timeout <secs>` — override default 300s timeout

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Timeout in seconds (default: 300)
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Maximum output lines shown per failing section (default: 30; 0 = no limit)
    #[arg(long, default_value = "30")]
    pub limit: usize,

    /// Page number for paginated output (default: 1)
    #[arg(long, default_value = "1")]
    pub page: usize,

    /// Show extracted errors above raw stderr on test failure (default: on)
    #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue, overrides_with = "no_errors_first")]
    pub errors_first: bool,

    /// Disable errors-first mode
    #[arg(long = "no-errors-first", action = clap::ArgAction::SetFalse, overrides_with = "errors_first")]
    pub no_errors_first: bool,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::test_report::TestReport;

use o8v_core::{exit_code_number, exit_label, validate_timeout};

pub struct TestCommand {
    pub args: Args,
}

impl Command for TestCommand {
    type Report = TestReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        validate_timeout(self.args.timeout).map_err(CommandError::Execution)?;

        let workspace = ctx
            .extensions
            .get::<crate::workspace::WorkspaceRoot>()
            .ok_or_else(|| {
                CommandError::Execution("8v: no workspace — run 8v init first".to_string())
            })?;

        let abs_path = workspace.resolve(&self.args.path);

        let root = o8v_core::project::ProjectRoot::new(&abs_path)
            .map_err(|e| CommandError::Execution(format!("8v: invalid path: {e}")))?;

        let result = o8v_stacks::detect_all(&root);
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
        let error_extractor = tools.error_extractor;
        let test_tool = match tools.test_runner {
            Some(t) => t,
            None => {
                // Config-language stacks (helm, kustomize, terraform, dockerfile)
                // have no native test concept — don't imply a gap we'll later fill.
                return Err(CommandError::Execution(format!(
                    "8v test: the {} stack has no test concept. \
                     If this project has tests, run them with the stack's native \
                     test tool directly.",
                    project.stack()
                )));
            }
        };

        // Refine the default at runtime for projects that deviate from the
        // stack's stock tool (pnpm/yarn/bun lockfile, gradlew wrapper, RSpec,
        // etc.). Without this the agent hits a native-runner error from the
        // stock default and thrashes since Bash is denied.
        let project_path = std::path::PathBuf::from(project.path().to_string());
        let resolved = o8v_stacks::resolve_test_tool(
            project.stack(),
            &project_path,
            test_tool.program,
            test_tool.args,
        )
        .map_err(|e| CommandError::Execution(format!("8v test: {e}")))?;

        let mut cmd = std::process::Command::new(&resolved.program);
        cmd.args(&resolved.args);
        cmd.current_dir(&project_path);

        let config = o8v_process::ProcessConfig {
            timeout: std::time::Duration::from_secs(self.args.timeout),
            max_stdout: o8v_process::DEFAULT_MAX_OUTPUT,
            max_stderr: o8v_process::DEFAULT_MAX_OUTPUT,
            interrupted: Some(ctx.interrupted),
        };

        let proc_result = o8v_process::run(cmd, &config);
        let cmd_str = format!("{} {}", resolved.program, resolved.args.join(" "));

        let success = matches!(proc_result.outcome, o8v_process::ExitOutcome::Success);

        // Extract structured errors on failure when errors_first is enabled.
        let errors = if !success && self.args.errors_first {
            if let Some(extractor) = &error_extractor {
                (extractor.extract)(
                    &proc_result.stdout,
                    &proc_result.stderr,
                    &project_path,
                    o8v_stacks::stack_tools::RunKind::Test,
                )
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let name = project_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        Ok(TestReport {
            name,
            process: o8v_core::process_report::ProcessReport {
                command: cmd_str,
                exit_code: exit_code_number(&proc_result.outcome),
                success,
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
                color: !self.args.format.no_color && std::env::var_os("NO_COLOR").is_none(),
                page: self.args.page,
                errors_first: self.args.errors_first,
            },
            errors,
        })
    }
}
