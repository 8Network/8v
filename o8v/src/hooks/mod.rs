// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Hooks module — installs and dispatches git and Claude Code hook events.
//!
//! CLI dispatch:
//! ```text
//! 8v hooks git on-commit              — called by .git/hooks/pre-commit
//! 8v hooks git on-commit-msg <file>   — called by .git/hooks/commit-msg
//! 8v hooks claude pre-tool-use        — called by Claude Code PreToolUse hook
//! 8v hooks claude post-tool-use       — called by Claude Code PostToolUse hook
//! ... (all 8 events)
//! ```

pub mod claude;
pub mod git;
pub mod install;

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: HooksCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum HooksCommand {
    /// Git hook handlers (pre-commit, commit-msg)
    Git(git::Args),
    /// Claude Code hook handlers (pre-tool-use, post-tool-use, etc.)
    Claude(claude::Args),
}

// ─── Run ─────────────────────────────────────────────────────────────────────

/// Run hooks and return the numeric exit code.
/// Used by `HooksCommand` to populate `HooksReport.exit_code`.
pub fn run_code(args: &Args, interrupted: &'static AtomicBool) -> u8 {
    let code = match &args.command {
        HooksCommand::Git(args) => git::run(args, interrupted),
        HooksCommand::Claude(args) => claude::run(args),
    };
    // ExitCode is opaque in stable Rust — map known values via constants.
    if code == ExitCode::SUCCESS {
        crate::cli::common::EXIT_OK
    } else if code == ExitCode::from(crate::cli::common::EXIT_NOTHING) {
        crate::cli::common::EXIT_NOTHING
    } else if code == ExitCode::from(130) {
        crate::cli::common::EXIT_INTERRUPTED
    } else {
        crate::cli::common::EXIT_FAIL
    }
}
