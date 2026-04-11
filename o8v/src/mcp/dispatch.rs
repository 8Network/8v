// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP command dispatch — maps `cli::Command` variants to typed command structs
//! and dispatches them through the o8v dispatch pipeline.
//!
//! Init, Hooks, Upgrade, and Mcp are unavailable in the MCP context and return
//! an error. All other commands are dispatched normally.

use crate::cli::Command;
use o8v_core::command::{CommandContext as CoreCommandContext, CommandError};
use o8v_core::render::Audience;

/// Dispatch a `Command` variant: construct the typed command struct and
/// call the generic `dispatch()` function, returning the rendered output string.
///
/// Takes `Command` by value because the Args structs do not implement `Clone`.
///
/// # Errors
///
/// - Init, Hooks, Upgrade: not available in this context (MCP, agent).
/// - Mcp: cannot nest MCP servers.
/// - Any other dispatch failure propagates as `CommandError`.
pub async fn run(
    command: Command,
    ctx: &CoreCommandContext,
    audience: Audience,
) -> Result<String, CommandError> {
    match command {
        Command::Check(args) => {
            let cmd = crate::commands::check::CheckCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Fmt(args) => {
            let cmd = crate::commands::fmt::FmtCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Read(args) => {
            let cmd = crate::commands::read::ReadCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Write(args) => {
            let cmd = crate::commands::write::WriteCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Test(args) => {
            let cmd = crate::commands::test::TestCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Search(args) => {
            let cmd = crate::commands::search::SearchCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Ls(args) => {
            let cmd = crate::commands::ls::LsCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Run(args) => {
            let cmd = crate::commands::run::RunCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Build(args) => {
            let cmd = crate::commands::build::BuildCommand { args };
            let (output, _, _) = o8v::dispatch::dispatch(&cmd, ctx, audience).await?;
            Ok(output)
        }
        Command::Init(_) => Err(CommandError::Execution(
            "init not available in this context".into(),
        )),
        Command::Hooks(_) => Err(CommandError::Execution(
            "hooks not available in this context".into(),
        )),
        Command::Upgrade(_) => Err(CommandError::Execution(
            "upgrade not available in this context".into(),
        )),
        Command::Mcp => Err(CommandError::Execution("cannot nest MCP servers".into())),
    }
}
