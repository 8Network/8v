// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Lifecycle events — emitted by dispatch for every invocation.
//!
//! Both CLI and MCP get identical observability through the EventBus.

use crate::caller::{AgentInfo, Caller};
use crate::types::{SessionId, TimestampMs};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The 8v version string embedded in every event.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estimate token count from byte length (GPT-4 approximation: bytes / 4).
fn estimate_tokens(bytes: u64) -> u64 {
    bytes / 4
}

/// Returns the process-lifetime session ID, minted on first call.
fn process_session_id() -> SessionId {
    static SESSION: OnceLock<SessionId> = OnceLock::new();
    SESSION.get_or_init(SessionId::new).clone()
}

/// Emitted before a command executes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStarted {
    /// Event kind discriminator.
    pub event: String,
    /// UUID scoped to this command invocation.
    pub run_id: String,
    /// Unix milliseconds (typed to prevent signed/unsigned mixing downstream).
    pub timestamp_ms: TimestampMs,
    /// 8v version.
    pub version: String,
    /// Who invoked the command.
    pub caller: Caller,
    /// The subcommand category (e.g. "read", "check", "build"). Kept as a
    /// `String` on the wire for forward-compat; aggregation converts to the
    /// typed [`crate::types::CommandName`] and warns on unknowns.
    pub command: String,
    /// Full argument tail as captured at the entry point.
    pub argv: Vec<String>,
    /// Byte length of the command string.
    pub command_bytes: u64,
    /// Estimated token count: command_bytes / 4.
    pub command_token_estimate: u64,
    /// Absolute path of the project root, if known.
    pub project_path: Option<String>,
    /// MCP client identity from the initialize handshake, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<AgentInfo>,
    /// Process-lifetime session identifier. No `serde(default)` — every
    /// event must carry it. Legacy events without it fail to deserialize
    /// and surface a `MalformedEventLine` warning via `--strict` or are
    /// skipped with a warning otherwise.
    pub session_id: SessionId,
}

impl CommandStarted {
    pub fn new(
        run_id: String,
        caller: Caller,
        command: impl Into<String>,
        argv: Vec<String>,
        project_path: Option<String>,
    ) -> Self {
        let command = command.into();
        let command_bytes = command.len() as u64;
        Self {
            event: "CommandStarted".to_string(),
            run_id,
            timestamp_ms: TimestampMs::now(),
            version: VERSION.to_string(),
            caller,
            command,
            argv,
            command_bytes,
            command_token_estimate: estimate_tokens(command_bytes),
            project_path,
            agent_info: None,
            session_id: process_session_id(),
        }
    }

    pub fn with_agent_info(mut self, info: Option<AgentInfo>) -> Self {
        self.agent_info = info;
        self
    }
}

/// Emitted after a command completes (success or failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCompleted {
    /// Event kind discriminator.
    pub event: String,
    /// Matches the [`CommandStarted`] for this invocation.
    pub run_id: String,
    /// Unix milliseconds.
    pub timestamp_ms: TimestampMs,
    /// Byte length of the rendered output.
    pub output_bytes: u64,
    /// Estimated token count: output_bytes / 4.
    pub token_estimate: u64,
    /// Total command duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the command succeeded.
    pub success: bool,
    /// Process-lifetime session identifier. See [`CommandStarted::session_id`].
    pub session_id: SessionId,
}

impl CommandCompleted {
    pub fn new(run_id: String, output_bytes: u64, duration_ms: u64, success: bool) -> Self {
        Self {
            event: "CommandCompleted".to_string(),
            run_id,
            timestamp_ms: TimestampMs::now(),
            output_bytes,
            token_estimate: estimate_tokens(output_bytes),
            duration_ms,
            success,
            session_id: process_session_id(),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_started_fields() {
        let ev = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check",
            vec!["check".into(), ".".into()],
            Some("/proj".into()),
        );
        assert_eq!(ev.event, "CommandStarted");
        assert_eq!(ev.run_id, "r1");
        assert_eq!(ev.caller, Caller::Cli);
        assert_eq!(ev.command, "check");
        assert_eq!(ev.argv, vec!["check".to_string(), ".".into()]);
        assert_eq!(ev.project_path, Some("/proj".into()));
    }

    #[test]
    fn command_started_mcp_caller() {
        let ev = CommandStarted::new("r2".into(), Caller::Mcp, "fmt", vec!["fmt".into()], None);
        assert_eq!(ev.caller, Caller::Mcp);
        assert_eq!(ev.project_path, None);
        assert_eq!(ev.argv, vec!["fmt".to_string()]);
    }

    #[test]
    fn command_started_argv_serializes() {
        let ev = CommandStarted::new(
            "r9".into(),
            Caller::Cli,
            "read",
            vec!["read".into(), "--full".into(), "a.rs".into(), "b.rs".into()],
            None,
        );
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            json.contains(r#""argv":["read","--full","a.rs","b.rs"]"#),
            "argv must serialize as an ordered array; got: {json}"
        );
        let parsed: CommandStarted = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.argv, ev.argv);
    }

    #[test]
    fn command_completed_fields() {
        let ev = CommandCompleted::new("r1".into(), 400, 50, true);
        assert_eq!(ev.event, "CommandCompleted");
        assert_eq!(ev.output_bytes, 400);
        assert_eq!(ev.token_estimate, 100);
        assert_eq!(ev.duration_ms, 50);
        assert!(ev.success);
    }

    #[test]
    fn command_completed_failure() {
        let ev = CommandCompleted::new("r3".into(), 200, 10, false);
        assert!(!ev.success);
    }

    #[test]
    fn command_started_serializes() {
        let ev = CommandStarted::new(
            "x".into(),
            Caller::Cli,
            "check",
            vec!["check".into(), ".".into()],
            Some("/p".into()),
        );
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("CommandStarted"));
        assert!(json.contains("cli"));
        assert!(json.contains("timestamp_ms"));
        assert!(
            !json.contains("agent_info"),
            "agent_info should be skipped when None"
        );
    }

    #[test]
    fn command_started_with_agent_info_serializes() {
        let info = AgentInfo {
            name: "claude-code".into(),
            version: "1.0.23".into(),
            protocol_version: "2025-03-26".into(),
            capabilities: vec!["roots".into(), "sampling".into()],
        };
        let ev = CommandStarted::new(
            "r5".into(),
            Caller::Mcp,
            "check",
            vec!["check".into(), ".".into()],
            None,
        )
        .with_agent_info(Some(info));
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("agent_info"));
        assert!(json.contains("claude-code"));
        assert!(json.contains("1.0.23"));
        assert!(json.contains("2025-03-26"));
        assert!(json.contains("roots"));
        assert!(json.contains("sampling"));

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let ai = parsed.get("agent_info").unwrap();
        assert_eq!(ai["name"], "claude-code");
        assert_eq!(ai["version"], "1.0.23");
        assert_eq!(ai["protocol_version"], "2025-03-26");
    }

    #[test]
    fn command_completed_serializes() {
        let ev = CommandCompleted::new("x".into(), 100, 5, true);
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("CommandCompleted"));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn session_id_is_stamped() {
        let ev = CommandStarted::new(
            "r_s1".into(),
            Caller::Cli,
            "check",
            vec!["check".into()],
            None,
        );
        assert!(
            ev.session_id.as_str().starts_with("ses_"),
            "session_id must start with 'ses_'; got: {}",
            ev.session_id
        );
    }

    #[test]
    fn session_id_is_stable_within_process() {
        let ev1 = CommandStarted::new(
            "r_stable1".into(),
            Caller::Cli,
            "check",
            vec!["check".into()],
            None,
        );
        let ev2 = CommandCompleted::new("r_stable2".into(), 200, 10, true);
        assert_eq!(
            ev1.session_id, ev2.session_id,
            "all events in the same process must share a session_id"
        );
    }
}
