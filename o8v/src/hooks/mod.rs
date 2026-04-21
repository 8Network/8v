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
pub(super) mod claude_install;
pub mod git;
pub(super) mod git_install;
pub mod install;

// ─── Shared helpers (used by git_install + claude_install) ───────────────────

/// POSIX sh single-quote escaping: `'...'` with internal `'` escaped as `'\''`.
/// Handles paths with spaces, parentheses, or any character except NUL.
pub(super) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Absolute path to the currently running 8v binary, shell-quoted for use in
/// installed hook scripts so they don't depend on PATH at hook fire time.
/// Falls back to the bare name `8v` (no quoting needed) if resolution fails.
pub(super) fn resolved_8v_command() -> String {
    match std::env::current_exe() {
        Ok(p) => match p.canonicalize() {
            Ok(abs) => shell_quote(&abs.to_string_lossy()),
            Err(_) => shell_quote(&p.to_string_lossy()),
        },
        Err(_) => "8v".to_string(),
    }
}

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(flatten)]
    pub format: crate::commands::output_format::OutputFormat,
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
