// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v hooks --json` output.
//!
//! These tests verify that all hooks subcommands accept `--json` and emit
//! valid JSON with the expected fields.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// `8v hooks claude pre-tool-use --json` with no stdin emits valid JSON.
///
/// pre-tool-use reads stdin JSON for tool_name; empty stdin → allow (exit 0).
#[test]
fn hooks_claude_pre_tool_use_json_allow() {
    use std::process::Stdio;

    let mut child = bin()
        .args(["hooks", "--json", "claude", "pre-tool-use"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hooks claude pre-tool-use --json");

    // Write empty JSON object so stdin parse succeeds.
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{}")
        .expect("write stdin");

    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        out.status.success(),
        "exit 0 for allowed tool\nstderr: {}\nstdout: {stdout}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert!(
        parsed["exit_code"].is_number(),
        "expected exit_code field, got: {parsed}"
    );
    assert!(
        parsed["success"].is_boolean(),
        "expected success field, got: {parsed}"
    );
    assert_eq!(parsed["exit_code"], 0, "allowed → exit_code 0");
    assert_eq!(parsed["success"], true, "allowed → success true");
}

/// `8v hooks claude pre-tool-use --json` with a blocked tool emits JSON with exit_code 2.
#[test]
fn hooks_claude_pre_tool_use_json_blocked() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = bin()
        .args(["hooks", "--json", "claude", "pre-tool-use"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"tool_name":"Read"}"#)
        .expect("write stdin");

    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Exit code 2 = tool blocked — not a process error, check stdout
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert_eq!(parsed["exit_code"], 2, "Read is blocked → exit_code 2");
    assert_eq!(parsed["success"], false, "blocked → success false");
}

/// `8v hooks claude post-tool-use --json` exits 0 and emits JSON.
#[test]
fn hooks_claude_post_tool_use_json() {
    let out = bin()
        .args(["hooks", "--json", "claude", "post-tool-use"])
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        out.status.success(),
        "exit 0\nstderr: {}\nstdout: {stdout}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["success"], true);
}

/// `8v hooks claude session-start --json` exits 0 and emits JSON.
#[test]
fn hooks_claude_session_start_json() {
    let out = bin()
        .args(["hooks", "--json", "claude", "session-start"])
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        out.status.success(),
        "exit 0\nstderr: {}\nstdout: {stdout}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["success"], true);
}

/// `8v hooks git on-commit-msg --json` with a temp file emits JSON.
#[test]
fn hooks_git_on_commit_msg_json() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let msg_file = dir.path().join("COMMIT_EDITMSG");
    fs::write(&msg_file, "feat: something\n").unwrap();

    let out = bin()
        .args([
            "hooks",
            "--json",
            "git",
            "on-commit-msg",
            msg_file.to_str().unwrap(),
        ])
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        out.status.success(),
        "exit 0\nstderr: {}\nstdout: {stdout}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["success"], true);
}
