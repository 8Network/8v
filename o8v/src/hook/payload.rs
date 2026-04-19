// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Serde-deserialize types for Claude Code hook stdin JSON payloads.
//!
//! Claude Code delivers hook events as JSON on stdin. These structs capture
//! the fields that the hook layer consumes. Unknown fields are ignored.

use serde::Deserialize;

/// Payload delivered by Claude Code for the `PreToolUse` hook event.
#[derive(Debug, Deserialize)]
pub struct PreToolUsePayload {
    pub hook_event_name: String,
    pub session_id: String,
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

/// Payload delivered by Claude Code for the `PostToolUse` hook event.
#[derive(Debug, Deserialize)]
pub struct PostToolUsePayload {
    pub hook_event_name: String,
    pub session_id: String,
    pub tool_use_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_response: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_tool_use_payload_parses_sample_fixture() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "abc123",
            "tool_use_id": "toolu_01XYZ",
            "tool_name": "Bash",
            "tool_input": { "command": "ls -la" }
        }"#;

        let payload: PreToolUsePayload =
            serde_json::from_str(json).expect("must parse PreToolUsePayload");

        assert_eq!(payload.hook_event_name, "PreToolUse");
        assert_eq!(payload.session_id, "abc123");
        assert_eq!(payload.tool_use_id, "toolu_01XYZ");
        assert_eq!(payload.tool_name, "Bash");
        assert_eq!(payload.tool_input["command"], "ls -la");
    }

    #[test]
    fn post_tool_use_payload_parses_sample_fixture() {
        let json = r#"{
            "hook_event_name": "PostToolUse",
            "session_id": "def456",
            "tool_use_id": "toolu_02ABC",
            "tool_name": "Read",
            "tool_input": { "file_path": "/home/user/main.rs" },
            "tool_response": { "content": "fn main() {}" }
        }"#;

        let payload: PostToolUsePayload =
            serde_json::from_str(json).expect("must parse PostToolUsePayload");

        assert_eq!(payload.hook_event_name, "PostToolUse");
        assert_eq!(payload.session_id, "def456");
        assert_eq!(payload.tool_use_id, "toolu_02ABC");
        assert_eq!(payload.tool_name, "Read");
        assert_eq!(payload.tool_input["file_path"], "/home/user/main.rs");
        assert_eq!(payload.tool_response["content"], "fn main() {}");
    }

    #[test]
    fn pre_tool_use_payload_ignores_unknown_fields() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "tool_use_id": "t1",
            "tool_name": "Grep",
            "tool_input": {},
            "unknown_future_field": "ignored"
        }"#;

        let payload: PreToolUsePayload =
            serde_json::from_str(json).expect("unknown fields must not cause parse failure");
        assert_eq!(payload.tool_name, "Grep");
    }

    #[test]
    fn pre_tool_use_payload_fails_on_missing_required_field() {
        // Missing tool_name — must fail to parse.
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "tool_use_id": "t1",
            "tool_input": {}
        }"#;

        let result: Result<PreToolUsePayload, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing tool_name must cause parse error");
    }
}
