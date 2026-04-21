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

// ─── M-5: hooks isError fix ──────────────────────────────────────────────────
//
// Bug: o8v/src/commands/mod.rs hardcodes `use_stderr=true` for the hooks
// command. The MCP handler maps `use_stderr=true` → `Err(out)` → isError=true,
// so every successful hooks invocation appeared as a failure to MCP callers.
//
// Fix: hooks must use `audience == Audience::Human` like all other commands,
// so that MCP callers (Audience::Agent) get isError=false on exit 0.
//
// PRE-FIX: isError=true even when hooks exits 0.
// POST-FIX: isError=false when hooks exits 0.

/// M-5: `hooks` invoked on a workspace with no hooks configured must return
/// isError=false. A missing hooks section is not an error — it just means
/// nothing ran.
///
/// PRE-FIX: fails because use_stderr is hardcoded to true for hooks.
/// POST-FIX: passes because use_stderr follows audience (Agent → false).
#[test]
fn hooks_exit_0_is_not_mcp_error() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    // `hooks claude post-tool-use` is a noop that always exits 0.
    // This must not appear as an MCP error.
    let resp = client.tools_call("hooks claude post-tool-use");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        !is_error,
        "M-5: hooks exit-0 must have isError=false, got isError=true\ncontent: {text}\nfull: {resp}"
    );
}

#[test]
fn mcp_upgrade_help_returns_ok() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("upgrade --help");
    let (is_error, _text) = parse_call_result(&resp);
    assert!(!is_error, "upgrade --help should succeed, got: {resp}");
}

// ─── init via MCP ──────────────────────────────────────────────────────────
//
// These tests FAIL on pre-fix code (init returns "not a dispatchable command")
// and PASS after the fix (init --yes succeeds via MCP).

/// `init --yes` via MCP must succeed (isError = false).
///
/// Pre-rename: failed with `error: not a dispatchable command`.
/// Post-rename: succeeds; init runs non-interactively via --yes.
#[test]
fn init_yes_via_mcp_succeeds() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("init --yes");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        !is_error,
        "init --yes via MCP should succeed
content: {text}
full: {resp}"
    );
}

/// `init --yes` result must not contain "not a dispatchable command".
#[test]
fn init_yes_via_mcp_does_not_return_not_dispatchable_error() {
    let ws = make_workspace();
    let mut client = McpClient::spawn(&file_uri(&ws));
    let resp = client.tools_call("init --yes");
    let (_is_error, text) = parse_call_result(&resp);
    assert!(
        !text.contains("not a dispatchable command"),
        "'not a dispatchable command' must not appear in MCP init response
content: {text}"
    );
}

// ─── Output cap (MCP-OC) ─────────────────────────────────────────────────────
//
// These tests FAIL on pre-cap code (no output gate exists) and PASS after the
// cap logic is added to handler.rs.
//
// All cap tests use O8V_MCP_OUTPUT_CAP=1000 so fixture sizes stay small and CI
// stays fast.  A fixture of ~1200 bytes triggers pre-flight (1200 × 1.20 = 1440
// > 1000).  A small file (~100 bytes) stays under cap for the pass-through test.

/// Spawn an McpClient with a custom O8V_MCP_OUTPUT_CAP env var.
///
/// `root_dir` is canonicalized so macOS /tmp → /private/tmp is resolved and
/// `.current_dir()` is set on the child process so the CWD fallback in
/// `handler.rs` (when `get_root_directory` returns None) resolves to the
/// workspace instead of the cargo test runner's CWD.
fn spawn_with_cap_env(root_dir: &TempDir, cap: &str) -> McpClient {
    let canonical = std::fs::canonicalize(root_dir.path()).expect("canonicalize root_dir");
    let root_uri = format!("file://{}", canonical.display());

    let mut child = Command::new(env!("CARGO_BIN_EXE_8v"))
        .arg("mcp")
        .env("O8V_MCP_OUTPUT_CAP", cap)
        .current_dir(&canonical)
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
        root_uri,
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
    client.recv_response();
    client.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    }));
    client
}

/// MCP-OC Test 1 — Pre-flight fires.
///
/// PRE-CAP: no output gate → `read --full` succeeds (Ok, no error).
/// POST-CAP: pre-flight aborts → `Err`, `isError: true`, §6 template in message,
///           per-file byte count listed (proof pre-flight ran, not post-render).
///
/// Fixture: ~1200 bytes. Cap: 1000. Pre-flight sum: 1200 × 1.20 = 1440 > 1000.
#[test]
fn mcp_oc_preflight_fires() {
    let ws = make_workspace();
    // Write a fixture file of ~1200 bytes (120 lines × ~10 chars each).
    let fixture_path = ws.path().join("big.txt");
    let line = "0123456789\n"; // 11 bytes
    let content: String = line.repeat(110); // 1210 bytes
    std::fs::write(&fixture_path, &content).expect("write fixture");

    let mut client = spawn_with_cap_env(&ws, "1000");
    let resp = client.tools_call("read ./big.txt --full");
    let (is_error, text) = parse_call_result(&resp);

    assert!(
        is_error,
        "MCP-OC pre-flight must set isError=true\nfull response: {resp}"
    );
    assert!(
        text.contains("output too large for MCP transport"),
        "MCP-OC pre-flight error must contain §6 header\ncontent: {text}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC pre-flight error must mention override env var\ncontent: {text}"
    );
    // Pre-flight proof: per-file byte count appears in the error message.
    // Post-render cannot know file sizes; only pre-flight reads metadata.
    assert!(
        text.contains("bytes"),
        "MCP-OC pre-flight error must list per-file byte sizes\ncontent: {text}"
    );
}

/// MCP-OC Test 2 — Post-render fires.
///
/// PRE-CAP: no output gate → `ls --tree` on a large dir succeeds.
/// POST-CAP: post-render catches oversized rendered output → `Err`, `isError: true`,
///           §6 template in message, no per-file byte sizes (post-render proof).
///
/// Fixture: directory with many small files so `ls --tree` output > 1000 chars.
/// Cap: 1000.
#[test]
fn mcp_oc_post_render_fires() {
    let ws = make_workspace();
    // Create enough files that `ls --tree` output exceeds 1000 chars.
    // Each file name is ~20 chars; 60 files × ~20 chars each ≈ 1200+ chars rendered.
    for i in 0..60 {
        let name = format!("file_{i:03}_placeholder.txt");
        std::fs::write(ws.path().join(&name), "x").expect("write file");
    }

    let mut client = spawn_with_cap_env(&ws, "1000");
    let resp = client.tools_call("ls --tree");
    let (is_error, text) = parse_call_result(&resp);

    assert!(
        is_error,
        "MCP-OC post-render must set isError=true\nfull response: {resp}"
    );
    assert!(
        text.contains("output too large for MCP transport"),
        "MCP-OC post-render error must contain §6 header\ncontent: {text}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC post-render error must mention override env var\ncontent: {text}"
    );
}

/// MCP-OC Test 3 — Cap override + under-cap passes.
///
/// PRE-CAP: passes (no gate).
/// POST-CAP: passes (cap is generous; file is tiny).
///
/// Large cap (100000), small file (~100 bytes). Must return Ok with content.
#[test]
fn mcp_oc_under_cap_passes() {
    let ws = make_workspace();
    let fixture_path = ws.path().join("small.txt");
    std::fs::write(&fixture_path, "hello world\n").expect("write fixture");

    let mut client = spawn_with_cap_env(&ws, "100000");
    let resp = client.tools_call("read ./small.txt --full");
    let (is_error, text) = parse_call_result(&resp);

    assert!(
        !is_error,
        "MCP-OC under-cap must succeed (isError=false)\ncontent: {text}\nfull: {resp}"
    );
    assert!(
        text.contains("hello world"),
        "MCP-OC under-cap must return file content\ncontent: {text}"
    );
}

/// MCP-OC Test 4 — Invalid cap override values produce an error before any command executes.
///
/// PRE-CAP: O8V_MCP_OUTPUT_CAP is ignored (no cap parsing) → command runs normally.
/// POST-CAP: each invalid value produces a distinct observable error on first
///           handle_command call, before any dispatch.
///
/// Parameterized over: "0", "-1", "abc", "".
#[test]
fn mcp_oc_invalid_cap_zero() {
    let ws = make_workspace();
    let mut client = spawn_with_cap_env(&ws, "0");
    let resp = client.tools_call("ls");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        is_error,
        "MCP-OC invalid cap '0' must produce isError=true\nfull: {resp}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC invalid cap error must mention O8V_MCP_OUTPUT_CAP\ncontent: {text}"
    );
}

#[test]
fn mcp_oc_invalid_cap_negative() {
    let ws = make_workspace();
    let mut client = spawn_with_cap_env(&ws, "-1");
    let resp = client.tools_call("ls");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        is_error,
        "MCP-OC invalid cap '-1' must produce isError=true\nfull: {resp}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC invalid cap error must mention O8V_MCP_OUTPUT_CAP\ncontent: {text}"
    );
}

#[test]
fn mcp_oc_invalid_cap_non_numeric() {
    let ws = make_workspace();
    let mut client = spawn_with_cap_env(&ws, "abc");
    let resp = client.tools_call("ls");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        is_error,
        "MCP-OC invalid cap 'abc' must produce isError=true\nfull: {resp}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC invalid cap error must mention O8V_MCP_OUTPUT_CAP\ncontent: {text}"
    );
}

#[test]
fn mcp_oc_invalid_cap_empty() {
    let ws = make_workspace();
    let mut client = spawn_with_cap_env(&ws, "");
    let resp = client.tools_call("ls");
    let (is_error, text) = parse_call_result(&resp);
    assert!(
        is_error,
        "MCP-OC invalid cap '' must produce isError=true\nfull: {resp}"
    );
    assert!(
        text.contains("O8V_MCP_OUTPUT_CAP"),
        "MCP-OC invalid cap error must mention O8V_MCP_OUTPUT_CAP\ncontent: {text}"
    );
}

/// MCP-OC Gap-1 test — Invalid cap errors come from cap validation, not post-render.
///
/// Prior mutation testing revealed that the four invalid-cap tests only check for
/// "O8V_MCP_OUTPUT_CAP", which appears in BOTH `get_output_cap()` error messages
/// AND in `oversized_error()`. When a mutant silently converts an invalid cap to 0
/// or 5, any non-empty command output triggers `oversized_error()`, which also
/// contains "O8V_MCP_OUTPUT_CAP". The tests then pass via the wrong code path.
///
/// Fix: each invalid cap value should produce a message containing the specific
/// rejection reason text that ONLY appears in `get_output_cap()`:
///   - zero/negative  → "is not a positive integer"
///   - non-numeric    → "is not a valid integer"
///   - empty string   → "is set but empty"
///
/// These strings do NOT appear in `oversized_error()` or in any other code path.
#[test]
fn mcp_oc_invalid_cap_error_comes_from_validation() {
    // (cap="0") must produce the positive-integer rejection, not a post-render error.
    {
        let ws = make_workspace();
        let mut client = spawn_with_cap_env(&ws, "0");
        let resp = client.tools_call("ls");
        let (is_error, text) = parse_call_result(&resp);
        assert!(is_error, "cap='0' must produce isError=true\nfull: {resp}");
        assert!(
            text.contains("is not a positive integer"),
            "cap='0' error must come from cap validation (missing rejection reason)\ncontent: {text}"
        );
    }

    // (cap="-1") same rejection message.
    {
        let ws = make_workspace();
        let mut client = spawn_with_cap_env(&ws, "-1");
        let resp = client.tools_call("ls");
        let (is_error, text) = parse_call_result(&resp);
        assert!(is_error, "cap='-1' must produce isError=true\nfull: {resp}");
        assert!(
            text.contains("is not a positive integer"),
            "cap='-1' error must come from cap validation\ncontent: {text}"
        );
    }

    // (cap="abc") must produce the non-integer rejection.
    {
        let ws = make_workspace();
        let mut client = spawn_with_cap_env(&ws, "abc");
        let resp = client.tools_call("ls");
        let (is_error, text) = parse_call_result(&resp);
        assert!(
            is_error,
            "cap='abc' must produce isError=true\nfull: {resp}"
        );
        assert!(
            text.contains("is not a valid integer"),
            "cap='abc' error must come from cap validation\ncontent: {text}"
        );
    }

    // (cap="") must produce the empty-string rejection.
    {
        let ws = make_workspace();
        let mut client = spawn_with_cap_env(&ws, "");
        let resp = client.tools_call("ls");
        let (is_error, text) = parse_call_result(&resp);
        assert!(is_error, "cap='' must produce isError=true\nfull: {resp}");
        assert!(
            text.contains("is set but empty"),
            "cap='' error must come from cap validation\ncontent: {text}"
        );
    }
}

/// MCP-OC Gap-2 test — Post-render path is distinct from pre-flight.
///
/// A mutation removing the post-render check was caught by `mcp_oc_post_render_fires`, but
/// that test has no discriminator proving the post-render path fired rather than
/// some other error path. This test strengthens the assertion by verifying:
///   - The error does NOT contain "bytes" (which is unique to the pre-flight
///     per-file size listing).
///   - The error does NOT contain "estimated" (which is unique to pre-flight's
///     estimated-chars line).
///   - The error DOES contain "chars" (which appears in `oversized_error()`'s
///     output-chars and cap lines, and not in `get_output_cap()` errors).
///
/// Together these three checks pin the error to `oversized_error()`.
#[test]
fn mcp_oc_post_render_error_is_from_post_render_path() {
    let ws = make_workspace();
    // Same fixture as mcp_oc_post_render_fires: 60 files so ls --tree > 1000 chars.
    for i in 0..60 {
        let name = format!("file_{i:03}_placeholder.txt");
        std::fs::write(ws.path().join(&name), "x").expect("write file");
    }

    let mut client = spawn_with_cap_env(&ws, "1000");
    let resp = client.tools_call("ls --tree");
    let (is_error, text) = parse_call_result(&resp);

    assert!(
        is_error,
        "MCP-OC post-render (gap-2) must set isError=true\nfull response: {resp}"
    );
    // Positive proof: "chars" appears in oversized_error() output lines.
    assert!(
        text.contains("chars"),
        "MCP-OC post-render error must contain 'chars' (oversized_error marker)\ncontent: {text}"
    );
    // Negative proofs: pre-flight-only words must NOT appear.
    assert!(
        !text.contains("bytes"),
        "MCP-OC post-render error must NOT contain 'bytes' (pre-flight marker)\ncontent: {text}"
    );
    assert!(
        !text.contains("estimated"),
        "MCP-OC post-render error must NOT contain 'estimated' (pre-flight marker)\ncontent: {text}"
    );
}
