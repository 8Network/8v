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
pub mod ls;
pub mod read;
pub mod run;
pub mod search;
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
pub(crate) enum Command {
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
    /// Run a command with containment, timeout, and structured output
    Run(run::Args),
    /// Search file contents or file names
    Search(search::Args),
    /// List project files and directory structure
    Ls(ls::Args),
    /// Start MCP server
    Mcp,
}

impl Command {
    /// Audience from CLI flags. MCP overrides this — caller determines default.
    fn cli_audience(&self) -> Audience {
        match self {
            Command::Build(a) => a.audience(),
            Command::Check(a) => a.audience(),
            Command::Fmt(a) => a.audience(),
            Command::Test(a) => a.audience(),
            Command::Read(a) => a.audience(),
            Command::Write(a) => a.audience(),
            Command::Run(a) => a.audience(),
            Command::Search(a) => a.audience(),
            Command::Ls(a) => a.audience(),
            Command::Init(_) | Command::Hooks(_) | Command::Upgrade(_) | Command::Mcp => {
                Audience::Human
            }
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
            Command::Run(_) => "run",
            Command::Search(_) => "search",
            Command::Ls(_) => "ls",
            Command::Mcp => "mcp",
        }
    }
}

/// Dispatch any command. One match, one place.
/// Both CLI and MCP call this.
///
/// Builds context, derives audience, dispatches. Interfaces provide only:
/// - The parsed command
/// - Who they are (Caller)
/// - The interrupted flag
pub(crate) async fn dispatch_command(
    command: Command,
    caller: Caller,
    interrupted: &'static AtomicBool,
) -> Result<(String, ExitCode, bool), CommandError> {
    let ctx = o8v::dispatch::build_context(interrupted);
    let audience = match caller {
        Caller::Mcp => Audience::Agent,
        Caller::Cli => command.cli_audience(),
    };
    let command_name = command.name();

    match command {
        Command::Build(args) => {
            let cmd = build::BuildCommand { args };
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            let exit = if report.process.success { ExitCode::SUCCESS } else { ExitCode::FAILURE };
            Ok((output, exit, false))
        }
        Command::Test(args) => {
            let cmd = test::TestCommand { args };
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            let exit = if report.process.success { ExitCode::SUCCESS } else { ExitCode::FAILURE };
            Ok((output, exit, false))
        }
        Command::Run(args) => {
            let cmd = run::RunCommand { args };
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            let exit = if report.process.success { ExitCode::SUCCESS } else { ExitCode::FAILURE };
            Ok((output, exit, false))
        }
        Command::Check(args) => {
            let use_stderr = audience == Audience::Human;
            let cmd = check::CheckCommand { args };
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
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
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
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
            let (output, _, report) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            let exit = if report.success { ExitCode::SUCCESS } else { ExitCode::from(report.exit_code) };
            Ok((output, exit, true))
        }
        Command::Upgrade(args) => {
            let cmd = upgrade::UpgradeCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Read(args) => {
            let cmd = read::ReadCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Write(args) => {
            let cmd = write::WriteCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Search(args) => {
            let cmd = search::SearchCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Ls(args) => {
            let cmd = ls::LsCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, &ctx, audience, caller, command_name).await?;
            Ok((output, ExitCode::SUCCESS, false))
        }
        Command::Init(_) | Command::Mcp => {
            Err(CommandError::Execution("not a dispatchable command".into()))
        }
    }
}
