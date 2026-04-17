// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Codex CLI driver — spawns the agent and parses the JSONL output.
//!
//! Parallel to `claude.rs`. Produces the same `AgentResult` so the
//! benchmark pipeline, experiment runner, and report builder work
//! unchanged.

use super::claude::{AgentResult, ToolCall, TurnUsage};
use super::types::ToolCallDetail;
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Stdio};

// ── Codex JSONL stream types ────────────────────────────────────────────────

/// Exit code used when a process was killed by a signal.
/// Signal-killed processes have no numeric exit code; the OS reports None.
const SIGNAL_KILLED: i32 = -1;

/// Top-level event types emitted by `codex exec --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CodexEventType {
    #[serde(rename = "thread.started")]
    ThreadStarted,
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.completed")]
    TurnCompleted,
    #[serde(rename = "item.started")]
    ItemStarted,
    #[serde(rename = "item.completed")]
    ItemCompleted,
    /// Any event type not listed above.
    #[serde(other)]
    Unknown,
}

/// Item types carried inside `item.started` / `item.completed` events.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum CodexItemType {
    AgentMessage,
    CommandExecution,
    McpToolCall,
    /// Any item type not listed above.
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct CodexEvent {
    #[serde(rename = "type")]
    event_type: CodexEventType,

    // thread.started
    #[serde(default)]
    thread_id: Option<String>,

    // turn.completed
    #[serde(default)]
    usage: Option<CodexUsage>,

    // item.started / item.completed
    #[serde(default)]
    item: Option<CodexItem>,
}

#[derive(Debug, Deserialize)]
struct CodexUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct CodexItem {
    #[serde(default)]
    _id: String,
    #[serde(rename = "type", default)]
    item_type: CodexItemType,

    // agent_message
    #[serde(default)]
    text: Option<String>,

    // command_execution
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    aggregated_output: Option<String>,
    #[serde(default)]
    exit_code: Option<i32>,
    #[serde(default)]
    status: Option<String>,

    // mcp_tool_call
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    arguments: Option<serde_json::Value>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

// ── Driver ──────────────────────────────────────────────────────────────────

pub struct CodexDriver;

impl CodexDriver {
    /// Spawn Codex with the given configuration.
    ///
    /// Uses `codex exec --json --ephemeral` for non-interactive,
    /// JSONL-output execution. Parses into AgentResult.
    ///
    /// When `disable_shell` is true, the shell tool is disabled
    /// (`--disable shell_tool`) so Codex must use MCP tools exclusively.
    /// MCP approval requires `--dangerously-bypass-approvals-and-sandbox`
    /// (Codex v0.121.0 limitation — `--full-auto` cancels MCP calls).
    pub fn run(
        prompt: &str,
        working_dir: &Path,
        disable_shell: bool,
    ) -> Result<AgentResult, String> {
        let mut args = vec![
            "exec",
            "--json",
            "--dangerously-bypass-approvals-and-sandbox",
            "--ephemeral",
        ];
        if disable_shell {
            args.push("--disable");
            args.push("shell_tool");
        }
        args.push(prompt);

        eprintln!("  [spawn] codex {}", args.join(" "));

        let mut cmd = Command::new("codex");
        cmd.args(&args)
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn codex: {e} (is `codex` in PATH?)"))?;

        let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            let stderr_lines: Vec<&str> = stderr.lines().collect();
            eprintln!("  [codex stderr] {} lines total", stderr_lines.len());
            for line in stderr_lines.iter().take(30) {
                eprintln!("  [codex stderr] {line}");
            }
            if stderr_lines.len() > 30 {
                eprintln!(
                    "  [codex stderr] ... ({} more lines)",
                    stderr_lines.len() - 30
                );
            }
        }

        Self::parse_output(
            &output.stdout,
            output.status.code().unwrap_or(SIGNAL_KILLED),
        )
    }

    fn parse_output(stdout: &[u8], exit_code: i32) -> Result<AgentResult, String> {
        let stdout = String::from_utf8_lossy(stdout);
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut tool_calls_detail: Vec<ToolCallDetail> = Vec::new();
        let mut response_text = String::new();
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut total_cached_tokens: u64 = 0;
        let mut turn_usage: Vec<TurnUsage> = Vec::new();
        let mut session_id: Option<String> = None;
        let mut parse_errors: usize = 0;
        let mut unknown_events: u32 = 0;

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event = match serde_json::from_str::<CodexEvent>(line) {
                Ok(e) => e,
                Err(e) => {
                    parse_errors += 1;
                    eprintln!(
                        "  [codex] parse error #{parse_errors}: {e} — line: {}",
                        &line[..line.len().min(200)]
                    );
                    continue;
                }
            };

            match event.event_type {
                CodexEventType::ThreadStarted => {
                    session_id = event.thread_id;
                }
                CodexEventType::TurnCompleted => {
                    if let Some(usage) = event.usage {
                        total_input_tokens += usage.input_tokens;
                        total_output_tokens += usage.output_tokens;
                        total_cached_tokens += usage.cached_input_tokens;
                        turn_usage.push(TurnUsage {
                            role: "turn".to_string(),
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cache_read_input_tokens: usage.cached_input_tokens,
                            cache_creation_input_tokens: 0,
                        });
                    }
                }
                CodexEventType::ItemCompleted => {
                    if let Some(item) = event.item {
                        match item.item_type {
                            CodexItemType::AgentMessage => {
                                if let Some(text) = item.text {
                                    if !response_text.is_empty() {
                                        response_text.push('\n');
                                    }
                                    response_text.push_str(&text);
                                }
                            }
                            CodexItemType::CommandExecution => {
                                let name = "shell".to_string();
                                let input = item
                                    .command
                                    .clone()
                                    .unwrap_or_else(|| "[no command]".to_string());
                                let output_bytes = item
                                    .aggregated_output
                                    .as_ref()
                                    .map(|s| s.len() as u64)
                                    .unwrap_or(0);
                                let is_error = item.status.as_deref() == Some("failed")
                                    || item.exit_code.map(|c| c != 0).unwrap_or(false);

                                tool_calls.push(ToolCall { name: name.clone() });
                                tool_calls_detail.push(ToolCallDetail {
                                    name,
                                    input,
                                    output_bytes,
                                    is_error,
                                });
                            }
                            CodexItemType::McpToolCall => {
                                let name = match (&item.server, &item.tool) {
                                    (Some(s), Some(t)) => format!("mcp__{s}__{t}"),
                                    (Some(s), None) => format!("mcp__{s}"),
                                    _ => "mcp_unknown".to_string(),
                                };
                                let input = item
                                    .arguments
                                    .map(|v| {
                                        serde_json::to_string(&v)
                                            .expect("serialize serde_json::Value")
                                    })
                                    .unwrap_or_default();
                                let output_bytes = item
                                    .result
                                    .as_ref()
                                    .map(|v| {
                                        serde_json::to_string(v)
                                            .expect("serialize serde_json::Value")
                                            .len() as u64
                                    })
                                    .unwrap_or(0);
                                let is_error = item.error.is_some()
                                    || item.status.as_deref() == Some("failed");

                                tool_calls.push(ToolCall { name: name.clone() });
                                tool_calls_detail.push(ToolCallDetail {
                                    name,
                                    input,
                                    output_bytes,
                                    is_error,
                                });
                            }
                            CodexItemType::Unknown => {
                                unknown_events += 1;
                            }
                        }
                    }
                }
                CodexEventType::TurnStarted | CodexEventType::ItemStarted => {
                    // no data to extract from these events
                }
                CodexEventType::Unknown => {
                    unknown_events += 1;
                }
            }
        }

        if unknown_events > 0 {
            eprintln!("  [codex] {unknown_events} unknown event(s)/item(s) dropped — check JSONL format version");
        }

        let total_tokens = total_input_tokens + total_output_tokens + total_cached_tokens;

        Ok(AgentResult {
            tool_calls,
            tool_calls_detail,
            response_text,
            total_tokens,
            cost_usd: None, // Codex JSONL doesn't include cost
            exit_code,
            turn_usage,
            init_message_bytes: 0,
            model: None, // Codex JSONL doesn't include model ID
            session_id,
            stop_reason: None,
            is_error: exit_code != 0,
            cache_read_tokens: total_cached_tokens,
            cache_creation_tokens: 0,
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            parse_errors: parse_errors as u32,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_jsonl() -> &'static str {
        r#"{"type":"thread.started","thread_id":"abc-123"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I will read the file."}}
{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"/bin/zsh -lc \"cat main.rs\"","aggregated_output":"fn main() {}\n","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_2","type":"mcp_tool_call","server":"8v","tool":"8v","arguments":{"command":"8v read main.rs"},"result":"fn main() {}","error":null,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"The file defines main."}}
{"type":"turn.completed","usage":{"input_tokens":1000,"output_tokens":200,"cached_input_tokens":500}}"#
    }

    #[test]
    fn parse_session_id() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert_eq!(result.session_id.as_deref(), Some("abc-123"));
    }

    #[test]
    fn parse_tokens() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert_eq!(result.input_tokens, 1000);
        assert_eq!(result.output_tokens, 200);
        assert_eq!(result.cache_read_tokens, 500);
        assert_eq!(result.total_tokens, 1700);
    }

    #[test]
    fn parse_tool_calls() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].name, "shell");
        assert_eq!(result.tool_calls[1].name, "mcp__8v__8v");
    }

    #[test]
    fn parse_response_text() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert!(result.response_text.contains("I will read the file."));
        assert!(result.response_text.contains("The file defines main."));
    }

    #[test]
    fn parse_tool_detail() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert_eq!(result.tool_calls_detail.len(), 2);

        let shell = &result.tool_calls_detail[0];
        assert_eq!(shell.name, "shell");
        assert!(shell.output_bytes > 0);
        assert!(!shell.is_error);

        let mcp = &result.tool_calls_detail[1];
        assert_eq!(mcp.name, "mcp__8v__8v");
        assert!(!mcp.is_error);
    }

    #[test]
    fn parse_failed_command() {
        let jsonl = r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"false","aggregated_output":"","exit_code":1,"status":"completed"}}
{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":50,"cached_input_tokens":0}}"#;

        let result = CodexDriver::parse_output(jsonl.as_bytes(), 0).unwrap();
        assert!(result.tool_calls_detail[0].is_error);
    }

    #[test]
    fn parse_failed_mcp() {
        let jsonl = r#"{"type":"item.completed","item":{"id":"item_0","type":"mcp_tool_call","server":"8v","tool":"8v","arguments":{},"result":null,"error":{"message":"user cancelled"},"status":"failed"}}
{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":50,"cached_input_tokens":0}}"#;

        let result = CodexDriver::parse_output(jsonl.as_bytes(), 0).unwrap();
        assert!(result.tool_calls_detail[0].is_error);
        assert_eq!(result.tool_calls_detail[0].name, "mcp__8v__8v");
    }

    #[test]
    fn no_cost_or_model() {
        let result = CodexDriver::parse_output(sample_jsonl().as_bytes(), 0).unwrap();
        assert!(result.cost_usd.is_none());
        assert!(result.model.is_none());
    }

    #[test]
    fn empty_output() {
        let result = CodexDriver::parse_output(b"", 0).unwrap();
        assert_eq!(result.total_tokens, 0);
        assert!(result.tool_calls.is_empty());
        assert!(result.response_text.is_empty());
    }
}
