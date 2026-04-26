// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v hook` — runs the compiled binary with an isolated
//! HOME directory, piping JSON payloads on stdin and asserting event output.

use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Create an isolated temp dir. Storage will land at `dir/.8v/`.
fn home() -> TempDir {
    TempDir::new().expect("create temp dir")
}

/// Read events.ndjson from the isolated home dir, return parsed JSON lines.
fn read_events(dir: &TempDir) -> Vec<serde_json::Value> {
    let path = dir.path().join(".8v").join("events.ndjson");
    if !path.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(&path).expect("read events.ndjson");
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("parse event line"))
        .collect()
}

/// Run `8v hook pre` with the given JSON payload on stdin.
fn run_pre(dir: &TempDir, payload: &str) -> std::process::Output {
    let mut child = bin()
        .args(["hook", "pre"])
        .env("_8V_HOME", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hook pre");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for 8v hook pre")
}

/// Run `8v hook post` with the given JSON payload on stdin.
fn run_post(dir: &TempDir, payload: &str) -> std::process::Output {
    let mut child = bin()
        .args(["hook", "post"])
        .env("_8V_HOME", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hook post");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for 8v hook post")
}

fn pre_payload(session_id: &str, tool_use_id: &str, tool_name: &str, command: &str) -> String {
    format!(
        r#"{{"hook_event_name":"PreToolUse","session_id":"{session_id}","tool_use_id":"{tool_use_id}","tool_name":"{tool_name}","tool_input":{{"command":"{command}"}}}}"#
    )
}

fn post_payload(session_id: &str, tool_use_id: &str, tool_name: &str, command: &str) -> String {
    format!(
        r#"{{"hook_event_name":"PostToolUse","session_id":"{session_id}","tool_use_id":"{tool_use_id}","tool_name":"{tool_name}","tool_input":{{"command":"{command}"}},"tool_response":{{"output":"ok"}}}}"#
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn hook_pre_emits_command_started() {
    let dir = home();
    let payload = pre_payload("claude_session_e2e", "toolu_e2e_001", "Bash", "ls -la");
    let out = run_pre(&dir, &payload);

    assert!(
        out.status.success(),
        "8v hook pre must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let events = read_events(&dir);
    assert_eq!(events.len(), 1, "exactly one event must be emitted");
    assert_eq!(
        events[0]["event"].as_str().unwrap(),
        "CommandStarted",
        "event kind must be CommandStarted"
    );
    assert_eq!(
        events[0]["caller"].as_str().unwrap(),
        "hook",
        "caller must be hook"
    );
    assert_eq!(
        events[0]["command"].as_str().unwrap(),
        "bash",
        "command must be lowercased tool name"
    );
    assert!(
        events[0]["session_id"]
            .as_str()
            .unwrap()
            .starts_with("ses_"),
        "session_id must start with ses_"
    );
}

#[test]
fn hook_post_pairs_with_pre() {
    let dir = home();
    let session = "claude_session_pair_e2e";
    let tool_use_id = "toolu_pair_e2e_001";

    run_pre(
        &dir,
        &pre_payload(session, tool_use_id, "Bash", "echo hello"),
    );
    run_post(
        &dir,
        &post_payload(session, tool_use_id, "Bash", "echo hello"),
    );

    let events = read_events(&dir);
    assert_eq!(
        events.len(),
        2,
        "must emit CommandStarted + CommandCompleted"
    );
    assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
    assert_eq!(events[1]["event"].as_str().unwrap(), "CommandCompleted");

    let started_run_id = events[0]["run_id"].as_str().unwrap();
    let completed_run_id = events[1]["run_id"].as_str().unwrap();
    assert_eq!(
        started_run_id, completed_run_id,
        "run_id must match between paired events"
    );

    let started_session = events[0]["session_id"].as_str().unwrap();
    let completed_session = events[1]["session_id"].as_str().unwrap();
    assert_eq!(
        started_session, completed_session,
        "session_id must match between paired events"
    );

    assert!(
        events[1]["success"].as_bool().unwrap(),
        "success must be true"
    );
    assert!(
        events[1]["duration_ms"].as_u64().unwrap() < 60_000,
        "duration_ms must be a plausible value"
    );
}

#[test]
fn hook_post_without_pre_synthesizes() {
    let dir = home();
    let payload = post_payload(
        "claude_session_orphan_e2e",
        "toolu_orphan_e2e_001",
        "Read",
        "some_file.rs",
    );
    let out = run_post(&dir, &payload);

    assert!(
        out.status.success(),
        "8v hook post must exit 0 even without prior pre; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let events = read_events(&dir);
    assert_eq!(
        events.len(),
        2,
        "orphaned post must synthesize CommandStarted + CommandCompleted"
    );
    assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
    assert_eq!(events[1]["event"].as_str().unwrap(), "CommandCompleted");

    let started_run_id = events[0]["run_id"].as_str().unwrap();
    let completed_run_id = events[1]["run_id"].as_str().unwrap();
    assert_eq!(
        started_run_id, completed_run_id,
        "synthesized events must share run_id"
    );

    assert_eq!(
        events[1]["duration_ms"].as_u64().unwrap(),
        0,
        "duration_ms must be 0 for orphaned events"
    );
    assert!(
        events[1]["success"].as_bool().unwrap(),
        "success must be true"
    );
}

#[test]
fn hook_bash_redacts_api_key() {
    let dir = home();
    // API key: sk- followed by 22 alphanumeric chars — matches sk-[A-Za-z0-9]{20,}
    let command_with_key =
        "curl -H Authorization: sk-AAAAAAAAAAAAAAAAAAAAAA https://api.example.com";
    let session = "claude_session_redact_e2e";
    let tool_use_id = "toolu_redact_e2e_001";

    run_pre(
        &dir,
        &pre_payload(session, tool_use_id, "Bash", command_with_key),
    );

    let events = read_events(&dir);
    assert_eq!(events.len(), 1);

    let argv = &events[0]["argv"];
    let argv_str = argv.to_string();
    assert!(
        !argv_str.contains("sk-AAAAAA"),
        "API key must be redacted from argv; got: {argv_str}"
    );
    assert!(
        argv_str.contains("<secret>"),
        "redacted placeholder must appear in argv; got: {argv_str}"
    );
}

#[test]
fn hook_pre_malformed_json_exits_one_fail_closed() {
    // H-1 fail-closed contract: `hook pre` must BLOCK the tool call when stdin
    // is malformed JSON by exiting 1. Observability is sacrificed at the Pre
    // boundary because Pre is the only place Claude honors exit 1 as a block.
    let dir = home();

    let out_pre = {
        let mut child = bin()
            .args(["hook", "pre"])
            .env("_8V_HOME", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn 8v hook pre");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"this is not json")
            .expect("write stdin");
        child.wait_with_output().expect("wait")
    };
    assert!(
        !out_pre.status.success(),
        "hook pre with malformed JSON must exit 1 (fail-closed); stderr: {}",
        String::from_utf8_lossy(&out_pre.stderr)
    );

    let events = read_events(&dir);
    assert!(
        events.is_empty(),
        "no events must be written on pre parse failure; got: {events:?}"
    );
}

#[test]
fn hook_post_malformed_json_exits_zero() {
    // Post hooks preserve the observability principle: malformed input on the
    // non-gating path is a parse failure we record-and-continue rather than
    // block on.
    let dir = home();

    let out_post = {
        let mut child = bin()
            .args(["hook", "post"])
            .env("_8V_HOME", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn 8v hook post");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{invalid}")
            .expect("write stdin");
        child.wait_with_output().expect("wait")
    };
    assert!(
        out_post.status.success(),
        "hook post with malformed JSON must exit 0; stderr: {}",
        String::from_utf8_lossy(&out_post.stderr)
    );

    let events = read_events(&dir);
    assert!(
        events.is_empty(),
        "no events must be written on post parse failure; got: {events:?}"
    );
}

#[test]
fn hook_is_hidden_from_top_level_help() {
    let out = bin().arg("--help").output().expect("run 8v --help");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // `hooks` (the git-hook subcommand) may appear; only the hidden `hook` subcommand
    // must be absent. Check that no line starts with whitespace + "hook " (without an 's').
    let has_hook_command = stdout
        .lines()
        .any(|l| l.trim_start().starts_with("hook ") || l.trim_start() == "hook");
    assert!(
        !has_hook_command,
        "`hook` must not appear as a top-level command in help output; got:\n{stdout}"
    );
}
