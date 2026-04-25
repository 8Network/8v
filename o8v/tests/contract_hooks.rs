// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary-boundary contract tests for `8v hooks claude pre-tool-use`.
//!
//! Full adversarial input matrix — every test spawns the binary, pipes stdin,
//! and asserts the exact exit code and stderr content.
//!
//! Exit codes:
//!   0 = allow
//!   2 = block (fail-closed)

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Spawn `8v hooks claude pre-tool-use`, send `payload` on stdin (None = close
/// immediately), wait up to `timeout_ms` milliseconds, and return the output.
/// Returns `None` if the process does not complete within the timeout.
fn run_pre_tool_use(payload: Option<&[u8]>, timeout_ms: u64) -> Option<std::process::Output> {
    let mut child = bin()
        .args(["hooks", "claude", "pre-tool-use"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hooks claude pre-tool-use");

    if let Some(bytes) = payload {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(bytes)
            .expect("write stdin");
    } else {
        drop(child.stdin.take());
    }

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out = child.wait_with_output();
        let _ = tx.send(out);
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(result) => Some(result.expect("wait_with_output")),
        Err(_) => None, // timed out — caller must mark test #[ignore]
    }
}

// ─── 1. Allow: unknown tool name ─────────────────────────────────────────────

#[test]
fn pre_tool_use_allows_unknown_tool_name() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"FooBar"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "FooBar is not blocked → exit 0; got {:?}",
        code
    );
}

// ─── 2. Allow: 8v MCP tool name ──────────────────────────────────────────────

#[test]
fn pre_tool_use_allows_8v_mcp_tool() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"mcp__8v__8v"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "mcp__8v__8v must be allowed → exit 0; got {:?}",
        code
    );
}

// ─── 3. Allow: WebSearch ─────────────────────────────────────────────────────

#[test]
fn pre_tool_use_allows_websearch() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"WebSearch"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "WebSearch must be allowed → exit 0; got {:?}",
        code
    );
}

// ─── 4. Allow: extra JSON fields are ignored ─────────────────────────────────

#[test]
fn pre_tool_use_allows_extra_fields() {
    let out = run_pre_tool_use(
        Some(br#"{"tool_name":"TodoWrite","extra":"field","num":42}"#),
        5000,
    )
    .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "extra fields must not affect allow decision; got {:?}",
        code
    );
}

// ─── 5. Allow: tool_name with whitespace (not blocked, not empty) ─────────────

#[test]
fn pre_tool_use_allows_whitespace_tool_name() {
    // " Read" (leading space) is NOT "Read" — must be allowed.
    let out = run_pre_tool_use(Some(br#"{"tool_name":" Read"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "' Read' with leading space is not in blocked list → exit 0; got {:?}",
        code
    );
}

// ─── 6. Allow: case-sensitive (lowercase read is not blocked) ─────────────────

#[test]
fn pre_tool_use_allows_lowercase_read() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"read"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(0),
        "blocking is case-sensitive: 'read' != 'Read' → exit 0; got {:?}",
        code
    );
}

// ─── 7. Block: Read ──────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_read() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Read"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Read must be blocked → exit 2; got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("8v read"),
        "stderr must mention '8v read'; got: {stderr}"
    );
}

// ─── 8. Block: Edit ──────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_edit() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Edit"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Edit must be blocked → exit 2; got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("8v write"),
        "stderr must mention '8v write'; got: {stderr}"
    );
}

// ─── 9. Block: Write ─────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_write() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Write"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Write must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 10. Block: Bash ─────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_bash() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Bash"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Bash must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 11. Block: Glob ─────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_glob() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Glob"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Glob must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 12. Block: Grep ─────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_grep() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Grep"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Grep must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 13. Block: Agent ────────────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_agent() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Agent"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "Agent must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 14. Block: NotebookEdit ─────────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_notebook_edit() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":"NotebookEdit"}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "NotebookEdit must be blocked → exit 2; got {:?}",
        code
    );
}

// ─── 15. Fail-closed: malformed JSON ─────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_on_malformed_json() {
    let out = run_pre_tool_use(Some(b"not json at all"), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "malformed JSON must block (exit 2); got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("blocking by default"),
        "stderr must mention 'blocking by default'; got: {stderr}"
    );
}

// ─── 16. Fail-closed: empty stdin ────────────────────────────────────────────

#[test]
fn pre_tool_use_blocks_on_empty_stdin() {
    let out = run_pre_tool_use(None, 5000)
        .expect("process must complete within timeout — empty stdin must not hang");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "empty stdin must block (exit 2); got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("blocking by default"),
        "stderr must mention 'blocking by default'; got: {stderr}"
    );
}

// ─── 17. Fail-closed: null tool_name field ───────────────────────────────────

#[test]
fn pre_tool_use_blocks_on_null_tool_name() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":null}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "null tool_name must block (exit 2); got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("blocking by default"),
        "stderr must mention 'blocking by default'; got: {stderr}"
    );
}

// ─── 18. Fail-closed: missing tool_name key ──────────────────────────────────

#[test]
fn pre_tool_use_blocks_on_missing_tool_name_key() {
    let out = run_pre_tool_use(Some(b"{}"), 5000).expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "missing tool_name key must block (exit 2); got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("blocking by default"),
        "stderr must mention 'blocking by default'; got: {stderr}"
    );
}

// ─── 19. Fail-closed: empty string tool_name ─────────────────────────────────

#[test]
fn pre_tool_use_blocks_on_empty_string_tool_name() {
    let out = run_pre_tool_use(Some(br#"{"tool_name":""}"#), 5000)
        .expect("process must complete within timeout");
    let code = out.status.code();
    assert_eq!(
        code,
        Some(2),
        "empty string tool_name must block (exit 2); got {:?}",
        code
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("blocking by default"),
        "stderr must mention 'blocking by default'; got: {stderr}"
    );
}

// ─── 20. Block message format: actionable hint present ───────────────────────

#[test]
fn pre_tool_use_block_message_format() {
    // Any blocked tool triggers the hint; use Read.
    let out = run_pre_tool_use(Some(br#"{"tool_name":"Read"}"#), 5000)
        .expect("process must complete within timeout");
    assert_eq!(out.status.code(), Some(2), "Read must exit 2");
    // Exact hint from claude.rs: "Blocked: use 8v read, 8v write, 8v check, 8v fmt, 8v test instead of native tools."
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("8v read"),
        "hint must mention '8v read'; got: {stderr}"
    );
    assert!(
        stderr.contains("8v write"),
        "hint must mention '8v write'; got: {stderr}"
    );
    assert!(
        stderr.contains("8v check"),
        "hint must mention '8v check'; got: {stderr}"
    );
    assert!(
        stderr.starts_with("Blocked:"),
        "hint must start with 'Blocked:'; got: {stderr}"
    );
}
