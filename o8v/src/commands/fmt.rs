//! The `fmt` command — args + execution.

use o8v_core::parse_timeout;
use o8v_core::project::ProjectRoot;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Path to format [default: current directory]
    pub path: Option<String>,

    /// Check mode: report if formatting needed, don't modify files
    #[arg(long)]
    pub check: bool,

    /// Show extra context (project paths, timing)
    #[arg(long, short)]
    pub verbose: bool,

    /// Timeout per formatter (e.g. "5m", "30s", "300")
    #[arg(long, value_parser = parse_timeout)]
    pub timeout: Option<Duration>,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

// ─── Run ────────────────────────────────────────────────────────────────────

/// Run `8v fmt`.
///
/// Returns the report. The caller decides how to render and what exit code to use.
pub(crate) fn run(
    args: &Args,
    interrupted: &'static AtomicBool,
) -> Result<o8v_core::FmtReport, String> {
    let path_str = args.path.as_deref().unwrap_or(".");
    if std::path::Path::new(path_str).is_file() {
        return Err(format!(
            "fmt requires a directory path; got file: {path_str}"
        ));
    }
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

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        run(&self.args, ctx.interrupted).map_err(CommandError::Execution)
    }
}
