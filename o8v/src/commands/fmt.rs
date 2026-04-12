//! The `fmt` command — args + execution.

use o8v_core::parse_timeout;
use o8v_project::ProjectRoot;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Path to format [default: current directory]
    pub path: Option<String>,

    /// Check mode: report if formatting needed, don't modify files
    #[arg(long)]
    pub check: bool,

    /// Show extra context (project paths, timing)
    #[arg(long, short)]
    pub verbose: bool,

    /// Plain text output for AI agents and pipes
    #[arg(long, conflicts_with = "json")]
    pub plain: bool,

    /// JSON output for tools and CI
    #[arg(long, conflicts_with = "plain")]
    pub json: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Timeout per formatter (e.g. "5m", "30s", "300")
    #[arg(long, value_parser = parse_timeout)]
    pub timeout: Option<Duration>,
}

impl Args {
    pub fn audience(&self) -> o8v_core::render::Audience {
        if self.json {
            o8v_core::render::Audience::Machine
        } else if self.plain {
            o8v_core::render::Audience::Agent
        } else {
            o8v_core::render::Audience::Human
        }
    }
}

// ─── Run ────────────────────────────────────────────────────────────────────

/// Run `8v fmt`.
///
/// Returns the report. The caller decides how to render and what exit code to use.
pub(crate) fn run(args: &Args, interrupted: &'static AtomicBool) -> Result<o8v_core::FmtReport, String> {
    let path_str = args.path.as_deref().unwrap_or(".");
    let root = ProjectRoot::new(path_str)
        .map_err(|e| o8v_core::render::sanitize_for_display(&e.to_string()))?;

    let fmt_config = o8v_core::FmtConfig {
        timeout: args.timeout,
        check_mode: args.check,
        interrupted,
    };

    Ok(o8v_stacks::fmt(&root, &fmt_config))
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::FmtReport;

pub struct FmtCommand {
    pub args: Args,
}

impl Command for FmtCommand {
    type Report = FmtReport;

    async fn execute(
        &self,
        ctx: &CommandContext,
    ) -> Result<Self::Report, CommandError> {
        run(&self.args, ctx.interrupted).map_err(CommandError::Execution)
    }
}
