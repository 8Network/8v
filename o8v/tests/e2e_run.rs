// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v run` — runs the compiled binary against real commands.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── Basic execution ────────────────────────────────────────────────────────

#[test]
fn run_echo_succeeds() {
    let out = bin()
        .args(["run", "echo hello"])
        .output()
        .expect("run 8v run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("hello"),
        "should contain echo output: {stdout}"
    );
    assert!(
        stdout.contains("exit: 0 (success)"),
        "should show success exit: {stdout}"
    );
    assert!(
        stdout.contains("duration:"),
        "should show duration: {stdout}"
    );
}

#[test]
fn run_failing_command_exits_nonzero() {
    let out = bin()
        .args(["run", "ls /nonexistent-path-xyz"])
        .output()
        .expect("run 8v run");

    assert!(
        !out.status.success(),
        "should exit non-zero for failing command"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("failed") || stdout.contains("spawn error"),
        "should indicate failure: {stdout}"
    );
}

#[test]
fn run_nonexistent_program_reports_spawn_error() {
    let out = bin()
        .args(["run", "nonexistent-program-xyz-123"])
        .output()
        .expect("run 8v run");

    assert!(!out.status.success(), "should exit non-zero");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("spawn error"),
        "should report spawn error: {combined}"
    );
}

// ─── Shell injection safety ─────────────────────────────────────────────────

#[test]
fn run_semicolon_not_interpreted_as_shell() {
    // "echo hello; echo injected" should NOT execute as two commands.
    // shlex splits it into ["echo", "hello;", "echo", "injected"] — all args to echo.
    let out = bin()
        .args(["run", "echo hello; echo injected"])
        .output()
        .expect("run 8v run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // echo receives all args as one: "hello; echo injected"
    assert!(
        stdout.contains("hello;"),
        "semicolon should be literal: {stdout}"
    );
}

#[test]
fn run_pipe_not_interpreted_as_shell() {
    let out = bin()
        .args(["run", "echo hello | cat"])
        .output()
        .expect("run 8v run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // echo receives "hello", "|", "cat" as arguments
    assert!(stdout.contains("|"), "pipe should be literal: {stdout}");
}

// ─── Input validation ───────────────────────────────────────────────────────

#[test]
fn run_empty_command_returns_error() {
    let out = bin().args(["run", ""]).output().expect("run 8v run");

    assert!(!out.status.success(), "empty command should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("empty command"),
        "should say empty: {stderr}"
    );
}

#[test]
fn run_unbalanced_quotes_returns_error() {
    let out = bin()
        .args(["run", "'unmatched"])
        .output()
        .expect("run 8v run");

    assert!(!out.status.success(), "unbalanced quotes should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unbalanced"),
        "should mention unbalanced: {stderr}"
    );
}

#[test]
fn run_timeout_exceeded_returns_error() {
    let out = bin()
        .args(["run", "sleep 30", "--timeout", "601"])
        .output()
        .expect("run 8v run");

    assert!(!out.status.success(), "should fail with timeout too large");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exceeds maximum"),
        "should mention max: {stderr}"
    );
}

// ─── JSON output ────────────────────────────────────────────────────────────

#[test]
fn run_json_output_has_required_fields() {
    let out = bin()
        .args(["run", "echo json-test", "--json"])
        .output()
        .expect("run 8v run --json");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(parsed.get("command").is_some(), "missing command field");
    assert!(parsed.get("exit_code").is_some(), "missing exit_code field");
    assert!(parsed.get("stdout").is_some(), "missing stdout field");
    assert!(parsed.get("stderr").is_some(), "missing stderr field");
    assert!(
        parsed.get("duration_ms").is_some(),
        "missing duration_ms field"
    );
    assert!(parsed.get("truncated").is_some(), "missing truncated field");

    let truncated = parsed.get("truncated").unwrap();
    assert!(
        truncated.get("stdout").is_some(),
        "truncated missing stdout"
    );
    assert!(
        truncated.get("stderr").is_some(),
        "truncated missing stderr"
    );
}

// ─── Timeout enforcement ────────────────────────────────────────────────────

#[test]
fn run_timeout_kills_process() {
    let out = bin()
        .args(["run", "sleep 60", "--timeout", "1"])
        .output()
        .expect("run 8v run with timeout");

    assert!(!out.status.success(), "should exit non-zero on timeout");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("timeout"),
        "should mention timeout: {stdout}"
    );
}

// ─── Multiline output ───────────────────────────────────────────────────────

#[test]
fn run_captures_multiline_stdout() {
    let out = bin()
        .args(["run", "printf line1\\nline2\\nline3"])
        .output()
        .expect("run 8v run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("line1"), "should capture line1: {stdout}");
    assert!(stdout.contains("line3"), "should capture line3: {stdout}");
}
