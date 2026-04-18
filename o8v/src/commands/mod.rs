// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Command modules and dispatch.
//!
//! Each sub-module owns the `Args` struct (parsed by clap) and the typed command
//! struct that implements `o8v_core::command::Command`.
//!
//! `dispatch_command()` is the single match on the Command enum — both CLI and
//! MCP entry points call it. It builds context, derives audience, dispatches.

pub mod build;
pub mod check;
pub mod fmt;
pub mod hooks;
pub mod init;
pub mod log;
pub mod ls;
pub mod output_format;
pub mod read;
pub mod search;
pub mod stats;
pub mod test;
pub mod upgrade;
pub mod write;

use clap::Subcommand;
use o8v_core::caller::Caller;
use o8v_core::command::CommandError;
use o8v_core::render::Audience;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build the project
    Build(build::Args),
    /// Check a project directory
    Check(check::Args),
    /// Format a project directory
    Fmt(fmt::Args),
    /// Initialize 8v in a project
    Init(crate::init::Args),
    /// Run hooks (git pre-commit, etc.)
    Hooks(crate::hooks::Args),
    /// Run project tests
    Test(test::Args),
    /// Upgrade 8v to the latest version
    Upgrade(upgrade::Args),
    /// Read a file — symbol map, line range, or full content
    Read(read::Args),
    /// Write to a file — line-based editing
    Write(write::Args),
    /// Search file contents or file names
    Search(search::Args),
    /// List project files and directory structure
    Ls(ls::Args),
    /// Show session history and command drill-down
    Log(log::Args),
    /// Analytical aggregates over events.ndjson
    Stats(stats::Args),
    /// Start MCP server
    Mcp,
}

impl Command {
    /// Apply per-command flag overrides on top of a pre-resolved `default` audience.
    ///
    /// Explicit flags (`--json`, `--human`, `--plain`) always win.
    /// When no flag is passed the caller-supplied `default` is returned unchanged.
    /// The default is resolved once at process entry (main.rs for CLI, handler.rs
    /// for MCP) — this function never reads environment variables.
    fn audience_with_default(&self, default: Audience) -> Audience {
        match self {
            Command::Build(a) => a.format.audience_with_default(default),
            Command::Check(a) => a.format.audience_with_default(default),
            Command::Fmt(a) => a.format.audience_with_default(default),
            Command::Test(a) => a.format.audience_with_default(default),
            Command::Read(a) => a.format.audience_with_default(default),
            Command::Write(a) => a.format.audience_with_default(default),
            Command::Search(a) => a.format.audience_with_default(default),
            Command::Ls(a) => a.format.audience_with_default(default),
            Command::Log(a) => a.format.audience_with_default(default),
            Command::Stats(a) => a.format.audience_with_default(default),
            Command::Init(a) => a.format.audience_with_default(default),
            Command::Hooks(_) | Command::Upgrade(_) | Command::Mcp => default,
        }
    }

    /// Resolve each variant's path field(s) against an MCP containment root.
    /// Each variant declares its own path semantics via its `Args` fields — the
    /// entry-point layer walks the typed enum, never matches on string names.
    pub fn resolve_mcp_paths(&mut self, root: &o8v_fs::ContainmentRoot) -> Result<(), String> {
        use crate::mcp::path::{resolve_optional_path, resolve_path, resolve_paths};
        match self {
            Command::Build(a) => resolve_path(&mut a.path, root),
            Command::Check(a) => resolve_optional_path(&mut a.path, root),
            Command::Fmt(a) => resolve_optional_path(&mut a.path, root),
            Command::Test(a) => resolve_path(&mut a.path, root),
            Command::Read(a) => resolve_paths(&mut a.paths, root),
            Command::Write(a) => resolve_path(&mut a.path, root),
            Command::Search(a) => resolve_optional_path(&mut a.path, root),
            Command::Ls(a) => resolve_optional_path(&mut a.path, root),
            Command::Init(_)
            | Command::Hooks(_)
            | Command::Upgrade(_)
            | Command::Log(_)
            | Command::Stats(_)
            | Command::Mcp => Ok(()),
        }
    }

    /// Human-readable command name for events.
    fn name(&self) -> &'static str {
        match self {
            Command::Build(_) => "build",
            Command::Check(_) => "check",
            Command::Fmt(_) => "fmt",
            Command::Init(_) => "init",
            Command::Hooks(_) => "hooks",
            Command::Test(_) => "test",
            Command::Upgrade(_) => "upgrade",
            Command::Read(_) => "read",
            Command::Write(_) => "write",
            Command::Search(_) => "search",
            Command::Ls(_) => "ls",
            Command::Log(_) => "log",
            Command::Stats(_) => "stats",
            Command::Mcp => "mcp",
        }
    }
}

/// Dispatch any command. One match, one place.
/// Both CLI and MCP call this.
///
/// Builds context, applies flag overrides on top of `default_audience`,
/// dispatches. Interfaces provide:
/// - The parsed command
/// - Who they are (Caller) — used for event recording
/// - The default audience resolved at process entry (`_8V_AGENT` is read there,
///   not here — never inside command logic)
/// - The interrupted flag
pub async fn dispatch_command(
    command: Command,
    caller: Caller,
    argv: Vec<String>,
    interrupted: &'static AtomicBool,
    default_audience: Audience,
) -> Result<(String, ExitCode, bool), CommandError> {
    dispatch_command_with_agent(command, caller, argv, interrupted, None, default_audience).await
}

pub async fn dispatch_command_with_agent(
    command: Command,
    caller: Caller,
    argv: Vec<String>,
    interrupted: &'static AtomicBool,
    agent_info: Option<o8v_core::caller::AgentInfo>,
    default_audience: Audience,
) -> Result<(String, ExitCode, bool), CommandError> {
    interrupted.store(false, std::sync::atomic::Ordering::Release);
    let mut ctx = crate::dispatch::build_context(interrupted);
    if let Some(info) = agent_info {
        ctx.extensions.insert(info);
    }
    let audience = command.audience_with_default(default_audience);
    let command_name = command.name();
    // Exit codes are CLI-specific — reports describe what happened,
    // this layer decides how to signal it to the shell.
    match command {
        Command::Build(args) => {
            let cmd = build::BuildCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.process.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((output, exit, false))
        }
        Command::Test(args) => {
            let cmd = test::TestCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.process.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((output, exit, false))
        }
        Command::Check(args) => {
            let use_stderr = audience == Audience::Human;
            let cmd = check::CheckCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.results().is_empty() && report.detection_errors().is_empty() {
                ExitCode::from(2u8)
            } else if report.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((output, exit, use_stderr))
        }
        Command::Fmt(args) => {
            let use_stderr = audience == Audience::Human;
            let cmd = fmt::FmtCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.entries.is_empty() && report.detection_errors.is_empty() {
                ExitCode::from(2u8)
            } else if report.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((output, exit, use_stderr))
        }
        Command::Hooks(args) => {
            let cmd = hooks::HooksCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(report.exit_code)
            };
            Ok((output, exit, audience == Audience::Human))
        }
        Command::Upgrade(args) => {
            let cmd = upgrade::UpgradeCommand { args };
            let (output, _, _) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Read(args) => {
            use o8v_core::render::read_report::{MultiResult, ReadReport};
            let cmd = read::ReadCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            // Batch-mode errors are inline in `Multi.entries`. The single-path
            // case already propagates errors via CommandError. Exit non-zero
            // if any entry failed, so agents can detect failure.
            let exit = match &report {
                ReadReport::Multi { entries }
                    if entries
                        .iter()
                        .any(|e| matches!(e.result, MultiResult::Err { .. })) =>
                {
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            };
            Ok((output, exit, false))
        }
        Command::Write(args) => {
            let cmd = write::WriteCommand { args };
            let (output, _, _) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Search(args) => {
            let cmd = search::SearchCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            // files_skipped counts files we couldn't read (permission, I/O);
            // binary content is filtered separately. Surface read failures via
            // exit code so agents notice instead of silently under-searching.
            let exit = if search::had_read_errors(&report) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
            Ok((output, exit, false))
        }
        Command::Ls(args) => {
            let cmd = ls::LsCommand { args };
            let (output, _, _) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Log(args) => {
            let cmd = log::LogCommand { args };
            let (output, _, _) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Stats(args) => {
            let cmd = stats::StatsCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            // Exit 2 only when the user supplied an explicit time filter that produced zero
            // matching events. Empty history with default args exits 0 (valid first-run state).
            let exit = if report.report.filtered_empty {
                ExitCode::from(2u8)
            } else {
                ExitCode::SUCCESS
            };
            Ok((output, exit, false))
        }
        Command::Init(args) => {
            let cmd = init::InitCommand { args };
            let (output, _, report) =
                crate::dispatch::dispatch(&cmd, &mut ctx, audience, caller, command_name, &argv)
                    .await?;
            let exit = if report.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((output, exit, false))
        }
        Command::Mcp => Err(CommandError::Execution("not a dispatchable command".into())),
    }
}
