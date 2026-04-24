// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Claude CLI driver — spawns the agent and parses the stream-json output.
//!
//! This is the "external data" source: tool calls, tokens, cost, response text.

use super::profiles::ProfileArtifacts;
use super::types::{PermissionMode, ToolCallDetail};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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

const SIGNAL_KILLED: i32 = -1;

/// A single tool invocation recorded from agent output.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
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
    /// Number of JSONL lines that failed to parse. Non-zero indicates a corrupt or unexpected stream.
    pub parse_errors: u32,
}

impl AgentResult {
    /// Whether the agent used any tool with "8v" in its name.
    pub fn used_8v(&self) -> bool {
        self.tool_calls.iter().any(|t| t.name.contains("8v"))
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
        permission_mode: Option<PermissionMode>,
        settings_path: Option<&Path>,
        artifacts: &ProfileArtifacts,
        provenance: Option<&super::provenance::Provenance>,
    ) -> Result<AgentResult, String> {
        // ── Apply ProfileArtifacts before spawning ──────────────────────────
        // MCP json merge: if fragment present, merge its mcpServers into the
        // existing .mcp.json file.
        if let (Some(frag), Some(mcp_path)) = (&artifacts.mcp_json_fragment, mcp_config) {
            apply_mcp_fragment(mcp_path, frag)
                .map_err(|e| format!("failed to merge mcp_json_fragment: {e}"))?;
        }
        // CLAUDE.md prepend
        if let Some(prepend) = &artifacts.claude_md_prepend {
            let claude_md = working_dir.join("CLAUDE.md");
            apply_claude_md_prepend(&claude_md, prepend)
                .map_err(|e| format!("failed to prepend CLAUDE.md: {e}"))?;
        }
        let mut args = vec![
            "--output-format",
            "stream-json",
            "--input-format",
            "stream-json",
            "--verbose",
        ];
        if let Some(mode) = permission_mode {
            args.push("--permission-mode");
            args.push(mode.as_str());
        }

        let mcp_path_str;
        if let Some(config) = mcp_config {
            mcp_path_str = config
                .to_str()
                .ok_or_else(|| "path is not valid UTF-8".to_string())?
                .to_string();
            args.push("--mcp-config");
            args.push(&mcp_path_str);
        }

        let settings_path_str;
        if let Some(settings) = settings_path {
            settings_path_str = settings
                .to_str()
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
            .stderr(Stdio::piped())
            // Remove the cargo-test isolation override so the spawned MCP server
            // falls back to HOME and writes to ~/.8v/events.ndjson — making
            // benchmark sessions visible in `8v log` and `8v stats`.
            .env_remove("_8V_HOME");
        for (k, v) in &artifacts.env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn claude: {e} (is `claude` in PATH?)"))?;

        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "stdin not available".to_string())?;
            let msg = serde_json::json!({
                "type": "user",
                "message": { "role": "user", "content": prompt }
            });
            let msg_str = serde_json::to_string(&msg)
                .map_err(|e| format!("failed to serialize stdin message: {e}"))?;
            writeln!(stdin, "{}", msg_str).map_err(|e| format!("write stdin: {e}"))?;
        }

        let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

        if let Ok(transcript_dir) = std::env::var("BENCH_TRANSCRIPT_DIR") {
            let ts_result = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
            let ts = match ts_result {
                Ok(d) => d.as_millis(),
                Err(_) => 0,
            };
            let fname = if let Some(prov) = provenance {
                let short = &prov.provenance_id()[..8];
                format!("{}/{}-{}.jsonl", transcript_dir, ts, short)
            } else {
                format!("{}/{}.jsonl", transcript_dir, ts)
            };
            let _ = std::fs::write(&fname, &output.stdout);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            let stderr_lines: Vec<&str> = stderr.lines().collect();
            eprintln!("  [claude stderr] {} lines total", stderr_lines.len());
            for line in stderr_lines.iter().take(30) {
                eprintln!("  [claude stderr] {line}");
            }
            if stderr_lines.len() > 30 {
                eprintln!(
                    "  [claude stderr] ... ({} more lines)",
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
        let mut parse_errors: u32 = 0;

        for line in stdout.lines() {
            let Ok(msg) = serde_json::from_str::<ClaudeStreamMsg>(line) else {
                parse_errors += 1;
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
                                tool_calls.push(ToolCall { name: name.clone() });
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
                        if let ClaudeUserContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error: tr_err,
                        } = block
                        {
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
                        total_tokens += u.cache_creation_input_tokens;
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
            parse_errors,
        })
    }
}

// ── Dry-dump / assembly ───────────────────────────────────────────────────────

/// The fully-assembled context that would be passed to the agent.
///
/// `assemble_agent_context` produces this in-memory without touching disk or
/// spawning a subprocess, making it useful for smoke-testing and diffing.
pub struct AssembledContext {
    /// Final CLAUDE.md text (baseline content + any profile prepend).
    pub claude_md: String,
    /// Final .mcp.json value (baseline merged with any profile fragment).
    pub mcp_json: serde_json::Value,
}

/// Build the assembled agent context from a baseline and `ProfileArtifacts`,
/// **without** touching disk or spawning any process.
///
/// * `baseline_claude_md` — the CLAUDE.md text already in the workspace (pass
///   `""` if the workspace starts empty).
/// * `baseline_mcp_json` — the .mcp.json already in the workspace (pass
///   `serde_json::json!({})` if absent).
pub fn assemble_agent_context(
    baseline_claude_md: &str,
    baseline_mcp_json: serde_json::Value,
    artifacts: &super::profiles::ProfileArtifacts,
) -> anyhow::Result<AssembledContext> {
    // ── CLAUDE.md ────────────────────────────────────────────────────────────
    // Order: (1) profile prepend — frames everything below it (e.g. Caveman terse rule wraps
    // project docs), (2) common base — shared project rules every profile sees equally,
    // (3) task-specific baseline — kept last so task instructions are never shadowed.
    const COMMON_BASE: &str = include_str!("profiles/assets/common_base_claude.md");

    let claude_md = {
        let mut parts: Vec<&str> = Vec::new();
        let prepend_owned: String;
        if let Some(prepend) = &artifacts.claude_md_prepend {
            prepend_owned = prepend.clone();
            parts.push(&prepend_owned);
        }
        parts.push(COMMON_BASE);
        if !baseline_claude_md.is_empty() {
            parts.push(baseline_claude_md);
        }
        parts.join("\n\n")
    };

    // ── .mcp.json ────────────────────────────────────────────────────────────
    let mcp_json = match &artifacts.mcp_json_fragment {
        None => baseline_mcp_json,
        Some(frag) => {
            let frag_servers = match frag.get("mcpServers") {
                Some(serde_json::Value::Object(m)) => m.clone(),
                _ => {
                    return Ok(AssembledContext {
                        claude_md,
                        mcp_json: baseline_mcp_json,
                    })
                }
            };
            let mut merged = baseline_mcp_json;
            let servers = merged
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("mcp config root is not an object"))?
                .entry("mcpServers")
                .or_insert_with(|| serde_json::Value::Object(Default::default()))
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("mcpServers is not an object"))?;
            for (k, v) in frag_servers {
                servers.insert(k, v);
            }
            merged
        }
    };

    Ok(AssembledContext {
        claude_md,
        mcp_json,
    })
}

// ── ProfileArtifacts helpers ─────────────────────────────────────────────────

/// Merge `frag.mcpServers.*` into the existing MCP JSON file at `path`.
///
/// If the file doesn't exist yet, write the fragment as-is.
/// Keys in `frag.mcpServers` override existing keys.
fn apply_mcp_fragment(path: &Path, frag: &Value) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let frag_servers = match frag.get("mcpServers") {
        Some(Value::Object(m)) => m.clone(),
        _ => return Ok(()), // nothing to merge
    };

    let mut existing: Value = if path.exists() {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?
    } else {
        serde_json::json!({})
    };

    let servers = existing
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("mcp config root is not an object"))?
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("mcpServers is not an object"))?;

    for (k, v) in frag_servers {
        servers.insert(k, v);
    }

    let out = serde_json::to_string_pretty(&existing).context("serialize merged mcp config")?;
    std::fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Prepend `text` to `CLAUDE.md` (creating it if absent).
fn apply_claude_md_prepend(path: &Path, text: &str) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let existing = if path.exists() {
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
    } else {
        String::new()
    };

    let combined = if existing.is_empty() {
        text.to_string()
    } else {
        format!("{}\n\n{}", text, existing)
    };

    std::fs::write(path, combined).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_captures_tool_result_bytes_string_form() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
            "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"tu_1","name":"Read","input":{"file":"x"}}]}}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_1","content":"hello world","is_error":false}]}}"#,
            "\n",
            r#"{"type":"result","result":"done","is_error":false}"#,
            "\n",
        );
        let res = ClaudeDriver::parse_output(stream.as_bytes(), 0).unwrap();
        assert_eq!(res.tool_calls_detail.len(), 1);
        assert_eq!(res.tool_calls_detail[0].name, "Read");
        assert_eq!(
            res.tool_calls_detail[0].output_bytes,
            "hello world".len() as u64
        );
        assert!(!res.tool_calls_detail[0].is_error);
    }

    #[test]
    fn parse_output_captures_tool_result_bytes_block_form_and_error() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
            "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"content":[{"type":"tool_use","id":"tu_a","name":"Bash","input":{"cmd":"x"}}]}}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu_a","content":[{"type":"text","text":"abc"},{"type":"text","text":"def"}],"is_error":true}]}}"#,
            "\n",
            r#"{"type":"result","result":"","is_error":false}"#,
            "\n",
        );
        let res = ClaudeDriver::parse_output(stream.as_bytes(), 0).unwrap();
        assert_eq!(res.tool_calls_detail.len(), 1);
        assert_eq!(res.tool_calls_detail[0].output_bytes, 6);
        assert!(res.tool_calls_detail[0].is_error);
    }

    #[test]
    fn profile_artifacts_mcp_merge_and_claude_md_prepend() {
        use crate::benchmark::profiles::ProfileArtifacts;
        use std::collections::HashMap;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let mcp_path = dir.path().join(".mcp.json");
        let claude_md = dir.path().join("CLAUDE.md");

        // Seed existing .mcp.json with server "a"
        let existing_mcp = serde_json::json!({
            "mcpServers": { "a": { "command": "a-cmd" } }
        });
        std::fs::write(&mcp_path, serde_json::to_string(&existing_mcp).unwrap()).unwrap();

        // Seed existing CLAUDE.md
        std::fs::write(&claude_md, "hello").unwrap();

        // Fragment adds server "b"
        let artifacts = ProfileArtifacts {
            mcp_json_fragment: Some(serde_json::json!({
                "mcpServers": { "b": { "command": "b-cmd" } }
            })),
            claude_md_prepend: Some("CAVE TEXT".to_string()),
            env: HashMap::new(),
        };

        // Apply MCP merge
        apply_mcp_fragment(&mcp_path, artifacts.mcp_json_fragment.as_ref().unwrap()).unwrap();
        // Apply CLAUDE.md prepend
        apply_claude_md_prepend(&claude_md, artifacts.claude_md_prepend.as_ref().unwrap()).unwrap();

        // Verify MCP: both "a" and "b" present
        let merged: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
        assert!(merged["mcpServers"]["a"].is_object(), "server 'a' missing");
        assert!(merged["mcpServers"]["b"].is_object(), "server 'b' missing");

        // Verify CLAUDE.md: prepend + separator + original
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.starts_with("CAVE TEXT"), "prepend missing");
        assert!(content.contains("hello"), "original content missing");
    }
}
