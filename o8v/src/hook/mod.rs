// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Hook execution layer — captures native tool calls from Claude hooks
//! (PreToolUse / PostToolUse) and converts them into 8v events.
//!
//! Slice 1 ships the types and pure functions only. Slice 2c wires the
//! `hook pre` / `hook post` subcommands and adds E2E tests.

pub mod argv_map;
pub mod dispatch;
pub mod payload;
pub mod redact;
pub mod run_id;

use clap::{Parser, Subcommand};

/// Clap `Args` for `8v hook`. Hidden from top-level help.
#[derive(Debug, Parser)]
#[clap(hide = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: HookCommand,
}

/// Sub-subcommands accepted by `8v hook`.
#[derive(Debug, Subcommand)]
pub enum HookCommand {
    /// Handle a PreToolUse hook event (reads JSON from stdin).
    #[clap(hide = true)]
    Pre,
    /// Handle a PostToolUse hook event (reads JSON from stdin).
    #[clap(hide = true)]
    Post,
}
