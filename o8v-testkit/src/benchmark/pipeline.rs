// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The benchmark pipeline: setup → run → collect → verify → persist.
//!
//! `run_scenario()` is the single entry point. It handles everything.
//! The caller gets a `Observation` back — already persisted.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use super::claude::{AgentResult, ClaudeDriver};
use super::codex::CodexDriver;
use super::profiles::ToolProfile;
use super::store::BenchmarkStore;
use super::types::*;
use crate::scaffold::{fixture_path, TempProject};

/// Execute a benchmark scenario end-to-end.
///
/// 1. Set up an isolated project from the fixture
/// 2. Run the Claude agent
/// 3. Collect internal events (8v event reader)
/// 4. Run verification (cargo test, 8v check)
/// 5. Persist the Observation
/// 6. Return the record for assertions
///
/// Panics on infrastructure failures (cannot create temp dir, cannot spawn claude).
/// Agent failures are recorded in the Observation, not panicked.
pub fn run_scenario(
    scenario: &Scenario,
    binary: &str,
    persist: bool,
    profile: ToolProfile,
) -> Observation {
    // NOTE: Benchmark scenarios must run sequentially (--test-threads=1) because events.ndjson is global.
    let prompt = scenario.task.resolved_prompt();

    // ── 0. Read binary version ──────────────────────────────────────────────
    let version = {
        let version_output = Command::new(binary)
            .arg("--version")
            .output()
            .expect("8v --version");
        String::from_utf8_lossy(&version_output.stdout)
            .trim()
            .to_string()
    };

    // Warn if the working tree is dirty — benchmark results from a dirty tree
    // cannot be attributed to a specific commit and should not be published.
    if let Ok(status) = Command::new("git").args(["status", "--porcelain"]).output() {
        let dirty = !status.stdout.is_empty();
        if dirty {
            eprintln!(
                "  [benchmark] WARNING: working tree is dirty — commit changes before \
                 publishing results. Results will be tagged with the current HEAD but \
                 do not reflect a clean build."
            );
        }
    }

    // ── 1. Setup ────────────────────────────────────────────────────────────
    let fixture = fixture_path("o8v", scenario.task.fixture);
    let project = TempProject::from_fixture(&fixture);
    setup_git(project.path());

    // ── 1a. Profile setup ───────────────────────────────────────────────────
    let profile_harness = profile.harness();
    let artifacts = profile_harness
        .setup(project.path(), scenario.env.agent)
        .expect("profile setup failed");
    let profile_version = profile_harness.version();

    let mcp_config;
    let settings_path;

    match scenario.env.agent {
        Agent::Claude => {
            if scenario.env.setup_8v {
                run_8v_init(project.path(), binary);
                settings_path = Some(
                    std::fs::canonicalize(project.path().join(".claude/settings.json"))
                        .expect("8v init --yes ran but .claude/settings.json is missing — this is a bug in 8v init"),
                );
                mcp_config = Some(project.path().join(".mcp.json"));
            } else {
                // Baseline: truly bare. No MCP, no settings. The 8v schema
                // token cost is reported separately as a tax column.
                settings_path = None;
                mcp_config = None;
            }
        }
        Agent::Codex => {
            if scenario.env.setup_8v {
                run_8v_init(project.path(), binary);
                write_codex_config(project.path(), binary);
            }
            mcp_config = None;
            settings_path = None;
        }
    }

    // ── 2. Run agent ────────────────────────────────────────────────────────
    let start_ms = unix_ms();

    // Clean events before this run so we only collect this scenario's events.
    // When persist=false the caller (experiment.rs) owns isolation — skip here.
    let events_path = events_ndjson_path();
    if persist {
        match std::fs::remove_file(&events_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!(
                "  [benchmark] warning: failed to remove events file {}: {}",
                events_path.display(),
                e
            ),
        }
    }

    let agent_result = match scenario.env.agent {
        Agent::Claude => ClaudeDriver::run(
            &prompt,
            project.path(),
            mcp_config.as_deref(),
            scenario.env.permission_mode,
            settings_path.as_deref(),
            &artifacts,
        )
        .expect("claude driver failed"),
        Agent::Codex => {
            // When 8v MCP is registered, shell access is redundant and we
            // disable it so the agent goes through MCP.
            let disable_shell = scenario.env.setup_8v;
            CodexDriver::run(&prompt, project.path(), disable_shell).expect("codex driver failed")
        }
    };

    if agent_result.parse_errors > 0 {
        eprintln!(
            "  [benchmark] warning: {} unparseable line(s) in agent stream — \
             metrics for this run may be incomplete",
            agent_result.parse_errors
        );
    }

    // ── 3. Collect internal events ──────────────────────────────────────────
    let events = collect_events(&events_path);

    // ── 4. Verify ───────────────────────────────────────────────────────────
    let verification = run_verification(project.path(), binary);

    // ── 5. Build record ─────────────────────────────────────────────────────
    let git_commit = current_git_commit();
    let record = Observation {
        scenario: scenario.name.to_string(),
        task_name: scenario.task.name.to_string(),
        timestamp_ms: start_ms,
        git_commit,
        version,
        total_tokens: agent_result.total_tokens,
        cost_usd: agent_result.cost_usd,
        exit_code: agent_result.exit_code,
        tool_names: agent_result
            .tool_calls
            .iter()
            .map(|t| t.name.clone())
            .collect(),
        turns: agent_result
            .turn_usage
            .iter()
            .map(|t| TurnRecord {
                role: TurnRole::from_str(&t.role),
                input_tokens: t.input_tokens,
                output_tokens: t.output_tokens,
                cache_read_input_tokens: t.cache_read_input_tokens,
                cache_creation_input_tokens: t.cache_creation_input_tokens,
            })
            .collect(),
        init_message_bytes: agent_result.init_message_bytes,
        response_text: agent_result.response_text.clone(),
        model: agent_result.model.clone(),
        session_id: agent_result.session_id.clone(),
        stop_reason: agent_result.stop_reason.clone(),
        is_error: agent_result.is_error,
        cache_read_input_tokens: agent_result.cache_read_tokens,
        cache_creation_input_tokens: agent_result.cache_creation_tokens,
        input_tokens: agent_result.input_tokens,
        output_tokens: agent_result.output_tokens,
        turn_count: agent_result.turn_usage.len() as u32,
        event_count: events.count,
        event_output_bytes: events.output_bytes,
        event_command_bytes: events.command_bytes,
        event_total_duration_ms: events.total_duration_ms,
        agent_name: events.agent.name,
        agent_version: events.agent.version,
        mcp_protocol_version: events.agent.protocol_version,
        agent_capabilities: events.agent.capabilities,
        verification,
        feedback: None, // TODO: agent feedback in a later increment
        tool_calls_detail: agent_result.tool_calls_detail.clone(),
        profile,
        profile_version,
    };

    // ── 6. Persist ──────────────────────────────────────────────────────────
    // When persist=false the caller (experiment.rs) owns persistence — skip here.
    if persist {
        match BenchmarkStore::open() {
            Ok(store) => {
                if let Err(e) = store.append(&record) {
                    eprintln!("  [benchmark] warning: failed to persist record: {e}");
                }
            }
            Err(e) => {
                eprintln!("  [benchmark] warning: failed to open benchmark store: {e}");
            }
        }
    }

    // ── 7. Print summary ────────────────────────────────────────────────────
    print_summary(scenario.name, &agent_result, &record);

    record
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(super) fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .expect("system clock is before Unix epoch")
}

fn setup_git(project: &Path) {
    let out = Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .expect("spawn git init");
    assert!(
        out.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(project)
        .output()
        .expect("spawn git config email");
    assert!(
        out.status.success(),
        "git config user.email failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(project)
        .output()
        .expect("spawn git config name");
    assert!(
        out.status.success(),
        "git config user.name failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new("git")
        .args(["add", "-A"])
        .current_dir(project)
        .output()
        .expect("spawn git add");
    assert!(
        out.status.success(),
        "git add -A failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(project)
        .output()
        .expect("spawn git commit");
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run_8v_init(project: &Path, binary: &str) {
    // --mcp-command writes the test binary path directly into .mcp.json, so
    // the spawned MCP server is the same binary under test. No post-init
    // patching — single source of truth.
    let output = Command::new(binary)
        .args(["init", "--yes", "--mcp-command", binary])
        .current_dir(project)
        .output()
        .expect("run 8v init");
    assert!(
        output.status.success(),
        "8v init --yes --mcp-command {binary} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_codex_config(project: &Path, binary: &str) {
    let codex_dir = project.join(".codex");
    std::fs::create_dir_all(&codex_dir).expect("create .codex dir");

    // Use the `toml` crate to serialize so binary paths with backslashes or
    // quotes are escaped correctly without manual string manipulation.
    let config = {
        let mut root = toml::Table::new();
        let mut mcp_servers = toml::Table::new();
        let mut server = toml::Table::new();
        server.insert("command".into(), toml::Value::String(binary.to_string()));
        server.insert(
            "args".into(),
            toml::Value::Array(vec![toml::Value::String("mcp".into())]),
        );
        mcp_servers.insert("8v".into(), toml::Value::Table(server));
        root.insert("mcp_servers".into(), toml::Value::Table(mcp_servers));
        toml::to_string(&root).expect("serialize codex config.toml")
    };
    std::fs::write(codex_dir.join("config.toml"), config.as_bytes())
        .expect("write .codex/config.toml");
}

pub(super) fn events_ndjson_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .expect("[benchmark] HOME environment variable is not set — cannot locate events.ndjson; this is a configuration error");
    std::path::PathBuf::from(home)
        .join(".8v")
        .join("events.ndjson")
}

/// Agent identity extracted from MCP handshake events.
#[derive(Default)]
struct EventAgentInfo {
    name: Option<String>,
    version: Option<String>,
    protocol_version: Option<String>,
    capabilities: Vec<String>,
}

struct CollectedEvents {
    count: usize,
    output_bytes: u64,
    command_bytes: u64,
    total_duration_ms: u64,
    agent: EventAgentInfo,
}

fn collect_events(events_path: &Path) -> CollectedEvents {
    let content = match std::fs::read_to_string(events_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return CollectedEvents {
                count: 0,
                output_bytes: 0,
                command_bytes: 0,
                total_duration_ms: 0,
                agent: EventAgentInfo::default(),
            }
        }
        Err(e) => panic!(
            "[benchmark] failed to read events file {}: {}",
            events_path.display(),
            e
        ),
    };

    let mut count = 0usize;
    let mut output_bytes = 0u64;
    let mut command_bytes = 0u64;
    let mut total_duration_ms = 0u64;
    let mut agent = EventAgentInfo::default();
    let mut parse_errors = 0usize;
    let mut no_type_events = 0usize;

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(line) else {
            parse_errors += 1;
            eprintln!(
                "  [benchmark] warning: unparseable event line {}: {}",
                i + 1,
                line
            );
            continue;
        };
        let Some(event_type) = raw.get("event").and_then(|v| v.as_str()) else {
            no_type_events += 1;
            eprintln!(
                "  [benchmark] warning: event line {} has no 'event' field: {}",
                i + 1,
                line
            );
            continue;
        };
        match event_type {
            "CommandStarted" => {
                count += 1;
                if let Some(b) = raw.get("command_bytes").and_then(|v| v.as_u64()) {
                    command_bytes += b;
                }
                if agent.name.is_none() {
                    if let Some(info) = raw.get("agent_info") {
                        agent.name = info.get("name").and_then(|v| v.as_str()).map(String::from);
                        agent.version = info
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        agent.protocol_version = info
                            .get("protocol_version")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        if let Some(caps) = info.get("capabilities").and_then(|v| v.as_array()) {
                            agent.capabilities = caps
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                        }
                    }
                }
            }
            "CommandCompleted" => {
                if let Some(b) = raw.get("output_bytes").and_then(|v| v.as_u64()) {
                    output_bytes += b;
                }
                if let Some(d) = raw.get("duration_ms").and_then(|v| v.as_u64()) {
                    total_duration_ms += d;
                }
            }
            _ => {}
        }
    }

    if parse_errors > 0 {
        eprintln!(
            "  [benchmark] warning: {} unparseable event line(s) skipped",
            parse_errors
        );
    }
    if no_type_events > 0 {
        eprintln!(
            "  [benchmark] warning: {} event line(s) with no 'event' field skipped",
            no_type_events
        );
    }
    CollectedEvents {
        count,
        output_bytes,
        command_bytes,
        total_duration_ms,
        agent,
    }
}

pub(super) fn run_verification(project: &Path, _binary: &str) -> Verification {
    // Python-only fixtures: pyproject.toml present, no Cargo.toml. Verify with
    // pytest. Build/check gates don't apply — leave as None so summaries treat
    // them as N/A rather than failures.
    let has_cargo = project.join("Cargo.toml").exists();
    let has_pyproject = project.join("pyproject.toml").exists();
    let has_go_mod = project.join("go.mod").exists();
    let has_tsconfig = project.join("tsconfig.json").exists();
    if has_pyproject && !has_cargo {
        let test_result = Some(
            Command::new("python3")
                .args(["-m", "pytest", "-q"])
                .current_dir(project)
                .output()
                .expect("[benchmark] failed to spawn `python3 -m pytest` — is python3 installed?")
                .status
                .success(),
        );
        return Verification {
            tests_pass: test_result,
            check_pass: None,
            build_pass: None,
        };
    }

    // Go-only fixtures: go.mod present, no Cargo.toml. Gate is `go test ./...`.
    if has_go_mod && !has_cargo {
        let test_result = Some(
            Command::new("go")
                .args(["test", "./..."])
                .current_dir(project)
                .output()
                .expect("[benchmark] failed to spawn `go test ./...` — is go installed?")
                .status
                .success(),
        );
        return Verification {
            tests_pass: test_result,
            check_pass: None,
            build_pass: None,
        };
    }

    // TypeScript-only fixtures: tsconfig.json present, no Cargo.toml. Gate is
    // `tsc --noEmit` — covers type-level bugs. Reuses `tests_pass` as the
    // single success signal per design (scenario-fix-typescript.md).
    if has_tsconfig && !has_cargo {
        let test_result = Some(
            Command::new("tsc")
                .args(["--noEmit"])
                .current_dir(project)
                .output()
                .expect("[benchmark] failed to spawn `tsc --noEmit` — is typescript installed (tsc on PATH)?")
                .status
                .success(),
        );
        return Verification {
            tests_pass: test_result,
            check_pass: None,
            build_pass: None,
        };
    }

    let test_result = Some(
        Command::new("cargo")
            .args(["test"])
            .current_dir(project)
            .output()
            .expect("[benchmark] failed to spawn `cargo test` — is cargo installed?")
            .status
            .success(),
    );

    let check_result = Some(
        match Command::new("cargo")
            .args(["clippy", "--", "-D", "warnings"])
            .current_dir(project)
            .output()
        {
            Ok(out) => out,
            Err(e) => panic!("[benchmark] failed to spawn `cargo clippy -- -D warnings`: {e}"),
        }
        .status
        .success(),
    );

    let build_result = Some(
        match Command::new("cargo")
            .args(["build"])
            .current_dir(project)
            .output()
        {
            Ok(out) => out,
            Err(e) => panic!("[benchmark] failed to spawn `cargo build`: {e}"),
        }
        .status
        .success(),
    );

    Verification {
        tests_pass: test_result,
        check_pass: check_result,
        build_pass: build_result,
    }
}

pub(super) fn current_git_commit() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("[benchmark] failed to spawn `git rev-parse HEAD` — git is a hard dependency for benchmarks");
    assert!(
        output.status.success(),
        "[benchmark] `git rev-parse HEAD` failed — are you in a git repository? stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("[benchmark] `git rev-parse HEAD` output is not valid UTF-8")
        .trim()
        .to_string()
}

fn print_summary(name: &str, agent: &AgentResult, record: &Observation) {
    let cost_str = match record.cost_usd {
        Some(c) => format!("${:.4}", c),
        None => "n/a".to_string(),
    };
    eprintln!("\n============================================================");
    eprintln!("BENCHMARK: {name}");
    eprintln!("============================================================");
    eprintln!("  tokens:          {}", record.total_tokens);
    eprintln!("  cost:            {cost_str}");
    eprintln!("  tool_calls:      {}", record.tool_names.len());
    eprintln!("  used_8v:         {}", agent.used_8v());
    eprintln!("  exit_code:       {}", record.exit_code);
    eprintln!("  events:          {}", record.event_count);
    eprintln!("  event_out_bytes: {}", record.event_output_bytes);
    eprintln!("  tests_pass:      {:?}", record.verification.tests_pass);
    eprintln!("  check_pass:      {:?}", record.verification.check_pass);
    eprintln!("  build_pass:      {:?}", record.verification.build_pass);
    eprintln!("  tool names:      {:?}", record.tool_names);
    for (i, detail) in record.tool_calls_detail.iter().enumerate() {
        let err = if detail.is_error { " [ERROR]" } else { "" };
        let input_preview: String = detail.input.chars().take(120).collect();
        eprintln!(
            "  tool[{i}]:         {} → {}{} ({} bytes out)",
            detail.name, input_preview, err, detail.output_bytes
        );
    }
    if let Some(name) = &record.agent_name {
        let ver = record.agent_version.as_deref().unwrap_or("?");
        let proto = record.mcp_protocol_version.as_deref().unwrap_or("?");
        eprintln!("  agent:           {name} v{ver} (MCP {proto})");
        if !record.agent_capabilities.is_empty() {
            eprintln!("  capabilities:    {:?}", record.agent_capabilities);
        }
    }
}
