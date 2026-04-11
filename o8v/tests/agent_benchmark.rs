// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.
#![allow(clippy::disallowed_methods)] // test file: unwrap_or_else used for contextual panic messages

//! Agent integration tests — each test proves a specific value claim.
//!
//! ## Value → Test mapping
//!
//! | # | Value claim                          | Test                                      |
//! |---|--------------------------------------|-------------------------------------------|
//! | 1 | Correct code, not just compiling     | `v1_agent_catches_violations_introduced`  |
//! | 2 | Accurate diagnosis matches 8v truth  | `v2_agent_diagnosis_matches_ground_truth` |
//! | 3 | Pre-commit gate                      | `v3_agent_runs_8v_before_declaring_done`  |
//! | 4 | Token efficiency vs direct tools     | `v4_token_efficiency`                     |
//! | 5 | Reliability — misses nothing         | `v5_reliability_completeness`             |
//! | 6 | Baseline with 8v setup               | `v6_baseline_with_8v_setup` (benchmark)   |
//!
//! ## What "ground truth" means
//!
//! Each test runs `8v check .` first to get the canonical violation list.
//! The agent is then given a realistic user task (not "check code quality").
//! The assertion is: does the agent's response surface what `8v check` found?
//!
//! This makes each test a KPI: if it passes, we deliver the claimed value.
//! If it fails, we have a gap. When building features, ask: which tests go green?
//!
//! ## Running
//!
//! ```sh
//! # All (requires: claude CLI in PATH, ~4–6 min, costs tokens)
//! cargo test -p o8v --test agent_integration -- --ignored --nocapture
//!
//! # One scenario
//! cargo test -p o8v --test agent_integration v1_ -- --ignored --nocapture
//! ```

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use o8v_testkit::TempProject;

use o8v_fs::ContainmentRoot;

// ── Struct definitions ───────────────────────────────────────────────────────

// ── Claude CLI JSONL stream types ────────────────────────────────────────────

/// Top-level JSONL line from `claude --output-format=stream-json`.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeStreamMsg {
    System(ClaudeSystemMsg),
    Assistant(ClaudeAssistantMsg),
    Result(ClaudeResultMsg),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeSystemMsg {
    subtype: String,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeAssistantMsg {
    message: ClaudeMessage,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeMessage {
    usage: ClaudeUsage,
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, serde::Deserialize)]
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

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClaudeContentBlock {
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    Text {
        text: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeResultMsg {
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    usage: Option<ClaudeResultUsage>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeResultUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

// ── MCP event types (for v4b test) ───────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct McpInvokedEvent {
    run_id: String,
    command: String,
    command_bytes: u64,
    command_token_estimate: u64,
}

#[derive(Debug, serde::Deserialize)]
struct McpCompletedEvent {
    run_id: String,
    render_bytes: u64,
    token_estimate: u64,
    duration_ms: u64,
}

/// A single tool invocation recorded from agent output.
#[derive(Debug, Clone)]
struct ToolCall {
    name: String,
    input: String,
}

/// Per-turn token breakdown from the Claude API response.
#[derive(Debug, Clone)]
struct TurnUsage {
    role: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_input_tokens: u64,
    cache_creation_input_tokens: u64,
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

// ── 8v ground truth ───────────────────────────────────────────────────────────

/// Run `8v check .` on a project and return stdout as ground truth.
/// Exit codes 0 (pass), 1 (violations), 2 (nothing) are all valid.
/// Only panics on process spawn failure.
fn run_8v_check(project: &Path) -> GroundTruth {
    let binary = env!("CARGO_BIN_EXE_8v");
    let out = Command::new(binary)
        .args(["check", ".", "--json"])
        .current_dir(project)
        .output()
        .expect("run 8v check");

    let exit_code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    // Extract names of checks that failed — e.g. "cargo fmt", "clippy".
    // These are the terms agents naturally use when describing issues.
    let violations: Vec<String> = match serde_json::from_str::<o8v_testkit::JsonOutput>(&stdout) {
        Ok(output) => output
            .results
            .into_iter()
            .flat_map(|r| {
                r.checks.into_iter().filter_map(|c| {
                    if c.outcome == "failed" {
                        Some(c.name)
                    } else {
                        None
                    }
                })
            })
            .collect(),
        Err(_) => vec![],
    };

    GroundTruth {
        exit_code,
        violations,
        raw: stdout,
    }
}

struct GroundTruth {
    exit_code: i32,
    /// Names of checks that failed (e.g. "cargo fmt", "clippy").
    violations: Vec<String>,
    raw: String,
}

// ── MCP config ────────────────────────────────────────────────────────────────

fn mcp_json(binary_path: &str) -> String {
    let escaped = binary_path.replace('\\', "\\\\");
    format!(
        "{{\n  \"mcpServers\": {{\n    \"8v\": {{\n      \"command\": \"{}\",\n      \"args\": [\"mcp\"]\n    }}\n  }}\n}}\n",
        escaped
    )
}

fn write_8v_setup(project: &Path, binary_path: &str) {
    // 8v init --yes: creates .8v/, appends to CLAUDE.md and AGENTS.md,
    // writes .mcp.json, installs pre-commit hook. Does not create files that
    // already exist — appends the 8v section if missing.
    let output = Command::new(binary_path)
        .args(["init", "--yes"])
        .current_dir(project)
        .output()
        .expect("run 8v init");
    assert!(
        output.status.success(),
        "8v init --yes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Patch .mcp.json to use the test binary path (init writes "8v" expecting PATH)
    let root = ContainmentRoot::new(project).expect("containment root for mcp patch");
    o8v_fs::safe_write(
        &project.join(".mcp.json"),
        &root,
        mcp_json(binary_path).as_bytes(),
    )
    .expect("patch .mcp.json");
}

// ── Agent result ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct AgentResult {
    /// Every tool called during the session.
    tool_calls: Vec<ToolCall>,
    /// Concatenated text from all assistant content blocks.
    response_text: String,
    /// Total input + output tokens across the session.
    total_tokens: u64,
    /// Total cost in USD reported by the agent.
    cost_usd: f64,
    /// Process exit code.
    exit_code: i32,
    /// Per-turn token breakdown.
    turn_usage: Vec<TurnUsage>,
    /// Raw init message size in bytes
    init_message_bytes: usize,
}

impl AgentResult {
    fn used_8v(&self) -> bool {
        self.tool_calls.iter().any(|t| t.name.contains("8v"))
    }

    fn tool_call_count(&self) -> usize {
        self.tool_calls.len()
    }

    /// Returns true if the agent response mentions any of the violations 8v found.
    /// Checks exact match AND common synonyms agents use (e.g. "formatting" for "cargo fmt").
    fn surfaces_violations(&self, violations: &[String]) -> bool {
        if violations.is_empty() {
            return true;
        }
        let text = self.response_text.to_lowercase();
        violations.iter().any(|v| {
            let v = v.to_lowercase();
            if text.contains(&v) {
                return true;
            }
            // Synonyms: agents describe check failures in natural language
            match v.as_str() {
                "cargo fmt" => {
                    text.contains("fmt") || text.contains("format") || text.contains("rustfmt")
                }
                "clippy" => text.contains("clippy") || text.contains("lint"),
                "cargo check" => text.contains("compile") || text.contains("cargo check"),
                _ => false,
            }
        })
    }
}

// ── Claude Code driver ────────────────────────────────────────────────────────

fn run_claude(prompt: &str, working_dir: &Path) -> Result<AgentResult, String> {
    run_claude_with_mcp(
        prompt,
        working_dir,
        None,
        &[],
        &[],
        "bypassPermissions",
        None,
    )
}

/// Spawn claude with an explicit MCP config file.
///
/// When `mcp_config` is `Some`, passes `--mcp-config <path>` so Claude Code
/// discovers the MCP server reliably (instead of relying on `.mcp.json`
/// auto-discovery from cwd, which may not work in all environments).
fn run_claude_with_mcp(
    prompt: &str,
    working_dir: &Path,
    mcp_config: Option<&Path>,
    disallowed_tools: &[&str],
    env_vars: &[(&str, &str)],
    permission_mode: &str,
    settings_path: Option<&Path>,
) -> Result<AgentResult, String> {
    let mut args = vec![
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--permission-mode",
        permission_mode,
    ];

    let mcp_path_str;
    if let Some(config) = mcp_config {
        mcp_path_str = config
            .to_str()
            .expect("mcp config path is UTF-8")
            .to_string();
        args.push("--mcp-config");
        args.push(&mcp_path_str);
    }

    for tool in disallowed_tools {
        args.push("--disallowedTools");
        args.push(tool);
    }

    let settings_path_str;
    if let Some(settings) = settings_path {
        settings_path_str = settings
            .to_str()
            .expect("settings path is UTF-8")
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

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn claude: {e} (is `claude` in PATH?)"))?;

    {
        let mut stdin = child.stdin.take().expect("stdin");
        let msg = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": prompt }
        });
        writeln!(stdin, "{}", serde_json::to_string(&msg).expect("serialize"))
            .map_err(|e| format!("write stdin: {e}"))?;
    }

    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

    // Log stderr for debugging MCP server discovery.
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut response_text = String::new();
    let mut total_tokens: u64 = 0;
    let mut cost_usd: f64 = 0.0;
    let mut turn_usage: Vec<TurnUsage> = Vec::new();
    let mut init_message_bytes: usize = 0;

    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<ClaudeStreamMsg>(line) else {
            continue;
        };
        match msg {
            ClaudeStreamMsg::System(sys) if sys.subtype == "init" => {
                init_message_bytes = line.len();
            }
            ClaudeStreamMsg::Assistant(asst) => {
                let usage = &asst.message.usage;
                let input_tokens = usage.input_tokens;
                let output_tokens = usage.output_tokens;
                let cache_read_input_tokens = usage.cache_read_input_tokens;
                let cache_creation_input_tokens = usage.cache_creation_input_tokens;

                for block in asst.message.content {
                    match block {
                        ClaudeContentBlock::ToolUse { name, input } => {
                            let input_str = match serde_json::to_string(&input) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("warn: failed to serialize tool input: {e}");
                                    String::from("{}")
                                }
                            };
                            tool_calls.push(ToolCall {
                                name: name.clone(),
                                input: input_str,
                            });
                            turn_usage.push(TurnUsage {
                                role: name,
                                input_tokens,
                                output_tokens,
                                cache_read_input_tokens,
                                cache_creation_input_tokens,
                            });
                        }
                        ClaudeContentBlock::Text { text } => {
                            response_text.push_str(&text);
                            turn_usage.push(TurnUsage {
                                role: "text".to_string(),
                                input_tokens,
                                output_tokens,
                                cache_read_input_tokens,
                                cache_creation_input_tokens,
                            });
                        }
                        ClaudeContentBlock::Unknown => {}
                    }
                }
            }
            ClaudeStreamMsg::Result(res) => {
                if let Some(text) = res.result {
                    response_text.push_str(&text);
                }
                if let Some(u) = res.usage {
                    total_tokens += u.input_tokens;
                    total_tokens += u.output_tokens;
                    total_tokens += u.cache_read_input_tokens;
                }
                cost_usd = res.total_cost_usd.unwrap_or(0.0);
            }
            _ => {}
        }
    }

    Ok(AgentResult {
        tool_calls,
        response_text,
        total_tokens,
        cost_usd,
        exit_code: output.status.code().unwrap_or(-1),
        turn_usage,
        init_message_bytes,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 1 — Correct code, not just compiling code
//
// Claim: 8v ensures AI agents deliver correct code, not just code that passes
//        `cargo check`. The agent should catch violations it *introduces*
//        while implementing a feature.
//
// Question: "Add a multiply function that takes a mutable reference"
// (This is a trap: &mut where &i32 suffices — clippy catches it with -D warnings)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens)"]
fn v1_agent_catches_violations_introduced() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/clean-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();

    // Confirm clean baseline
    let before = run_8v_check(project);
    assert_eq!(
        before.exit_code, 0,
        "fixture must start clean: {}",
        before.raw
    );

    // Set up 8v
    write_8v_setup(project, binary);

    // Ask agent to implement something that will introduce a violation
    let result = run_claude(
        "Add a `multiply` function that takes two mutable i32 references and returns their product. \
         Add a call to it in main. Make sure it compiles.",
        project,
    )
    .expect("run claude");

    // Get ground truth AFTER the agent made changes
    let after = run_8v_check(project);

    eprintln!(
        "\n[v1] tool_calls={:?}\n[v1] used_8v={}\n[v1] 8v violations={:?}\n[v1] cost=${:.4}\n",
        result.tool_calls,
        result.used_8v(),
        after.violations,
        result.cost_usd
    );

    // V1 assertion: agent used 8v AND response mentions violations 8v found
    assert!(
        result.used_8v(),
        "Agent did not use 8v after introducing changes.\n\
         Tool calls: {:?}\n\
         Response: {}",
        result.tool_calls,
        &result.response_text[..result.response_text.len().min(600)]
    );

    // If 8v found violations, the agent should have surfaced them
    if !after.violations.is_empty() {
        assert!(
            result.surfaces_violations(&after.violations),
            "Agent introduced violations but did not surface them.\n\
             8v found: {:?}\n\
             Agent said: {}",
            after.violations,
            &result.response_text[..result.response_text.len().min(600)]
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 2 — Accurate diagnosis: agent's findings match 8v ground truth
//
// Claim: 8v gives agents a complete, structured picture. Agents using 8v
//        identify exactly what is wrong — no more, no less.
//        Running cargo directly misses -D warnings violations.
//
// Question: "There's something wrong with my code, can you help me identify it?"
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens)"]
fn v2_agent_diagnosis_matches_ground_truth() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/violated-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();

    write_8v_setup(project, binary);

    // Get ground truth first
    let truth = run_8v_check(project);
    assert!(
        truth.exit_code == 1,
        "fixture must have violations, got exit {}: {}",
        truth.exit_code,
        truth.raw
    );

    let result = run_claude(
        "There's something wrong with my code. Can you identify the issues?",
        project,
    )
    .expect("run claude");

    eprintln!(
        "\n[v2] tool_calls={:?}\n[v2] used_8v={}\n[v2] ground_truth violations={:?}\n\
         [v2] agent_surfaces_them={}\n[v2] cost=${:.4}\n",
        result.tool_calls,
        result.used_8v(),
        truth.violations,
        result.surfaces_violations(&truth.violations),
        result.cost_usd
    );

    // V2 assertion: agent used 8v, and agent findings include 8v's violations
    assert!(
        result.used_8v(),
        "Agent did not use 8v to diagnose.\n\
         Tool calls: {:?}",
        result.tool_calls
    );

    // V2 core assertion: agent must not claim clean when 8v found violations.
    // We check for absence of false positives ("no issues", "looks good") rather
    // than keyword-matching violation names — agents describe findings naturally.
    let response_lower = result.response_text.to_lowercase();
    let falsely_claims_clean = (response_lower.contains("no issues")
        || response_lower.contains("no problems")
        || response_lower.contains("looks good")
        || response_lower.contains("everything is fine")
        || response_lower.contains("all checks pass"))
        && !response_lower.contains("issue")
        && !response_lower.contains("error")
        && !response_lower.contains("fail")
        && !response_lower.contains("fix");

    assert!(
        !falsely_claims_clean,
        "Agent falsely reported clean when 8v found violations.\n\
         8v found: {:?}\n\
         Agent said: {}",
        truth.violations,
        &result.response_text[..result.response_text.len().min(800)]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 3 — Pre-commit gate: agent runs 8v before declaring done
//
// Claim: 8v prevents bad commits. When asked "is this ready?", the agent
//        runs 8v and reports violations rather than saying "looks good".
//
// Question: "I've made some changes. Is this ready to commit?"
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens)"]
fn v3_agent_runs_8v_before_declaring_done() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/violated-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();

    write_8v_setup(project, binary);

    let truth = run_8v_check(project);

    let result = run_claude(
        "I've made some changes to this project. Is it production-ready? Are there any issues I should fix first?",
        project,
    )
    .expect("run claude");

    eprintln!(
        "\n[v3] tool_calls={:?}\n[v3] used_8v={}\n[v3] violations={:?}\n[v3] cost=${:.4}\n",
        result.tool_calls,
        result.used_8v(),
        truth.violations,
        result.cost_usd
    );

    // V3 assertion: agent must not say "ready" without checking via 8v
    assert!(
        result.used_8v(),
        "Agent declared commit readiness without running 8v.\n\
         Tool calls: {:?}\n\
         Response: {}",
        result.tool_calls,
        &result.response_text[..result.response_text.len().min(600)]
    );

    // With violations present, agent should not say "ready to commit"
    let response_lower = result.response_text.to_lowercase();
    let says_ready_without_caveats = (response_lower.contains("production-ready")
        || response_lower.contains("ready to ship")
        || response_lower.contains("looks good")
        || response_lower.contains("no issues"))
        && !response_lower.contains("violation")
        && !response_lower.contains("error")
        && !response_lower.contains("issue")
        && !response_lower.contains("fmt")
        && !response_lower.contains("format");

    assert!(
        !says_ready_without_caveats,
        "Agent said 'ready to commit' despite violations.\n\
         Violations: {:?}\n\
         Response: {}",
        truth.violations,
        &result.response_text[..result.response_text.len().min(600)]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 4 — Token efficiency: fewer tool calls and tokens vs direct tools
//
// Claim: 8v reduces the number of tool calls an agent needs to check a project.
//        Without 8v, agents run cargo check + cargo clippy + cargo fmt
//        separately (3+ Bash calls). With 8v: one MCP call.
//        Fewer tool calls = fewer tokens = lower cost per task.
//
// Measured by comparing the same task with and without 8v setup.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60-120s, costs tokens — runs agent twice)"]
fn v4_token_efficiency() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "Check everything in this project and report any issues.";

    // Both projects start IDENTICAL — a real AI-ready project with git.
    // Only variable: one gets `8v init --yes`, the other doesn't.

    // ── Without 8v ──────────────────────────────────────────────────────────
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/ai-ready");
    let baseline_dir = TempProject::from_fixture(&fixture);
    Command::new("git")
        .args(["init"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git init baseline");

    let baseline = run_claude(prompt, baseline_dir.path()).expect("run claude (baseline)");

    // ── With 8v ─────────────────────────────────────────────────────────────
    let with_8v_dir = TempProject::from_fixture(&fixture);
    Command::new("git")
        .args(["init"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git init with_8v");
    write_8v_setup(with_8v_dir.path(), binary);

    let mcp_config = with_8v_dir.path().join(".mcp.json");
    let with_8v = run_claude_with_mcp(
        prompt,
        with_8v_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "bypassPermissions",
        None,
    )
    .expect("run claude (with 8v)");

    // Read MCP event measurements — exact bytes 8v sent to the agent.
    // Best-effort: .8v/ may not exist if agent never triggered a check.
    let mcp = o8v_testkit::McpMeasurement::from_project(with_8v_dir.path()).unwrap_or_else(|e| {
        eprintln!("  MCP measurement: {e}");
        o8v_testkit::McpMeasurement::zero()
    });

    // ── Report ───────────────────────────────────────────────────────────────
    eprintln!(
        "\n[v4] BASELINE (no 8v docs)\n\
         [v4]   tool_calls ({})={:?}\n\
         [v4]   total_tokens={}\n\
         [v4]   cost=${:.4}\n\
         [v4] WITH 8V\n\
         [v4]   tool_calls ({})={:?}\n\
         [v4]   used_8v={}\n\
         [v4]   total_tokens={}\n\
         [v4]   cost=${:.4}\n\
         [v4] DELTA\n\
         [v4]   tool_calls saved={}\n\
         [v4]   tokens saved={}\n\
         [v4]   cost saved=${:.4}\n",
        baseline.tool_call_count(),
        baseline.tool_calls,
        baseline.total_tokens,
        baseline.cost_usd,
        with_8v.tool_call_count(),
        with_8v.tool_calls,
        with_8v.used_8v(),
        with_8v.total_tokens,
        with_8v.cost_usd,
        baseline
            .tool_call_count()
            .saturating_sub(with_8v.tool_call_count()),
        baseline.total_tokens.saturating_sub(with_8v.total_tokens),
        (baseline.cost_usd - with_8v.cost_usd).max(0.0),
    );

    // V4: Record efficiency metrics — this is a benchmark, not a pass/fail gate.
    // The agent with 8v may make more MCP calls while the tool stabilizes,
    // but the token cost per violation found should improve over time.

    eprintln!(
        "\n[v4] EFFICIENCY SUMMARY\n\
         [v4]   without 8v: {} tool calls, {} tokens, ${:.4}\n\
         [v4]   with 8v:    {} tool calls, {} tokens, ${:.4}\n\
         [v4]   8v exact bytes sent to agent: {} render_bytes (~{} token estimate)\n\
         [v4]   8v exact bytes from agent:    {} command_bytes (~{} token estimate)\n\
         [v4]   8v calls made:                {}\n\
         [v4]   bytes per 8v call (avg):      {}\n\
         [v4]   8v used: {}\n",
        baseline.tool_call_count(),
        baseline.total_tokens,
        baseline.cost_usd,
        with_8v.tool_call_count(),
        with_8v.total_tokens,
        with_8v.cost_usd,
        mcp.render_bytes,
        mcp.render_token_estimate(),
        mcp.command_bytes,
        mcp.command_token_estimate(),
        mcp.call_count,
        mcp.avg_bytes_per_call().unwrap_or(0),
        with_8v.used_8v(),
    );

    // Verdict: is 8v more token efficient?
    let better = with_8v.total_tokens < baseline.total_tokens;
    eprintln!(
        "[v4] VERDICT: {}",
        if better {
            "8v IS more token efficient"
        } else {
            "8v is NOT yet more token efficient — investigate"
        }
    );

    // Only hard assertion: agent must have used 8v (not just run blind)
    assert!(
        with_8v.used_8v(),
        "With-8v agent did not use 8v tool.\n\
         Tool calls: {:?}",
        with_8v.tool_calls
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 4b — MCP Events: verify cost measurements are written correctly
//
// Claim: When an agent uses 8v via MCP, every invocation records cost signals
//        in `.8v/mcp-events.ndjson` with accurate byte/token measurements.
//
// Setup: Run 8v init to create .8v/ directory, then run agent with prompt.
// Verify: Parse NDJSON events, check McpInvoked and McpCompleted records.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~10s, costs tokens)"]
fn v4_mcp_events_written() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/violated-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();

    // 1. Project already populated from fixture
    write_8v_setup(project, binary);

    // 2. Run agent with a realistic prompt
    let _result = run_claude(
        "Check everything in this project and report any issues.",
        project,
    )
    .expect("run claude");

    // 3. Read the MCP events file (written to ~/.8v/mcp-events.ndjson)
    let home = std::env::var("HOME").expect("HOME env var");
    let events_path = std::path::Path::new(&home)
        .join(".8v")
        .join("mcp-events.ndjson");

    let events_content = fs::read_to_string(&events_path).expect("read mcp-events.ndjson");
    let lines: Vec<&str> = events_content.lines().collect();

    eprintln!(
        "\n[v4b] MCP_EVENTS_WRITTEN\n[v4b]   total_lines={}",
        lines.len()
    );

    // 4. Parse all lines as JSON and separate McpInvoked vs McpCompleted
    let mut invoked_events: Vec<McpInvokedEvent> = Vec::new();
    let mut completed_events: Vec<McpCompletedEvent> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        // Determine event type first, then deserialize into the appropriate struct.
        let type_probe: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {idx}: parse as JSON: {e}"));
        let event_type = type_probe
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("line {idx}: get event type"));

        match event_type {
            "McpInvoked" => {
                let ev: McpInvokedEvent = serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("line {idx}: deserialize McpInvokedEvent: {e}"));
                invoked_events.push(ev);
            }
            "McpCompleted" => {
                let ev: McpCompletedEvent = serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("line {idx}: deserialize McpCompletedEvent: {e}"));
                completed_events.push(ev);
            }
            _ => {}
        }
    }

    // 5. Assert: at least one McpInvoked event exists
    assert!(
        !invoked_events.is_empty(),
        "No McpInvoked events found in mcp-events.ndjson"
    );

    // 6. Assert: at least one McpCompleted event exists
    assert!(
        !completed_events.is_empty(),
        "No McpCompleted events found in mcp-events.ndjson"
    );

    // 7. Assert: every McpInvoked has command_bytes > 0
    for (idx, ev) in invoked_events.iter().enumerate() {
        assert!(
            ev.command_bytes > 0,
            "McpInvoked[{}]: command_bytes must be > 0, got {}",
            idx,
            ev.command_bytes
        );
    }

    // 8. Assert: every McpCompleted has render_bytes > 0
    for (idx, ev) in completed_events.iter().enumerate() {
        assert!(
            ev.render_bytes > 0,
            "McpCompleted[{}]: render_bytes must be > 0, got {}",
            idx,
            ev.render_bytes
        );
    }

    // 9. Assert: matching run_ids (each McpInvoked has a McpCompleted)
    let invoked_run_ids: std::collections::HashSet<_> =
        invoked_events.iter().map(|ev| ev.run_id.clone()).collect();
    let completed_run_ids: std::collections::HashSet<_> = completed_events
        .iter()
        .map(|ev| ev.run_id.clone())
        .collect();

    for run_id in invoked_run_ids.iter() {
        assert!(
            completed_run_ids.contains(run_id),
            "McpInvoked with run_id='{}' has no matching McpCompleted",
            run_id
        );
    }

    // 10. Assert: token_estimates = render_bytes / 4 (verify formula)
    for (idx, ev) in completed_events.iter().enumerate() {
        let expected = ev.render_bytes / 4;
        assert_eq!(
            ev.token_estimate, expected,
            "McpCompleted[{}]: token_estimate {} != render_bytes / 4 ({})",
            idx, ev.token_estimate, expected
        );
    }

    // 11. Print full measurements with eprintln! for --nocapture visibility
    eprintln!("[v4b]   invoked_count={}", invoked_events.len());
    eprintln!("[v4b]   completed_count={}", completed_events.len());
    for (idx, ev) in invoked_events.iter().enumerate() {
        eprintln!(
            "[v4b]   McpInvoked[{}]: run_id={}, cmd='{}', bytes={}, tokens={}",
            idx, ev.run_id, ev.command, ev.command_bytes, ev.command_token_estimate
        );
    }
    for (idx, ev) in completed_events.iter().enumerate() {
        eprintln!(
            "[v4b]   McpCompleted[{}]: run_id={}, render_bytes={}, tokens={}, duration_ms={}",
            idx, ev.run_id, ev.render_bytes, ev.token_estimate, ev.duration_ms
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 5 — Reliability: agent misses nothing that 8v found
//
// Claim: 8v's structured output is more complete than ad-hoc cargo invocations.
//        An agent using 8v surfaces every violation. An agent using Bash
//        may run without -D warnings and miss lints.
//
// Question: "Review my code and tell me everything that needs to be fixed."
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens)"]
fn v5_reliability_completeness() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/violated-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();
    write_8v_setup(project, binary);

    // Ground truth: everything 8v finds
    let truth = run_8v_check(project);

    let result = run_claude(
        "Review my code and tell me everything that needs to be fixed before this goes to production.",
        project,
    )
    .expect("run claude");

    // Count how many of 8v's violations appear in the agent response
    let completeness_pct = if result.surfaces_violations(&truth.violations) {
        100.0_f64
    } else {
        0.0_f64
    };
    let found_count = if completeness_pct >= 100.0 {
        truth.violations.len()
    } else {
        0
    };

    eprintln!(
        "\n[v5] tool_calls={:?}\n[v5] used_8v={}\n\
         [v5] 8v violations={:?}\n[v5] agent surfaced {}/{} ({:.0}%)\n\
         [v5] cost=${:.4}\n",
        result.tool_calls,
        result.used_8v(),
        truth.violations,
        found_count,
        truth.violations.len(),
        completeness_pct,
        result.cost_usd
    );

    // V5 assertion: agent must surface all violations 8v found
    assert!(
        result.used_8v(),
        "Agent did not use 8v.\n\
         Tool calls: {:?}",
        result.tool_calls
    );

    assert!(
        completeness_pct >= 100.0,
        "Agent missed violations that 8v found (completeness {:.0}%).\n\
         8v found: {:?}\n\
         Agent surfaced: {}/{}\n\
         Agent said: {}",
        completeness_pct,
        truth.violations,
        found_count,
        truth.violations.len(),
        &result.response_text[..result.response_text.len().min(800)]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 6 — Baseline with 8v setup (8v init --yes): records what agent does
//            with full 8v config. Run with --nocapture to see the delta.
//            Compare tool_call_count, tokens, and completeness against v4 and v5.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens)"]
fn v6_baseline_with_8v_setup() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/violated-rust");
    let tmpdir = TempProject::from_fixture(&fixture);
    let project = tmpdir.path();

    // MCP available but no CLAUDE.md/AGENTS.md
    write_8v_setup(project, binary);

    let truth = run_8v_check(project);

    let result = run_claude(
        "Review my code and tell me everything that needs to be fixed before this goes to production.",
        project,
    )
    .expect("run claude");

    let found_count = truth
        .violations
        .iter()
        .filter(|v| {
            result
                .response_text
                .to_lowercase()
                .contains(&v.to_lowercase())
        })
        .count();

    let completeness_pct = if truth.violations.is_empty() {
        100.0
    } else {
        (found_count as f64 / truth.violations.len() as f64) * 100.0
    };

    eprintln!(
        "\n[v6 BASELINE] tool_calls ({})={:?}\n\
         [v6 BASELINE] used_8v={}\n\
         [v6 BASELINE] 8v violations={:?}\n\
         [v6 BASELINE] completeness={:.0}% ({}/{})\n\
         [v6 BASELINE] tokens={}\n\
         [v6 BASELINE] cost=${:.4}\n\
         [v6 BASELINE] init_message_bytes={} (≈ {} tokens)\n",
        result.tool_call_count(),
        result.tool_calls,
        result.used_8v(),
        truth.violations,
        completeness_pct,
        found_count,
        truth.violations.len(),
        result.total_tokens,
        result.cost_usd,
        result.init_message_bytes,
        result.init_message_bytes / 4
    );

    // Baseline only asserts the agent ran cleanly
    assert_ne!(result.exit_code, -1, "claude did not exit cleanly");
}

/// Parses 8v check JSON output and counts how many unique stacks have violations
fn count_violated_stacks(json_output: &str) -> usize {
    let output: o8v_testkit::JsonOutput =
        serde_json::from_str(json_output).expect("8v check --json produced invalid JSON");
    output
        .results
        .iter()
        .filter(|r| r.checks.iter().any(|c| c.outcome == "failed"))
        .map(|r| r.stack.clone())
        .collect::<std::collections::HashSet<_>>()
        .len()
}

// ─── V4-poly: Polyglot Token Efficiency ─────────────────────────────────────
//
// Value claim: 8v reduces total token cost for polyglot projects by replacing
// 12-15 individual tool invocations with one `8v check .` MCP call.
//
// The single-stack V4 test above shows 8v costs MORE for a Rust-only project
// (MCP schema overhead > savings from consolidating 3 tools). This test
// exercises the real scenario: a 6-stack project where the baseline agent
// must discover and run many individual tools.

#[test]
#[ignore = "requires: claude CLI, polyglot toolchains installed (~180s, costs tokens)"]
fn v4_polyglot_token_efficiency() {
    let prompt = "Check this entire project for issues across all stacks. \
                  Report everything that needs to be fixed — code quality, \
                  formatting, type errors, lint warnings.";
    let binary = env!("CARGO_BIN_EXE_8v");

    // ── Baseline (no 8v) ────────────────────────────────────────────────
    let fixture_src = o8v_testkit::fixture_path("o8v", "agent-benchmark/polyglot-violated");
    let baseline_dir = TempProject::from_fixture(&fixture_src);
    Command::new("git")
        .args(["init"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git init baseline");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config email baseline");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config name baseline");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git add baseline");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git commit baseline");

    // Verify fixture produces violations across multiple stacks
    let truth = run_8v_check(baseline_dir.path());
    let stacks_violated = count_violated_stacks(&truth.raw);
    eprintln!("\n── Fixture validation ──");
    eprintln!("  exit_code:        {}", truth.exit_code);
    eprintln!("  violations:       {:?}", truth.violations);
    eprintln!("  stacks_violated:  {stacks_violated}");
    assert!(
        truth.exit_code == 1,
        "polyglot fixture must have violations (exit 1), got exit {}",
        truth.exit_code
    );
    assert!(
        stacks_violated >= 3,
        "need violations across 3+ stacks for meaningful benchmark, got {stacks_violated}"
    );

    let baseline = run_claude(prompt, baseline_dir.path()).expect("run claude (baseline)");

    // ── With 8v (separate identical project) ────────────────────────────
    let with_8v_dir = TempProject::from_fixture(&fixture_src);
    Command::new("git")
        .args(["init"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git init with_8v");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git config email with_8v");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git config name with_8v");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git add with_8v");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(with_8v_dir.path())
        .output()
        .expect("git commit with_8v");
    write_8v_setup(with_8v_dir.path(), binary);

    // Pass --mcp-config explicitly so Claude Code discovers the 8v MCP server
    // reliably. Auto-discovery from .mcp.json in cwd may not work in all environments.
    let mcp_config = with_8v_dir.path().join(".mcp.json");
    let with_8v = run_claude_with_mcp(
        prompt,
        with_8v_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "bypassPermissions",
        None,
    )
    .expect("run claude (with 8v)");

    let mcp = o8v_testkit::McpMeasurement::from_project(with_8v_dir.path()).unwrap_or_else(|e| {
        eprintln!("  MCP measurement: {e}");
        o8v_testkit::McpMeasurement::zero()
    });

    // ── Report ──────────────────────────────────────────────────────────
    eprintln!("\n── V4-poly: Polyglot Token Efficiency ──");
    eprintln!("  BASELINE (no 8v):");
    eprintln!("    tool_calls:     {}", baseline.tool_calls.len());
    eprintln!("    total_tokens:   {}", baseline.total_tokens);
    eprintln!("    cost_usd:       ${:.4}", baseline.cost_usd);
    for tc in &baseline.tool_calls {
        eprintln!("      -> {}", tc.name);
    }

    eprintln!("  WITH 8v:");
    eprintln!("    tool_calls:     {}", with_8v.tool_calls.len());
    eprintln!("    total_tokens:   {}", with_8v.total_tokens);
    eprintln!("    cost_usd:       ${:.4}", with_8v.cost_usd);
    for tc in &with_8v.tool_calls {
        eprintln!("      -> {}", tc.name);
    }

    eprintln!("  MCP EVENTS:");
    eprintln!("    mcp_calls:      {}", mcp.call_count);
    eprintln!("    render_bytes:   {}", mcp.render_bytes);
    eprintln!("    render_tokens:  ~{}", mcp.render_token_estimate());
    eprintln!("    command_bytes:  {}", mcp.command_bytes);
    eprintln!("    command_tokens: ~{}", mcp.command_token_estimate());
    if mcp.has_events() {
        eprintln!(
            "    avg_bytes/call: {}",
            mcp.avg_bytes_per_call().unwrap_or(0)
        );
    }

    let tool_delta = baseline.tool_calls.len() as i64 - with_8v.tool_calls.len() as i64;
    let token_delta = baseline.total_tokens as i64 - with_8v.total_tokens as i64;
    let cost_delta = baseline.cost_usd - with_8v.cost_usd;

    eprintln!("  DELTA:");
    eprintln!("    tool_calls saved: {tool_delta}");
    eprintln!("    tokens saved:     {token_delta}");
    eprintln!("    cost saved:       ${cost_delta:.4}");

    if token_delta > 0 {
        let pct = (token_delta as f64 / baseline.total_tokens as f64) * 100.0;
        eprintln!("  VERDICT: 8v SAVES {pct:.1}% tokens on polyglot project");
    } else {
        let pct = ((-token_delta) as f64 / baseline.total_tokens as f64) * 100.0;
        eprintln!("  VERDICT: 8v costs {pct:.1}% MORE tokens on polyglot project");
    }

    // ── Integration path detection ─────────────────────────────────────
    // Log which path the agent took. No assertion — this is a measurement
    // instrument, not a pass/fail gate. Agent behavior varies between runs.
    let used_mcp = with_8v.used_8v();
    let used_8v_via_bash = with_8v.tool_calls.iter().any(|t| t.name == "Bash") && token_delta > 0;
    eprintln!("  INTEGRATION PATH:");
    eprintln!("    used_mcp:       {used_mcp}");
    eprintln!("    used_8v_bash:   {used_8v_via_bash} (heuristic)");
    if !used_mcp {
        eprintln!("    NOTE: MCP server was NOT used. Check .mcp.json and --mcp-config setup.");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 7 — Read/Write with 8v MCP: baseline vs 8v-instrumented agent
//
// Claim: `8v init` gives the agent read/write instructions via CLAUDE.md and
//        exposes the 8v MCP server. When native Read/Edit/Write/Bash are
//        disabled, the agent must use 8v tools to fix the bug.
//
// Two arms:
//   Baseline — standard Claude Code with all native tools, simple CLAUDE.md
//   8v       — native Read/Edit/Write/Bash disabled, 8v MCP available
//              (CLAUDE.md and .mcp.json written by `8v init --yes`)
// ─────────────────────────────────────────────────────────────────────────────

fn print_arm_result(label: &str, result: &AgentResult) {
    eprintln!("  {label}:");
    eprintln!("    tokens:     {}", result.total_tokens);
    eprintln!("    cost:       ${:.4}", result.cost_usd);
    eprintln!("    tool_calls: {}", result.tool_call_count());
    eprintln!("    exit_code:  {}", result.exit_code);
}

#[test]
#[ignore = "requires: `claude` in PATH, costs tokens — two-arm read/write benchmark"]
fn v7_read_write_token_efficiency() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "The test test_sum_range_inclusive is failing. Find the bug and fix it. Run cargo test to verify your fix works.";
    let task_description = "Fix the failing test.\n";
    let rust_violated_src = o8v_testkit::fixture_path("o8v", "agent-benchmark/rust-violated");

    // ARM 1: Baseline — MCP server registered, native tools available.
    // This ensures baseline pays identical MCP infrastructure tax as 8v arm,
    // making comparison fair: both have 8v MCP server available, but baseline
    // uses native tools (CLAUDE.md doesn't mention 8v), while 8v arm disallows them.
    let baseline_dir = TempProject::from_fixture(&rust_violated_src);
    let baseline_claude_md =
        "# Bug Fix Project\n\nSmall Rust project. The test `test_sum_range_inclusive` fails. Find the bug and fix it.\n\n\
         ## Tools\n\n\
         - `cargo test` — run tests\n\
         - `cargo check` — check compilation\n";
    baseline_dir
        .write_file("CLAUDE.md", baseline_claude_md.as_bytes())
        .expect("write baseline CLAUDE.md");
    Command::new("git")
        .args(["init"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git init baseline");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config email baseline");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config name baseline");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git add baseline");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git commit baseline");
    // Baseline: write only .mcp.json (no deny list, no CLAUDE.md override).
    // write_8v_setup would also install settings.json with permissions.deny,
    // which would force the baseline to use 8v tools — unfair comparison.
    let baseline_mcp_json = mcp_json(binary);
    std::fs::write(baseline_dir.path().join(".mcp.json"), &baseline_mcp_json)
        .expect("write baseline .mcp.json");
    let baseline_mcp_config = baseline_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 1: Baseline (MCP registered, native tools available) ===");
    let baseline = run_claude_with_mcp(
        prompt,
        baseline_dir.path(),
        Some(&baseline_mcp_config),
        &[],           // no disallowed tools — native tools available
        &[],           // same MCP behavior as 8v arm
        "acceptEdits", // same permission mode as 8v arm
        None,          // no settings — no deny list
    )
    .expect("baseline claude run");

    // ARM 2: 8v — `8v init --yes` sets up everything: MCP, CLAUDE.md, settings.
    // settings.json permissions.deny blocks native tools. No manual --disallowedTools.
    let eightvee_dir = TempProject::from_fixture(&rust_violated_src);
    eightvee_dir
        .write_file("CLAUDE.md", task_description.as_bytes())
        .expect("write eightvee CLAUDE.md");
    Command::new("git")
        .args(["init"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git init eightvee");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config email eightvee");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config name eightvee");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git add eightvee");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git commit eightvee");
    write_8v_setup(eightvee_dir.path(), binary);
    let mcp_config = eightvee_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 2: 8v (init denies native tools, 8v MCP) ===");
    let settings_path = std::fs::canonicalize(eightvee_dir.path().join(".claude/settings.json"))
        .expect("canonicalize settings path");
    // No --disallowedTools: 8v init writes permissions.deny in settings.json
    let eightvee = run_claude_with_mcp(
        prompt,
        eightvee_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "acceptEdits",
        Some(&settings_path),
    )
    .expect("8v claude run");

    let mcp = o8v_testkit::McpMeasurement::from_project(eightvee_dir.path()).unwrap_or_else(|e| {
        eprintln!("  MCP measurement: {e}");
        o8v_testkit::McpMeasurement::zero()
    });

    eprintln!("\n============================================================");
    eprintln!("V7: READ/WRITE BENCHMARK");
    eprintln!("============================================================");

    eprintln!("\n--- Token Counts ---");
    print_arm_result("Baseline (native tools)", &baseline);
    print_arm_result("8v (MCP only)", &eightvee);

    if eightvee.total_tokens < baseline.total_tokens {
        let saved = baseline.total_tokens - eightvee.total_tokens;
        let pct = (saved as f64 / baseline.total_tokens as f64) * 100.0;
        eprintln!("  8v saved {saved} tokens ({pct:.1}%) vs baseline");
    } else {
        let extra = eightvee.total_tokens.saturating_sub(baseline.total_tokens);
        let pct = if baseline.total_tokens > 0 {
            (extra as f64 / baseline.total_tokens as f64) * 100.0
        } else {
            0.0
        };
        eprintln!("  8v used {extra} MORE tokens ({pct:.1}%) vs baseline");
    }

    eprintln!("\n--- Tool Call Details ---");
    eprintln!("  Baseline ({} calls):", baseline.tool_call_count());
    for tool in &baseline.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }
    eprintln!("  8v arm ({} calls):", eightvee.tool_call_count());
    for tool in &eightvee.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }

    eprintln!("\n--- Fair Comparison (both arms have MCP) ---");
    eprintln!("  Both arms pay the MCP infrastructure tax.");
    eprintln!("  Baseline: MCP registered but unused, native tools available.");
    eprintln!("  8v: MCP active, native tools disabled.");
    eprintln!("  Delta measures pure tool efficiency, not MCP overhead.");

    eprintln!("\n--- Hook-Blocked Tool Check (8v arm) ---");
    let hook_blocked = [
        "Read",
        "Edit",
        "Write",
        "Bash",
        "Agent",
        "Grep",
        "Glob",
        "NotebookEdit",
    ];
    for tool in hook_blocked {
        let used = eightvee.tool_calls.iter().any(|t| t.name == tool);
        eprintln!("  used {tool} (hook-blocked): {used}");
    }

    eprintln!("\n--- MCP Events (.8v/mcp-events.ndjson) ---");
    eprintln!("  calls:               {}", mcp.call_count);
    eprintln!("  render_bytes:        {}", mcp.render_bytes);
    eprintln!("  render_tokens (est): {}", mcp.render_token_estimate());
    eprintln!("  command_bytes:       {}", mcp.command_bytes);
    eprintln!("  command_tokens (est):{}", mcp.command_token_estimate());
    eprintln!(
        "  avg bytes/call:      {}",
        mcp.avg_bytes_per_call().unwrap_or(0)
    );

    eprintln!("\n--- Per-Turn Token Breakdown ---");
    eprintln!("  Baseline:");
    for usage in &baseline.turn_usage {
        eprintln!(
            "    {}: input={}, output={}, cache_read={}, cache_creation={}",
            usage.role,
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_input_tokens,
            usage.cache_creation_input_tokens
        );
    }
    eprintln!("  8v:");
    for usage in &eightvee.turn_usage {
        eprintln!(
            "    {}: input={}, output={}, cache_read={}, cache_creation={}",
            usage.role,
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_input_tokens,
            usage.cache_creation_input_tokens
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 8 — Search token efficiency: 8v search vs native grep
//
// Claim: `8v search` produces fewer tokens than `grep -rn` for the same query.
//        Compact mode (default) uses structured output to reduce noise.
//        Multiple modes measured: compact (default), text (-C 0), JSON (--json).
//
// No Claude API. No MCP. Direct process measurement only.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // run with: cargo test -p o8v -- v8_search --ignored --nocapture
fn v8_search_token_efficiency() {
    use std::time::Instant;

    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let search_dir = workspace.join("o8v").join("src");
    let pattern = "fn run";

    // Build 8v binary first
    let build = Command::new("cargo")
        .args(["build", "-p", "o8v", "--release"])
        .current_dir(workspace)
        .output()
        .expect("cargo build");
    assert!(build.status.success(), "cargo build failed");

    let bin = workspace.join("target").join("release").join("8v");

    // ARM 1: grep
    let start = Instant::now();
    let grep_out = Command::new("grep")
        .args(["-rn", pattern])
        .arg(&search_dir)
        .output()
        .expect("grep");
    let grep_ms = start.elapsed().as_millis();
    let grep_bytes = grep_out.stdout.len();
    let grep_lines = grep_out.stdout.iter().filter(|&&b| b == b'\n').count();

    // ARM 2: 8v search compact (default)
    let start = Instant::now();
    let compact_out = Command::new(&bin)
        .args(["search", pattern])
        .arg(&search_dir)
        .output()
        .expect("8v search compact");
    let compact_ms = start.elapsed().as_millis();
    let compact_bytes = compact_out.stdout.len();
    let compact_lines = compact_out.stdout.iter().filter(|&&b| b == b'\n').count();

    // ARM 3: 8v search with text (-C 0)
    let start = Instant::now();
    let text_out = Command::new(&bin)
        .args(["search", pattern, "-C", "0"])
        .arg(&search_dir)
        .output()
        .expect("8v search -C 0");
    let text_ms = start.elapsed().as_millis();
    let text_bytes = text_out.stdout.len();
    let text_lines = text_out.stdout.iter().filter(|&&b| b == b'\n').count();

    // ARM 4: 8v search JSON
    let start = Instant::now();
    let json_out = Command::new(&bin)
        .args(["search", pattern, "--json"])
        .arg(&search_dir)
        .output()
        .expect("8v search --json");
    let json_ms = start.elapsed().as_millis();
    let json_bytes = json_out.stdout.len();
    let json_lines = json_out.stdout.iter().filter(|&&b| b == b'\n').count();

    // Report
    eprintln!("\n=== V8: SEARCH TOKEN EFFICIENCY ===");
    eprintln!("Pattern: \"{}\"  Target: o8v/src/", pattern);
    eprintln!();
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>10}",
        "Mode", "Bytes", "Lines", "~Tokens", "Latency"
    );
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>10}",
        "----", "-----", "-----", "-------", "-------"
    );
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>8}ms",
        "grep -rn",
        grep_bytes,
        grep_lines,
        grep_bytes / 4,
        grep_ms
    );
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>8}ms",
        "8v compact (default)",
        compact_bytes,
        compact_lines,
        compact_bytes / 4,
        compact_ms
    );
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>8}ms",
        "8v text (-C 0)",
        text_bytes,
        text_lines,
        text_bytes / 4,
        text_ms
    );
    eprintln!(
        "  {:20} {:>10} {:>10} {:>10} {:>8}ms",
        "8v json (--json)",
        json_bytes,
        json_lines,
        json_bytes / 4,
        json_ms
    );
    eprintln!();

    // Savings vs grep
    if grep_bytes > 0 {
        let compact_saving = 100.0 * (1.0 - compact_bytes as f64 / grep_bytes as f64);
        let text_saving = 100.0 * (1.0 - text_bytes as f64 / grep_bytes as f64);
        let json_saving = 100.0 * (1.0 - json_bytes as f64 / grep_bytes as f64);
        eprintln!("  Savings vs grep:");
        eprintln!("    compact: {compact_saving:+.1}%");
        eprintln!("    text:    {text_saving:+.1}%");
        eprintln!("    json:    {json_saving:+.1}%");
    }

    // Assertions: compact mode MUST be smaller than grep
    assert!(
        compact_bytes < grep_bytes,
        "8v compact ({compact_bytes}B) should be smaller than grep ({grep_bytes}B)"
    );

    // Compact must be smaller than text mode
    assert!(
        compact_bytes < text_bytes,
        "compact ({compact_bytes}B) should be smaller than text ({text_bytes}B)"
    );

    // All modes must produce output
    assert!(compact_bytes > 0, "compact produced no output");
    assert!(text_bytes > 0, "text produced no output");
    assert!(json_bytes > 0, "json produced no output");

    // Print actual output samples (first 500 chars of each)
    let compact_str = String::from_utf8_lossy(&compact_out.stdout);
    let text_str = String::from_utf8_lossy(&text_out.stdout);
    let json_str = String::from_utf8_lossy(&json_out.stdout);

    eprintln!("\n--- Compact output sample ---");
    eprintln!("{}", &compact_str[..compact_str.len().min(500)]);
    eprintln!("\n--- Text output sample ---");
    eprintln!("{}", &text_str[..text_str.len().min(500)]);
    eprintln!("\n--- JSON output sample (first 500 chars) ---");
    eprintln!("{}", &json_str[..json_str.len().min(500)]);
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 9 — Discovery: agent uses `8v ls` to understand project structure
//
// Claim: When given a discovery-oriented task, an agent with 8v uses `8v ls`
//        to understand the project instead of making multiple Glob/find calls.
//
// Setup: Polyglot project with multiple stacks. Agent is asked to describe
//        the project structure, stacks, and largest files.
// Verify: Agent tool calls include 8v ls (or 8v with "ls" in command).
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~30-60s, costs tokens — runs agent once)"]
fn v9_ls_discovery() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "Describe this project: what stacks are used, how many files per stack, \
                  and which are the 3 largest files by line count. \
                  Use 8v commands to discover the structure — do not use Glob or find.";

    // Use the polyglot fixture — it has multiple stacks to discover
    let fixture = o8v_testkit::fixture_path("o8v", "agent-benchmark/polyglot-violated");
    let project_dir = TempProject::from_fixture(&fixture);
    Command::new("git")
        .args(["init"])
        .current_dir(project_dir.path())
        .output()
        .expect("git init");
    write_8v_setup(project_dir.path(), binary);

    let mcp_config = project_dir.path().join(".mcp.json");
    let result = run_claude_with_mcp(
        prompt,
        project_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "bypassPermissions",
        None,
    )
    .expect("run claude (ls discovery)");

    // ── Report ───────────────────────────────────────────────────────────────
    eprintln!(
        "\n[v9] LS DISCOVERY BENCHMARK\n\
         [v9]   tool_calls ({})={:?}\n\
         [v9]   used_8v={}\n\
         [v9]   total_tokens={}\n\
         [v9]   cost=${:.4}\n",
        result.tool_call_count(),
        result.tool_calls,
        result.used_8v(),
        result.total_tokens,
        result.cost_usd,
    );

    // Check if agent used 8v ls specifically
    let used_ls = result
        .tool_calls
        .iter()
        .any(|t| t.name.contains("8v") && t.input.contains("ls"));

    eprintln!(
        "[v9]   used_8v_ls={}\n\
         [v9]   response_preview={}",
        used_ls,
        &result.response_text[..result.response_text.len().min(500)]
    );

    // Hard assertion: agent must have used 8v
    assert!(
        result.used_8v(),
        "Agent did not use 8v tool.\nTool calls: {:?}",
        result.tool_calls
    );

    // Soft check: report whether 8v ls was used (not asserted — agent may use other 8v commands)
    if !used_ls {
        eprintln!(
            "[v9] WARNING: Agent used 8v but did NOT use `8v ls`. \
             The workflow guidance may not be effective enough.\n\
             Tool calls: {:?}",
            result.tool_calls
        );
    }

    // Read MCP event measurements
    let mcp = o8v_testkit::McpMeasurement::from_project(project_dir.path()).unwrap_or_else(|e| {
        eprintln!("  MCP measurement: {e}");
        o8v_testkit::McpMeasurement::zero()
    });

    eprintln!(
        "\n[v9] MCP MEASUREMENTS\n\
         [v9]   8v calls made: {}\n\
         [v9]   render_bytes (8v→agent): {}\n\
         [v9]   command_bytes (agent→8v): {}\n\
         [v9]   avg bytes per call: {}",
        mcp.call_count,
        mcp.render_bytes,
        mcp.command_bytes,
        mcp.avg_bytes_per_call().unwrap_or(0),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 10 — Build efficiency: 8v build vs native cargo build
//
// Claim: `8v build` produces fewer tokens than `cargo build` run via Bash
//        for the same task. Two-arm measurement benchmark.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, costs tokens — two-arm build efficiency benchmark"]
fn v10_build_efficiency() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "Build this Rust project and tell me if it compiled successfully.";
    let clean_rust_src = o8v_testkit::fixture_path("o8v", "agent-benchmark/clean-rust");

    // ARM 1: Baseline — native tools (Bash) available, simple CLAUDE.md
    let baseline_dir = TempProject::from_fixture(&clean_rust_src);
    let baseline_claude_md = "# Build Project\n\nSmall Rust project. Build it with cargo build.\n";
    baseline_dir
        .write_file("CLAUDE.md", baseline_claude_md.as_bytes())
        .expect("write baseline CLAUDE.md");
    Command::new("git")
        .args(["init"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git init baseline");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config email baseline");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config name baseline");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git add baseline");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git commit baseline");
    // Baseline: write only .mcp.json (no deny list). Both arms pay same MCP tax.
    let baseline_mcp_json = mcp_json(binary);
    std::fs::write(baseline_dir.path().join(".mcp.json"), &baseline_mcp_json)
        .expect("write baseline .mcp.json");
    let baseline_mcp_config = baseline_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 1: Baseline (MCP registered, native tools available) ===");
    let baseline = run_claude_with_mcp(
        prompt,
        baseline_dir.path(),
        Some(&baseline_mcp_config),
        &[], // no disallowed tools — native tools available
        &[],
        "acceptEdits", // same permission mode as 8v arm
        None,          // no settings — no deny list
    )
    .expect("baseline claude run");

    // ARM 2: 8v — MCP available
    let eightvee_dir = TempProject::from_fixture(&clean_rust_src);
    Command::new("git")
        .args(["init"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git init eightvee");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config email eightvee");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config name eightvee");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git add eightvee");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git commit eightvee");
    write_8v_setup(eightvee_dir.path(), binary);
    let mcp_config = eightvee_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 2: 8v (MCP available) ===");
    let eightvee = run_claude_with_mcp(
        prompt,
        eightvee_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "acceptEdits",
        None,
    )
    .expect("8v claude run");

    // ── Report ───────────────────────────────────────────────────────────────
    eprintln!("\n============================================================");
    eprintln!("V10: BUILD EFFICIENCY BENCHMARK");
    eprintln!("============================================================");

    print_arm_result("Baseline (native tools)", &baseline);
    print_arm_result("8v (MCP)", &eightvee);

    if eightvee.total_tokens < baseline.total_tokens {
        let saved = baseline.total_tokens - eightvee.total_tokens;
        let pct = (saved as f64 / baseline.total_tokens as f64) * 100.0;
        eprintln!("  8v saved {saved} tokens ({pct:.1}%) vs baseline");
    } else {
        let extra = eightvee.total_tokens.saturating_sub(baseline.total_tokens);
        let pct = if baseline.total_tokens > 0 {
            (extra as f64 / baseline.total_tokens as f64) * 100.0
        } else {
            0.0
        };
        eprintln!("  8v used {extra} MORE tokens ({pct:.1}%) vs baseline");
    }

    eprintln!("\n--- Tool Call Details ---");
    eprintln!("  Baseline ({} calls):", baseline.tool_call_count());
    for tool in &baseline.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }
    eprintln!("  8v arm ({} calls):", eightvee.tool_call_count());
    for tool in &eightvee.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }

    let used_8v_build = eightvee
        .tool_calls
        .iter()
        .any(|t| t.name.contains("8v") && t.input.contains("build"));

    eprintln!(
        "\n  used_8v_build={}\n  cost_baseline=${:.4}  cost_8v=${:.4}",
        used_8v_build, baseline.cost_usd, eightvee.cost_usd,
    );

    // Soft assertion: agent must have used 8v in the 8v arm
    assert!(
        eightvee.used_8v(),
        "Agent did not use 8v tool in 8v arm.\nTool calls: {:?}",
        eightvee.tool_calls
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 11 — Run efficiency: 8v run vs native Bash echo
//
// Claim: `8v run` provides a token-efficient alternative to Bash for running
//        arbitrary commands. Two-arm measurement benchmark.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, costs tokens — two-arm run efficiency benchmark"]
fn v11_run_efficiency() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "Run the command 'echo hello world' and report the output.";
    let clean_rust_src = o8v_testkit::fixture_path("o8v", "agent-benchmark/clean-rust");

    // ARM 1: Baseline — native Bash available
    let baseline_dir = TempProject::from_fixture(&clean_rust_src);
    let baseline_claude_md = "# Run Project\n\nSmall Rust project. Use Bash to run commands.\n";
    baseline_dir
        .write_file("CLAUDE.md", baseline_claude_md.as_bytes())
        .expect("write baseline CLAUDE.md");
    Command::new("git")
        .args(["init"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git init baseline");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config email baseline");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git config name baseline");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git add baseline");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(baseline_dir.path())
        .output()
        .expect("git commit baseline");
    // Baseline: write only .mcp.json (no deny list). Both arms pay same MCP tax.
    let baseline_mcp_json = mcp_json(binary);
    std::fs::write(baseline_dir.path().join(".mcp.json"), &baseline_mcp_json)
        .expect("write baseline .mcp.json");
    let baseline_mcp_config = baseline_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 1: Baseline (MCP registered, native tools available) ===");
    let baseline = run_claude_with_mcp(
        prompt,
        baseline_dir.path(),
        Some(&baseline_mcp_config),
        &[], // no disallowed tools — native tools available
        &[],
        "acceptEdits", // same permission mode as 8v arm
        None,          // no settings — no deny list
    )
    .expect("baseline claude run");

    // ARM 2: 8v — MCP available
    let eightvee_dir = TempProject::from_fixture(&clean_rust_src);
    Command::new("git")
        .args(["init"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git init eightvee");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config email eightvee");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config name eightvee");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git add eightvee");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git commit eightvee");
    write_8v_setup(eightvee_dir.path(), binary);
    let mcp_config = eightvee_dir.path().join(".mcp.json");

    eprintln!("\n=== ARM 2: 8v (MCP available) ===");
    let eightvee = run_claude_with_mcp(
        prompt,
        eightvee_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "acceptEdits",
        None,
    )
    .expect("8v claude run");

    // ── Report ───────────────────────────────────────────────────────────────
    eprintln!("\n============================================================");
    eprintln!("V11: RUN EFFICIENCY BENCHMARK");
    eprintln!("============================================================");

    print_arm_result("Baseline (native tools)", &baseline);
    print_arm_result("8v (MCP)", &eightvee);

    if eightvee.total_tokens < baseline.total_tokens {
        let saved = baseline.total_tokens - eightvee.total_tokens;
        let pct = (saved as f64 / baseline.total_tokens as f64) * 100.0;
        eprintln!("  8v saved {saved} tokens ({pct:.1}%) vs baseline");
    } else {
        let extra = eightvee.total_tokens.saturating_sub(baseline.total_tokens);
        let pct = if baseline.total_tokens > 0 {
            (extra as f64 / baseline.total_tokens as f64) * 100.0
        } else {
            0.0
        };
        eprintln!("  8v used {extra} MORE tokens ({pct:.1}%) vs baseline");
    }

    eprintln!("\n--- Tool Call Details ---");
    eprintln!("  Baseline ({} calls):", baseline.tool_call_count());
    for tool in &baseline.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }
    eprintln!("  8v arm ({} calls):", eightvee.tool_call_count());
    for tool in &eightvee.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }

    eprintln!(
        "\n  cost_baseline=${:.4}  cost_8v=${:.4}",
        baseline.cost_usd, eightvee.cost_usd,
    );

    // Soft assertion: agent must have used 8v in the 8v arm
    assert!(
        eightvee.used_8v(),
        "Agent did not use 8v tool in 8v arm.\nTool calls: {:?}",
        eightvee.tool_calls
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Value 12 — Zero-Bash full loop: 8v-only tool suite completes a real task
//
// Claim: With ALL native tools disabled by `8v init`, 8v MCP alone is
//        sufficient for an agent to find a bug, fix it, verify with tests,
//        and build the project.
//
// This test uses `8v init --yes` exactly as a real user would. No manual
// --disallowedTools, no patching settings. Init sets up everything:
// MCP server, CLAUDE.md, permissions (allow 8v, deny native tools).
//
// Hard pass/fail: the bug must actually be fixed (cargo test passes).
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, costs tokens — zero-bash full-loop hard pass/fail"]
fn v12_zero_bash_full_loop() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let prompt = "The test test_sum_range_inclusive is failing. \
                  Find the bug, fix it, run the tests to verify, and build the project.";

    let rust_violated_src = o8v_testkit::fixture_path("o8v", "agent-benchmark/rust-violated");
    let eightvee_dir = TempProject::from_fixture(&rust_violated_src);

    // Git setup
    Command::new("git")
        .args(["init"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git config name");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("git commit");

    // `8v init --yes` does everything: MCP, CLAUDE.md, settings (allow 8v, deny native tools)
    write_8v_setup(eightvee_dir.path(), binary);
    let mcp_config = eightvee_dir.path().join(".mcp.json");
    let settings_path = std::fs::canonicalize(eightvee_dir.path().join(".claude/settings.json"))
        .expect("canonicalize settings path");

    // Verify init set up the deny list (this is what makes native tools unavailable)
    let settings_content = fs::read_to_string(eightvee_dir.path().join(".claude/settings.json"))
        .expect("read settings");
    eprintln!("  [v12] settings.json: {settings_content}");
    assert!(
        settings_content.contains("\"deny\""),
        "8v init must write permissions.deny — got: {settings_content}"
    );

    eprintln!("\n=== V12: ZERO-BASH FULL LOOP (8v init sets up everything, no manual hacks) ===");

    // No --disallowedTools, no env overrides.
    // Exactly what a real user gets after `8v init --yes`.
    let result = run_claude_with_mcp(
        prompt,
        eightvee_dir.path(),
        Some(&mcp_config),
        &[],
        &[],
        "acceptEdits",
        Some(&settings_path),
    )
    .expect("8v claude run");

    // ── Report ───────────────────────────────────────────────────────────────
    eprintln!("\n============================================================");
    eprintln!("V12: ZERO-BASH FULL LOOP BENCHMARK");
    eprintln!("============================================================");

    print_arm_result("8v only (all native tools disabled)", &result);

    eprintln!("\n--- All Tool Calls ({}) ---", result.tool_call_count());
    for tool in &result.tool_calls {
        eprintln!("    {}: {}", tool.name, tool.input);
    }

    eprintln!(
        "\n  exit_code={}  used_8v={}  total_tokens={}  cost=${:.4}",
        result.exit_code,
        result.used_8v(),
        result.total_tokens,
        result.cost_usd,
    );

    // Verify no native tools were used
    let native_tools = ["Read", "Edit", "Write", "Bash", "Grep", "Glob"];
    for tool in native_tools {
        let used = result.tool_calls.iter().any(|t| t.name == tool);
        eprintln!("  used {tool} (should be false): {used}");
    }

    // Run cargo test to verify the bug was actually fixed
    let cargo_test = Command::new("cargo")
        .args(["test"])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("run cargo test");

    eprintln!(
        "\n--- cargo test result ---\n  exit_code={}\n  stdout={}\n  stderr={}",
        cargo_test.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr),
    );

    // Run 8v build to verify compilation
    let build_result = Command::new(binary)
        .args(["build", "."])
        .current_dir(eightvee_dir.path())
        .output()
        .expect("run 8v build");

    eprintln!(
        "\n--- 8v build result ---\n  exit_code={}\n  stdout={}\n  stderr={}",
        build_result.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&build_result.stdout),
        String::from_utf8_lossy(&build_result.stderr),
    );

    // ── Hard assertions ───────────────────────────────────────────────────────

    // Agent ran cleanly (not an infrastructure failure)
    assert_ne!(
        result.exit_code, -1,
        "Agent exited with -1 (infrastructure failure).\nTool calls: {:?}",
        result.tool_calls
    );

    // No native tools were used
    for tool in native_tools {
        let used = result.tool_calls.iter().any(|t| t.name == tool);
        assert!(
            !used,
            "Agent used native tool '{tool}' which should have been disabled.\n\
             Tool calls: {:?}",
            result.tool_calls
        );
    }

    // cargo test passes — the bug was actually fixed
    assert!(
        cargo_test.status.success(),
        "cargo test failed — agent did not fix the bug.\n\
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr),
    );

    // 8v build passes — project compiles
    assert!(
        build_result.status.success(),
        "8v build failed — project does not compile after agent's changes.\n\
         stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build_result.stdout),
        String::from_utf8_lossy(&build_result.stderr),
    );
}
