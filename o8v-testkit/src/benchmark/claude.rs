// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Claude CLI driver — spawns the agent and parses the stream-json output.
//!
//! This is the "external data" source: tool calls, tokens, cost, response text.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use serde::Deserialize;
use super::types::ToolCallDetail;

// ── Claude CLI JSONL stream types ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeStreamMsg {
    System(ClaudeSystemMsg),
    Assistant(ClaudeAssistantMsg),
    User(ClaudeUserMsg),
    Result(ClaudeResultMsg),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct ClaudeSystemMsg {
    subtype: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeAssistantMsg {
    message: ClaudeMessage,
}

#[derive(Debug, Deserialize)]
struct ClaudeUserMsg {
    message: ClaudeUserMessage,
}

#[derive(Debug, Deserialize)]
struct ClaudeUserMessage {
    #[serde(default)]
    content: Vec<ClaudeUserContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeUserContentBlock {
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: ToolResultContent,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum ToolResultContent {
    Text(String),
    Blocks(Vec<ToolResultBlock>),
    #[default]
    None,
}

impl ToolResultContent {
    fn byte_len(&self) -> usize {
        match self {
            ToolResultContent::Text(s) => s.len(),
            ToolResultContent::Blocks(blocks) => blocks.iter().map(|b| b.text.len()).sum(),
            ToolResultContent::None => 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolResultBlock {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    usage: ClaudeUsage,
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeContentBlock {
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Text {
        text: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct ClaudeResultMsg {
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    usage: Option<ClaudeResultUsage>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    is_error: bool,
}

#[derive(Debug, Deserialize)]
struct ClaudeResultUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

// ── Public types ─────────────────────────────────────────────────────────────

/// A single tool invocation recorded from agent output.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    /// Raw JSON of the tool input. Written for record-keeping; read by test code.
    #[allow(dead_code)]
    pub input: String,
}

/// Per-turn token breakdown from the Claude API response.
#[derive(Debug, Clone)]
pub struct TurnUsage {
    pub role: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// The parsed result of a Claude CLI session.
#[derive(Debug)]
pub struct AgentResult {
    pub tool_calls: Vec<ToolCall>,
    /// Full detail for each tool invocation: name, serialized input, output size, error flag.
    pub tool_calls_detail: Vec<ToolCallDetail>,
    pub response_text: String,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub exit_code: i32,
    pub turn_usage: Vec<TurnUsage>,
    pub init_message_bytes: usize,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub stop_reason: Option<String>,
    pub is_error: bool,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl AgentResult {
    /// Whether the agent used any tool with "8v" in its name.
    pub fn used_8v(&self) -> bool {
        self.tool_calls.iter().any(|t| t.name.contains("8v"))
    }

    /// Total number of tool calls.
    #[allow(dead_code)]
    pub fn tool_call_count(&self) -> usize {
        self.tool_calls.len()
    }

    /// Whether the agent used any of the specified native tools.
    #[allow(dead_code)]
    pub fn used_native_tools(&self, native: &[&str]) -> bool {
        self.tool_calls.iter().any(|t| native.contains(&t.name.as_str()))
    }

    /// Returns true if the agent response mentions any of the given violations.
    #[allow(dead_code)]
    pub fn surfaces_violations(&self, violations: &[String]) -> bool {
        if violations.is_empty() {
            return true;
        }
        let text = self.response_text.to_lowercase();
        violations.iter().any(|v| {
            let v = v.to_lowercase();
            if text.contains(&v) {
                return true;
            }
            match v.as_str() {
                "cargo fmt" => text.contains("fmt") || text.contains("format") || text.contains("rustfmt"),
                "clippy" => text.contains("clippy") || text.contains("lint"),
                "cargo check" => text.contains("compile") || text.contains("cargo check"),
                _ => false,
            }
        })
    }
}

/// Drives the Claude CLI process.
pub struct ClaudeDriver;

impl ClaudeDriver {
    /// Spawn Claude with the given configuration.
    ///
    /// Parses the stream-json output into an AgentResult.
    pub fn run(
        prompt: &str,
        working_dir: &Path,
        mcp_config: Option<&Path>,
        permission_mode: &str,
        blocked_tools: &[&str],
        env_vars: &[(&str, &str)],
        settings_path: Option<&Path>,
    ) -> Result<AgentResult, String> {
        let mut args = vec![
            "--output-format", "stream-json",
            "--input-format", "stream-json",
            "--verbose",
            "--permission-mode", permission_mode,
        ];

        let mcp_path_str;
        if let Some(config) = mcp_config {
            mcp_path_str = config.to_str()
                .ok_or_else(|| "path is not valid UTF-8".to_string())?
                .to_string();
            args.push("--mcp-config");
            args.push(&mcp_path_str);
        }

        for tool in blocked_tools {
            args.push("--disallowedTools");
            args.push(tool);
        }

        let settings_path_str;
        if let Some(settings) = settings_path {
            settings_path_str = settings.to_str()
                .ok_or_else(|| "path is not valid UTF-8".to_string())?
                .to_string();
            args.push("--settings");
            args.push(&settings_path_str);
        }

        eprintln!("  [spawn] claude {}", args.join(" "));

        let mut cmd = Command::new("claude");
        cmd.args(&args)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        let mut child = cmd.spawn()
            .map_err(|e| format!("failed to spawn claude: {e} (is `claude` in PATH?)"))?;

        {
            let mut stdin = child.stdin.take().ok_or_else(|| "stdin not available".to_string())?;
            let msg = serde_json::json!({
                "type": "user",
                "message": { "role": "user", "content": prompt }
            });
            writeln!(stdin, "{}", serde_json::to_string(&msg).expect("serialize"))
                .map_err(|e| format!("write stdin: {e}"))?;
        }

        let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            let stderr_lines: Vec<&str> = stderr.lines().collect();
            eprintln!("  [claude stderr] {} lines total", stderr_lines.len());
            for line in stderr_lines.iter().take(30) {
                eprintln!("  [claude stderr] {line}");
            }
            if stderr_lines.len() > 30 {
                eprintln!("  [claude stderr] ... ({} more lines)", stderr_lines.len() - 30);
            }
        }

        Self::parse_output(&output.stdout, output.status.code().unwrap_or(-1))
    }

    fn parse_output(stdout: &[u8], exit_code: i32) -> Result<AgentResult, String> {
        let stdout = String::from_utf8_lossy(stdout);
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut tool_calls_detail: Vec<ToolCallDetail> = Vec::new();
        let mut response_text = String::new();
        let mut total_tokens: u64 = 0;
        let mut cost_usd: Option<f64> = None;
        let mut turn_usage: Vec<TurnUsage> = Vec::new();
        let mut init_message_bytes: usize = 0;
        let mut model: Option<String> = None;
        let mut session_id: Option<String> = None;
        let mut stop_reason: Option<String> = None;
        let mut is_error: bool = false;
        let mut cache_read_tokens: u64 = 0;
        let mut cache_creation_tokens: u64 = 0;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut tool_use_index: HashMap<String, usize> = HashMap::new();

        for line in stdout.lines() {
            let Ok(msg) = serde_json::from_str::<ClaudeStreamMsg>(line) else {
                eprintln!("  [benchmark] warning: unparseable stream line: {}", line);
                continue;
            };
            match msg {
                ClaudeStreamMsg::System(sys) if sys.subtype == "init" => {
                    init_message_bytes = line.len();
                    session_id = sys.session_id;
                }
                ClaudeStreamMsg::Assistant(asst) => {
                    let usage = &asst.message.usage;
                    let turn_input = usage.input_tokens;
                    let turn_output = usage.output_tokens;
                    let cache_read = usage.cache_read_input_tokens;
                    let cache_creation = usage.cache_creation_input_tokens;

                    for block in asst.message.content {
                        match block {
                            ClaudeContentBlock::ToolUse { id, name, input } => {
                                let input_str = serde_json::to_string(&input)
                                    .map_err(|e| format!("failed to serialize tool input: {e}"))?;
                                tool_calls.push(ToolCall { name: name.clone(), input: input_str.clone() });
                                let idx = tool_calls_detail.len();
                                tool_calls_detail.push(ToolCallDetail {
                                    name: name.clone(),
                                    input: input_str,
                                    output_bytes: 0,
                                    is_error: false,
                                });
                                tool_use_index.insert(id, idx);
                                turn_usage.push(TurnUsage {
                                    role: name,
                                    input_tokens: turn_input,
                                    output_tokens: turn_output,
                                    cache_read_input_tokens: cache_read,
                                    cache_creation_input_tokens: cache_creation,
                                });
                            }
                            ClaudeContentBlock::Text { text } => {
                                response_text.push_str(&text);
                                turn_usage.push(TurnUsage {
                                    role: "text".to_string(),
                                    input_tokens: turn_input,
                                    output_tokens: turn_output,
                                    cache_read_input_tokens: cache_read,
                                    cache_creation_input_tokens: cache_creation,
                                });
                            }
                            ClaudeContentBlock::Unknown => {}
                        }
                    }
                }
                ClaudeStreamMsg::User(user) => {
                    for block in user.message.content {
                        if let ClaudeUserContentBlock::ToolResult { tool_use_id, content, is_error: tr_err } = block {
                            if let Some(&idx) = tool_use_index.get(&tool_use_id) {
                                if let Some(detail) = tool_calls_detail.get_mut(idx) {
                                    detail.output_bytes = content.byte_len() as u64;
                                    detail.is_error = tr_err;
                                }
                            }
                        }
                    }
                }
                ClaudeStreamMsg::Result(res) => {
                    if let Some(text) = res.result {
                        response_text.push_str(&text);
                    }
                    if let Some(u) = res.usage {
                        input_tokens = u.input_tokens;
                        output_tokens = u.output_tokens;
                        cache_read_tokens = u.cache_read_input_tokens;
                        cache_creation_tokens = u.cache_creation_input_tokens;
                        total_tokens += u.input_tokens;
                        total_tokens += u.output_tokens;
                        total_tokens += u.cache_read_input_tokens;
                    }
                    cost_usd = res.total_cost_usd;
                    model = res.model;
                    stop_reason = res.stop_reason;
                    is_error = res.is_error;
                }
                _ => {}
            }
        }

        if total_tokens == 0 && !turn_usage.is_empty() {
            eprintln!(
                "  [benchmark] warning: total_tokens is 0 despite {} turns — Result message may be missing usage",
                turn_usage.len()
            );
        }

        Ok(AgentResult {
            tool_calls,
            tool_calls_detail,
            response_text,
            total_tokens,
            cost_usd,
            exit_code,
            turn_usage,
            init_message_bytes,
            model,
            session_id,
            stop_reason,
            is_error,
            cache_read_tokens,
            cache_creation_tokens,
            input_tokens,
            output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_captures_tool_result_bytes_string_form() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#, "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"tu_1","name":"Read","input":{"file":"x"}}]}}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"hello world","is_error":false}]}}"#, "\n",
            r#"{"type":"result","result":"done","is_error":false}"#, "\n",
        );
        let res = ClaudeDriver::parse_output(stream.as_bytes(), 0).unwrap();
        assert_eq!(res.tool_calls_detail.len(), 1);
        assert_eq!(res.tool_calls_detail[0].name, "Read");
        assert_eq!(res.tool_calls_detail[0].output_bytes, "hello world".len() as u64);
        assert!(!res.tool_calls_detail[0].is_error);
    }

    #[test]
    fn parse_output_captures_tool_result_bytes_block_form_and_error() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#, "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"tu_a","name":"Bash","input":{"cmd":"x"}}]}}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_a","content":[{"type":"text","text":"abc"},{"type":"text","text":"def"}],"is_error":true}]}}"#, "\n",
            r#"{"type":"result","result":"","is_error":false}"#, "\n",
        );
        let res = ClaudeDriver::parse_output(stream.as_bytes(), 0).unwrap();
        assert_eq!(res.tool_calls_detail.len(), 1);
        assert_eq!(res.tool_calls_detail[0].output_bytes, 6);
        assert!(res.tool_calls_detail[0].is_error);
    }
}
