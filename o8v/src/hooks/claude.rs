// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Claude Code hook handlers — what runs when Claude Code fires a hook event.

use serde::Deserialize;
use std::io::Read;
use std::process::ExitCode;

// ─── Blocked tools ───────────────────────────────────────────────────────────

/// Tools blocked by default — agent should use 8v commands instead.
const BLOCKED_TOOLS: &[&str] = &["Read", "Edit", "Write", "Glob", "Grep"];

// ─── Stdin JSON ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HookInput {
    tool_name: Option<String>,
}

/// Read and parse hook input from stdin.
///
/// Returns `Err` on I/O failure or JSON parse failure so the caller can fail
/// closed (block) instead of failing open.
fn read_stdin_json() -> Result<HookInput, String> {
    let mut buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
        return Err(format!("failed to read hook input: {e}"));
    }
    serde_json::from_str(&buf).map_err(|e| format!("failed to parse hook input: {e}"))
}

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: ClaudeCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum ClaudeCommand {
    /// PreToolUse — block native tools, allow 8v tools
    PreToolUse,
    /// PostToolUse — log tool usage (noop for now)
    PostToolUse,
    /// PostToolUseFailure — log failures (noop for now)
    PostToolUseFailure,
    /// UserPromptSubmit — inject context (noop for now)
    UserPromptSubmit,
    /// SessionStart — initialize session (noop for now)
    SessionStart,
    /// Stop — record session summary (noop for now)
    Stop,
    /// SubagentStart — log subagent spawn (noop for now)
    SubagentStart,
    /// SubagentStop — log subagent completion (noop for now)
    SubagentStop,
}

// ─── Run ─────────────────────────────────────────────────────────────────────

pub fn run(args: &Args) -> ExitCode {
    match &args.command {
        ClaudeCommand::PreToolUse => pre_tool_use(),
        ClaudeCommand::PostToolUse => ExitCode::SUCCESS,
        ClaudeCommand::PostToolUseFailure => ExitCode::SUCCESS,
        ClaudeCommand::UserPromptSubmit => ExitCode::SUCCESS,
        ClaudeCommand::SessionStart => ExitCode::SUCCESS,
        ClaudeCommand::Stop => ExitCode::SUCCESS,
        ClaudeCommand::SubagentStart => ExitCode::SUCCESS,
        ClaudeCommand::SubagentStop => ExitCode::SUCCESS,
    }
}

/// PreToolUse: read tool_name from stdin JSON, block if in blocked list.
///
/// Exit 2 to block. Exit 0 to allow.
/// Fails closed: parse errors, null tool_name, and empty tool_name all exit 2.
fn pre_tool_use() -> ExitCode {
    let input = match read_stdin_json() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: hook input invalid ({e}) — blocking by default");
            return ExitCode::from(2);
        }
    };

    let tool_name = match input.tool_name {
        Some(ref name) if !name.is_empty() => name.as_str(),
        Some(_) => {
            eprintln!("error: hook input invalid (tool_name is empty) — blocking by default");
            return ExitCode::from(2);
        }
        None => {
            eprintln!(
                "error: hook input invalid (tool_name is null or missing) — blocking by default"
            );
            return ExitCode::from(2);
        }
    };

    if BLOCKED_TOOLS.contains(&tool_name) {
        eprintln!(
            "Blocked: use 8v read, 8v write, 8v check, 8v fmt, 8v test instead of native tools."
        );
        // Claude Code protocol: exit 2 = block tool use, exit 1 = hook error.
        return ExitCode::from(2);
    }

    ExitCode::SUCCESS
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_tools_list_is_not_empty() {
        assert!(!BLOCKED_TOOLS.is_empty());
    }

    #[test]
    fn known_tools_are_blocked() {
        for tool in &["Read", "Edit", "Write", "Glob", "Grep"] {
            assert!(
                BLOCKED_TOOLS.contains(tool),
                "expected {tool} to be in BLOCKED_TOOLS"
            );
        }
    }

    #[test]
    fn unknown_tool_is_not_blocked() {
        for tool in &["mcp__8v__8v", "TodoWrite", "Bash", "Agent", "NotebookEdit"] {
            assert!(
                !BLOCKED_TOOLS.contains(tool),
                "expected {tool} NOT to be in BLOCKED_TOOLS"
            );
        }
    }
}
