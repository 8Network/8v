// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! M-2 regression: tracing must not leak to stdout in MCP mode.
//!
//! Spawns `8v mcp` with `RUST_LOG=debug`, sends `initialize` + `tools/list`
//! over stdin, and asserts every line on stdout parses as valid JSON.
//! Any tracing line (e.g. `DEBUG o8v::mcp ...`) would fail JSON parsing.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;

#[test]
fn tracing_does_not_pollute_mcp_stdout() {
    let _tmp = TempDir::new().expect("tempdir");

    let mut child = Command::new(env!("CARGO_BIN_EXE_8v"))
        .arg("mcp")
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn 8v mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Send initialize request.
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.0.0" }
        }
    });
    let msg = serde_json::to_string(&initialize).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Read initialize response — must be valid JSON.
    let mut line = String::new();
    reader.read_line(&mut line).expect("read line");
    let trimmed = line.trim();
    assert!(
        serde_json::from_str::<Value>(trimmed).is_ok(),
        "initialize response is not valid JSON — tracing leaked to stdout.\nGot: {trimmed:?}"
    );

    // Send initialized notification.
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let msg = serde_json::to_string(&initialized).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Send tools/list request.
    let tools_list = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let msg = serde_json::to_string(&tools_list).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    // Read tools/list response — must be valid JSON.
    let mut line2 = String::new();
    reader.read_line(&mut line2).expect("read line");
    let trimmed2 = line2.trim();
    assert!(
        serde_json::from_str::<Value>(trimmed2).is_ok(),
        "tools/list response is not valid JSON — tracing leaked to stdout.\nGot: {trimmed2:?}"
    );

    drop(stdin);
    let _ = child.wait();
}
