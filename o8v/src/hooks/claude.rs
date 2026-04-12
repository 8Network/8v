// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Claude Code hook handlers — what runs when Claude Code fires a hook event.

use serde::Deserialize;
use std::io::Read;
use std::process::ExitCode;

// ─── Blocked tools ───────────────────────────────────────────────────────────

/// Tools blocked by default — agent should use 8v commands instead.
const BLOCKED_TOOLS: &[&str] = &[
    "Read",
    "Edit",
    "Write",
    "Bash",
    "Glob",
    "Grep",
    "Agent",
    "NotebookEdit",
];

// ─── Stdin JSON ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct HookInput {
    #[serde(default)]
    tool_name: String,
}

fn read_stdin_json() -> HookInput {
    let mut buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
        eprintln!("8v: failed to read hook input: {e}");
        return HookInput::default();
    }
    let input: HookInput = match serde_json::from_str(&buf) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("8v: failed to parse hook input: {e}");
            return HookInput::default(); // don't block on parse failure
        }
    };
    input
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
fn pre_tool_use() -> ExitCode {
    let input = read_stdin_json();

    if input.tool_name.is_empty() {
        // No tool name — can't decide, allow through.
        return ExitCode::SUCCESS;
    }

    if BLOCKED_TOOLS.contains(&input.tool_name.as_str()) {
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
        for tool in &[
            "Read",
            "Edit",
            "Write",
            "Bash",
            "Glob",
            "Grep",
            "Agent",
            "NotebookEdit",
        ] {
            assert!(
                BLOCKED_TOOLS.contains(tool),
                "expected {tool} to be in BLOCKED_TOOLS"
            );
        }
    }

    #[test]
    fn unknown_tool_is_not_blocked() {
        assert!(!BLOCKED_TOOLS.contains(&"mcp__8v__8v"));
        assert!(!BLOCKED_TOOLS.contains(&"TodoWrite"));
    }
}
