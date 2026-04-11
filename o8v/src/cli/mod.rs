// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! CLI-specific dispatch helpers and tracing initialisation.
//!
//! These are binary-only utilities — they cannot live in the library
//! because `cli_dispatch` creates a tokio runtime (binary concern).

pub(crate) mod common;
pub(crate) mod signal;

use clap::{Parser, Subcommand};
use o8v_core::command::{Command as CommandTrait, CommandError};
use o8v_core::render::{Audience, Renderable};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

#[derive(Parser)]
#[command(
    name = "8v",
    version,
    about = "Code reliability tool — one command checks everything"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Build the project
    Build(crate::commands::build::Args),
    /// Check a project directory
    Check(crate::commands::check::Args),
    /// Format a project directory
    Fmt(crate::commands::fmt::Args),
    /// Initialize 8v in a project
    Init(crate::init::Args),
    /// Run hooks (git pre-commit, etc.)
    Hooks(crate::hooks::Args),
    /// Run project tests
    Test(crate::commands::test::Args),
    /// Upgrade 8v to the latest version
    Upgrade(crate::commands::upgrade::Args),
    /// Read a file — symbol map, line range, or full content
    Read(crate::commands::read::Args),
    /// Write to a file — line-based editing
    Write(crate::commands::write::Args),
    /// Run a command with containment, timeout, and structured output
    Run(crate::commands::run::Args),
    /// Search file contents or file names
    Search(crate::commands::search::Args),
    /// List project files and directory structure
    Ls(crate::commands::ls::Args),
    /// Start MCP server
    Mcp,
}

/// Run a command through the dispatch pipeline, returning the rendered output
/// and the typed report so the caller can extract exit codes.
pub(crate) fn cli_dispatch<C: CommandTrait>(
    command: &C,
    audience: Audience,
    interrupted: &'static AtomicBool,
) -> Result<(String, C::Report), CommandError>
where
    C::Report: Renderable,
    C::Event: 'static,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CommandError::Execution(format!("runtime: {e}")))?;
    let ctx = o8v::dispatch::core_context(interrupted);
    let (output, _, report) = rt.block_on(o8v::dispatch::dispatch(command, &ctx, audience))?;
    Ok((output, report))
}

/// Run a command that always succeeds (ls, read, search, write).
/// Errors become exit code 1; success is exit code 0.
pub(crate) fn cli_run<C: CommandTrait>(
    command: &C,
    audience: Audience,
    interrupted: &'static AtomicBool,
) -> ExitCode
where
    C::Report: Renderable,
    C::Event: 'static,
{
    match cli_dispatch(command, audience, interrupted) {
        Ok((output, _)) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Map a `ProcessReport` to an exit code.
///
/// Propagates the process's own exit code on failure so that `8v build`,
/// `8v test`, and `8v run` all mirror the underlying tool's exit code.
pub(crate) fn process_exit_code(report: &o8v_core::process_report::ProcessReport) -> ExitCode {
    if report.success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(report.exit_code.clamp(0, 255) as u8)
    }
}

pub(crate) fn init_tracing() {
    #[allow(clippy::disallowed_methods)]
    let filter = match tracing_subscriber::EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(e) => {
            // Only warn if RUST_LOG was set but invalid — not if it was absent.
            if std::env::var_os("RUST_LOG").is_some() {
                eprintln!("warning: invalid RUST_LOG filter: {e}");
            }
            tracing_subscriber::EnvFilter::new("off")
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
