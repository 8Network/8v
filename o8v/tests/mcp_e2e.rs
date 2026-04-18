// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Black-box MCP E2E tests — spawn `8v mcp` as a subprocess and speak
//! JSON-RPC 2.0 over stdio. No mocks, no patches; uses the real binary.
//!
//! Protocol notes:
//! - Client sends `initialize`, then `notifications/initialized`.
//! - During `tools/call`, the server sends a `roots/list` request back to the
//!   client to resolve the working directory. The client must respond before
//!   the server continues.
//! - The single exposed MCP tool is named `"8v"` and accepts a `command: String`.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::TempDir;

// ─── Client ──────────────────────────────────────────────────────────────────

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
    /// The URI returned to the server when it asks for roots.
    root_uri: String,
}

impl McpClient {
    fn spawn(root_uri: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_8v"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn 8v mcp");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let reader = BufReader::new(stdout);

        let mut client = McpClient {
            child,
            stdin,
            reader,
            next_id: 1,
            root_uri: root_uri.to_string(),
        };

        // Perform the MCP handshake.
        let init_id = client.alloc_id();
        client.send(json!({
            "jsonrpc": "2.0",
            "id": init_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "roots": { "listChanged": false } },
                "clientInfo": { "name": "mcp-e2e-test", "version": "0.0.1" }
            }
        }));
        // Read initialize response (handles any interleaved roots/list).
        client.recv_response();

        // Send initialized notification.
        client.send(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }));

        client
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Write a JSON-RPC message (newline-delimited).
    fn send(&mut self, msg: Value) {
        let line = serde_json::to_string(&msg).expect("serialize");
        writeln!(self.stdin, "{line}").expect("write to mcp stdin");
    }

    /// Read one line from stdout, parse as JSON.
    fn read_line(&mut self) -> Value {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .expect("read from mcp stdout");
        serde_json::from_str(line.trim()).expect("invalid JSON from server")
    }

    /// Read until we get a response (id present). Automatically handles
    /// interleaved `roots/list` requests from the server.
    fn recv_response(&mut self) -> Value {
        loop {
            let msg = self.read_line();
            if msg
                .get("method")
                .map(|m| m == "roots/list")
                .is_some_and(|v| v)
            {
                // Server is asking for the client's root directories.
                let req_id = msg["id"].clone();
                let root_uri = self.root_uri.clone();
                self.send(json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "roots": [{ "uri": root_uri, "name": "test" }]
                    }
                }));
                continue;
            }
            // Any message with an "id" and no "method" is a response.
            if msg.get("id").is_some() && msg.get("method").is_none() {
                return msg;
            }
            // Notifications (no id, has method) — skip.
        }
    }

    /// Call a tools/call request and return the full response value.
    fn tools_call(&mut self, command: &str) -> Value {
        let id = self.alloc_id();
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "8v",
                "arguments": { "command": command }
            }
        }));
        self.recv_response()
    }

    /// Call `tools/list` and return the result.
    fn tools_list(&mut self) -> Value {
        let id = self.alloc_id();
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        }));
        self.recv_response()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Returns `(is_error, content_text)` for a `tools/call` response.
fn parse_call_result(resp: &Value) -> (bool, String) {
    let result = &resp["result"];
    let is_error = result["isError"].as_bool() == Some(true);
    let text = match result["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["text"].as_str())
    {
        Some(s) => s.to_string(),
        None => String::new(),
    };
    (is_error, text)
}

fn make_workspace() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn file_uri(dir: &TempDir) -> String {
    // Canonicalize to resolve macOS /tmp → /private/tmp symlink so the URI
    // matches the containment root the server resolves via std::fs::canonicalize.
    let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize tempdir");
    format!("file://{}", canonical.display())
}

// ─── tools/list ──────────────────────────────────────────────────────────────

#[test]
fn tools_list_exposes_8v_tool() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_list();
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"8v"),
        "expected '8v' tool in tools/list, got: {names:?}"
    );
}

// ─── Smoke tests for working commands ────────────────────────────────────────

#[test]
fn mcp_ls_returns_ok() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("ls");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "ls should succeed, got: {resp}");
}

#[test]
fn mcp_search_returns_ok() {
    let ws = make_workspace();
    // Write a file so search has something to traverse.
    std::fs::write(ws.path().join("hello.txt"), "world").expect("write hello.txt");
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("search world");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "search should succeed, got: {resp}");
}

#[test]
fn mcp_read_returns_ok() {
    // Use a fixture path inside the git repo so safe_read containment passes.
    let fixture_dir = o8v_testkit::fixture_path("o8v", "build-rust");
    let uri = format!(
        "file://{}",
        std::fs::canonicalize(&fixture_dir)
            .expect("canonicalize fixture dir")
            .display()
    );
    let mut client = McpClient::spawn(&uri);
    let resp = client.tools_call("read src/main.rs --full");
    let (is_error, text) = parse_call_result(&resp);
    assert!(!is_error, "read should succeed, got: {resp}");
    assert!(!text.is_empty(), "read output should contain file content");
}

#[test]
fn mcp_write_returns_ok() {
    // Use a fixture path inside the git repo so safe_read containment passes.
    // No-op find+replace so the fixture file is not permanently modified.
    let fixture_dir = o8v_testkit::fixture_path("o8v", "build-rust");
    let uri = format!(
        "file://{}",
        std::fs::canonicalize(&fixture_dir)
            .expect("canonicalize fixture dir")
            .display()
    );
    let mut client = McpClient::spawn(&uri);
    // Line-based no-op: replace line 1 with the exact same content.
    let resp = client.tools_call("write src/main.rs:1 \"fn main() {\"");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "write should succeed, got: {resp}");
}

#[test]
fn mcp_hooks_help_returns_ok() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("hooks --help");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "hooks --help should succeed, got: {resp}");
}

#[test]
fn mcp_upgrade_help_returns_ok() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("upgrade --help");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "upgrade --help should succeed, got: {resp}");
}

// ─── F13: init via MCP ───────────────────────────────────────────────────────
//
// These tests FAIL on pre-fix code (init returns "not a dispatchable command")
// and PASS after the fix (init --yes succeeds via MCP).

/// F13: `init --yes` via MCP must succeed (isError = false).
///
/// PRE-FIX: fails with `error: not a dispatchable command`.
/// POST-FIX: succeeds; init runs non-interactively via --yes.
#[test]
fn f13_init_yes_via_mcp_succeeds() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("init --yes");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        !is_error,
        "init --yes via MCP should succeed (F13 fix required)\ncontent: {text}\nfull: {resp}"
    );
}

/// F13: `init --yes` result must not contain "not a dispatchable command".
#[test]
fn f13_init_not_dispatchable_error_is_gone() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("init --yes");
    let (_is_error, text) = parse_call_result(&resp);
    assert!(
        !text.contains("not a dispatchable command"),
        "F13: 'not a dispatchable command' must be gone after fix\ncontent: {text}"
    );
}
