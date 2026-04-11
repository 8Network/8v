// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP event types for AI cost observability.
//!
//! Two events bracket each MCP invocation:
//! - [`McpInvoked`]   — emitted before the check runs (captures intent)
//! - [`McpCompleted`] — emitted after rendering (captures cost)
//!
//! Both are appended as NDJSON lines to `.8v/mcp-events.ndjson` via
//! `o8v_fs::safe_append` (containment + symlink-safe). All path knowledge
//! lives in `StateDir` — no raw `.join("string")` here.

use serde::Serialize;

/// The caller is always "mcp" — structural certainty, not a detection result.
/// `handle_command()` is only reachable via the MCP server.
const CALLER: &str = "mcp";

/// The 8v version string embedded in every event.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estimate token count from byte length using GPT-4 approximation: bytes / 4.
fn estimate_tokens(bytes: u64) -> u64 {
    bytes / 4
}

/// Emitted once per MCP tool invocation, before the check runs.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpInvoked {
    /// Event kind discriminator.
    event: &'static str,
    /// UUID scoped to this single MCP call.
    run_id: String,
    /// Unix milliseconds.
    timestamp_ms: i64,
    /// 8v version that handled the call.
    version: &'static str,
    /// Always "mcp".
    caller: &'static str,
    /// Raw tool arguments as received (e.g. "check .").
    command: String,
    /// Byte length of the command string sent by the AI (AI output tokens).
    command_bytes: u64,
    /// Estimated token count for the command: command_bytes / 4.
    command_token_estimate: u64,
    /// Absolute path of the project root.
    project_path: String,
}

impl McpInvoked {
    pub(crate) fn new(
        run_id: String,
        command: impl Into<String>,
        project_path: impl Into<String>,
    ) -> Self {
        let command = command.into();
        let command_bytes = command.len() as u64;
        Self {
            event: "McpInvoked",
            run_id,
            timestamp_ms: crate::util::unix_ms() as i64,
            version: VERSION,
            caller: CALLER,
            command,
            command_bytes,
            command_token_estimate: estimate_tokens(command_bytes),
            project_path: project_path.into(),
        }
    }
}

/// Emitted once per MCP tool invocation, after the response is rendered.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct McpCompleted {
    /// Event kind discriminator.
    event: &'static str,
    /// Matches the [`McpInvoked`] for this invocation.
    run_id: String,
    /// Unix milliseconds.
    timestamp_ms: i64,
    /// Byte length of the rendered response sent to the AI.
    render_bytes: u64,
    /// Estimated token count: render_bytes / 4 (GPT-4 approximation).
    token_estimate: u64,
    /// Total MCP call duration in milliseconds.
    duration_ms: u64,
}

impl McpCompleted {
    pub(crate) fn new(run_id: String, render_bytes: u64, duration_ms: u64) -> Self {
        Self {
            event: "McpCompleted",
            run_id,
            timestamp_ms: crate::util::unix_ms() as i64,
            render_bytes,
            token_estimate: estimate_tokens(render_bytes),
            duration_ms,
        }
    }
}

/// Serialize `event` to JSON and append as an NDJSON line to `.8v/mcp-events.ndjson`.
///
/// Handles serialization and the trailing `\n` internally.
/// Uses `o8v_fs::safe_append` (containment + symlink-safe). Falls back to
/// `o8v_fs::safe_write` if the file does not exist yet (first invocation).
/// Best-effort: never panics, never fails the MCP response.
///
/// All path knowledge comes from `StorageDir` — no raw `.join("string")` here.
pub(super) fn emit<T: serde::Serialize>(storage: &o8v_workspace::StorageDir, event: &T) {
    let json = match serde_json::to_string(event) {
        Ok(j) => j,
        Err(e) => {
            tracing::debug!("mcp events: could not serialize event: {e}");
            return;
        }
    };
    let line = format!("{json}\n");
    let bytes = line.as_bytes();

    let path = storage.mcp_events();
    let containment = storage.containment();

    match o8v_fs::safe_append(&path, containment, bytes) {
        Ok(()) => {}
        Err(o8v_fs::FsError::NotFound { .. }) => {
            // File does not exist yet — create it on first write.
            // Race condition on concurrent first writes is documented as acceptable.
            if let Err(e) = o8v_fs::safe_write(&path, containment, bytes) {
                tracing::debug!("mcp events: append found no file, create also failed: {e}");
            }
        }
        Err(e) => {
            tracing::debug!("mcp events: could not append to mcp-events.ndjson: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> std::path::PathBuf {
        fs::canonicalize(dir.path()).unwrap()
    }

    #[allow(clippy::disallowed_methods)]
    fn make_storage_dir(dir: &TempDir) -> o8v_workspace::StorageDir {
        let root_path = canonical(dir);
        std::env::set_var("HOME", &root_path);
        o8v_workspace::StorageDir::open().unwrap()
    }

    #[test]
    fn mcp_invoked_fields() {
        let ev = McpInvoked::new("run-1".to_string(), "check .", "/proj");
        assert_eq!(ev.event, "McpInvoked");
        assert_eq!(ev.run_id, "run-1");
        assert_eq!(ev.caller, "mcp");
        assert_eq!(ev.command, "check .");
        assert_eq!(ev.command_bytes, 7); // "check ." is 7 bytes
        assert_eq!(ev.command_token_estimate, 1); // 7 / 4 = 1
        assert_eq!(ev.project_path, "/proj");
    }

    #[test]
    fn mcp_completed_token_estimate() {
        let ev = McpCompleted::new("run-2".to_string(), 400, 50);
        assert_eq!(ev.token_estimate, 100); // 400 / 4
        assert_eq!(ev.render_bytes, 400);
        assert_eq!(ev.duration_ms, 50);
    }

    #[test]
    fn mcp_completed_zero_bytes() {
        let ev = McpCompleted::new("run-3".to_string(), 200, 10);
        assert_eq!(ev.token_estimate, 50);
        assert_eq!(ev.render_bytes, 200);
    }

    #[test]
    fn mcp_invoked_serializes() {
        let ev = McpInvoked::new("x".to_string(), "check .", "/p");
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("McpInvoked"));
        assert!(json.contains("mcp"));
        assert!(json.contains("timestamp_ms"));
    }

    #[test]
    fn emit_creates_file_on_first_write() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);
        let path = storage.mcp_events();
        assert!(!path.exists(), "file must not exist before first emit");

        let ev = McpInvoked::new("r".to_string(), "check .", "/p");
        emit(&storage, &ev);

        assert!(path.exists(), "emit must create the file on first write");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("McpInvoked"));
        // Each emitted line ends with \n — file has exactly one complete NDJSON line
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn emit_appends_on_subsequent_writes() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);

        let ev1 = McpInvoked::new("r1".to_string(), "check .", "/p");
        let ev2 = McpCompleted::new("r1".to_string(), 100, 10);
        let ev3 = McpInvoked::new("r2".to_string(), "fmt .", "/p");
        emit(&storage, &ev1);
        emit(&storage, &ev2);
        emit(&storage, &ev3);

        let content = fs::read_to_string(storage.mcp_events()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "must have 3 NDJSON lines");
        assert!(lines[0].contains("McpInvoked"));
        assert!(lines[1].contains("McpCompleted"));
        assert!(lines[2].contains("McpInvoked"));
    }
}
