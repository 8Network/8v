// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Dispatch layer for Claude Code hook events.
//!
//! `handle_pre` and `handle_post` are the two entry points called by the
//! hook subcommands (Slice 3). They parse stdin JSON, build lifecycle events,
//! and write them to the event store — no clap wiring here.

use std::io::{self, ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};

use o8v_core::caller::Caller;
use o8v_core::events::{CommandCompleted, CommandStarted};
use o8v_core::types::SessionId;

use crate::hook::argv_map::build_argv;
use crate::hook::payload::{PostToolUsePayload, PreToolUsePayload};
use crate::hook::run_id::{
    delete_run_record, mint_run_id, read_run_record, temp_path, write_run_record,
};
use crate::workspace::StorageDir;

/// Errors returned by the dispatch layer.
///
/// Callers (Slice 3 subcommands) catch these and exit 0 — Claude hooks must
/// never block the agent pipeline. The error is for internal propagation only.
#[derive(Debug)]
pub enum HookError {
    Json(serde_json::Error),
    Io(io::Error),
}

impl From<serde_json::Error> for HookError {
    fn from(e: serde_json::Error) -> Self {
        HookError::Json(e)
    }
}

impl From<io::Error> for HookError {
    fn from(e: io::Error) -> Self {
        HookError::Io(e)
    }
}

/// Returns the current wall-clock time as Unix milliseconds.
fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

/// Appends a lifecycle event to the NDJSON event store.
fn emit(storage: &StorageDir, bytes: &[u8]) -> Result<(), HookError> {
    let mut line = bytes.to_vec();
    line.push(b'\n');
    let path = storage.events();
    let containment = storage.containment();
    match o8v_fs::safe_append(&path, containment, &line) {
        Ok(()) => Ok(()),
        Err(o8v_fs::FsError::NotFound { .. }) => o8v_fs::safe_write(&path, containment, &line)
            .map_err(|e| HookError::Io(io::Error::other(e.to_string()))),
        Err(e) => Err(HookError::Io(io::Error::other(e.to_string()))),
    }
}

/// Handles a `PreToolUse` hook event delivered on stdin.
///
/// Parses the JSON payload, builds a `CommandStarted` event with
/// `Caller::Hook`, mints a run_id, records the pre-call timestamp, writes
/// the temp-file correlation record, then emits the event to the store.
pub fn handle_pre(stdin: &str, storage: &StorageDir) -> Result<(), HookError> {
    let payload: PreToolUsePayload = serde_json::from_str(stdin)?;

    let session_id = SessionId::from_claude_session_id(&payload.session_id);
    let run_id = mint_run_id();
    let pre_ms = now_ms();

    // Write correlation temp file before emitting so that handle_post can
    // always find (run_id, pre_ms) even if the process crashes between the
    // two hook firings.
    let path = temp_path(
        storage.containment().as_path(),
        session_id.as_str(),
        &payload.tool_use_id,
    );
    write_run_record(&path, &run_id, pre_ms)?;

    let argv = build_argv(&payload.tool_name, &payload.tool_input);
    let command = argv.first().cloned().unwrap_or_default();

    let mut ev = CommandStarted::new(run_id, Caller::Hook, command, argv, None);
    ev.session_id = session_id;

    let bytes = serde_json::to_vec(&ev)?;
    emit(storage, &bytes)?;

    Ok(())
}

/// Handles a `PostToolUse` hook event delivered on stdin.
///
/// Reads the correlation temp file written by [`handle_pre`] to recover
/// `run_id` and `pre_ms`. If the file is missing (orphaned PostToolUse),
/// mints a fresh run_id, synthesizes a `CommandStarted`, then emits
/// `CommandCompleted` with `duration_ms=0`.
///
/// Per design §3.4: PostToolUse only fires on success, so `success=true`.
pub fn handle_post(stdin: &str, storage: &StorageDir) -> Result<(), HookError> {
    let payload: PostToolUsePayload = serde_json::from_str(stdin)?;

    let session_id = SessionId::from_claude_session_id(&payload.session_id);
    let path = temp_path(
        storage.containment().as_path(),
        session_id.as_str(),
        &payload.tool_use_id,
    );

    let (run_id, pre_ms, synthetic) = match read_run_record(&path) {
        Ok((rid, ms)) => (rid, ms, false),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Orphaned PostToolUse: no matching PreToolUse temp file.
            (mint_run_id(), 0u64, true)
        }
        Err(e) => return Err(HookError::Io(e)),
    };

    let post_ms = now_ms();
    let duration_ms = if synthetic || post_ms < pre_ms {
        0
    } else {
        post_ms - pre_ms
    };

    // Output size: byte length of the tool_response JSON.
    let output_bytes = payload.tool_response.to_string().len() as u64;

    if synthetic {
        // Emit a synthetic CommandStarted so the event stream is consistent.
        let argv = build_argv(&payload.tool_name, &payload.tool_input);
        let command = argv.first().cloned().unwrap_or_default();
        let mut started = CommandStarted::new(run_id.clone(), Caller::Hook, command, argv, None);
        started.session_id = session_id.clone();
        let started_bytes = serde_json::to_vec(&started)?;
        emit(storage, &started_bytes)?;
    } else {
        // Best-effort: ignore NotFound (already handled above), propagate other errors.
        delete_run_record(&path)?;
    }

    let mut completed = CommandCompleted::new(run_id, output_bytes, duration_ms, true);
    completed.session_id = session_id;
    let completed_bytes = serde_json::to_vec(&completed)?;
    emit(storage, &completed_bytes)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_storage(dir: &TempDir) -> StorageDir {
        StorageDir::at(dir.path()).unwrap()
    }

    fn read_events(storage: &StorageDir) -> Vec<serde_json::Value> {
        let path = storage.events();
        if !path.exists() {
            return Vec::new();
        }
        let content = fs::read_to_string(path).unwrap();
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn pre_stdin(session_id: &str, tool_use_id: &str, tool_name: &str) -> String {
        format!(
            r#"{{
                "hook_event_name": "PreToolUse",
                "session_id": "{session_id}",
                "tool_use_id": "{tool_use_id}",
                "tool_name": "{tool_name}",
                "tool_input": {{"command": "ls -la"}}
            }}"#
        )
    }

    fn post_stdin(session_id: &str, tool_use_id: &str, tool_name: &str) -> String {
        format!(
            r#"{{
                "hook_event_name": "PostToolUse",
                "session_id": "{session_id}",
                "tool_use_id": "{tool_use_id}",
                "tool_name": "{tool_name}",
                "tool_input": {{"command": "ls -la"}},
                "tool_response": {{"output": "file1\nfile2"}}
            }}"#
        )
    }

    // --- handle_pre ---

    #[test]
    fn handle_pre_writes_command_started() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let stdin = pre_stdin("claude_session_abc", "toolu_001", "Bash");
        handle_pre(&stdin, &storage).expect("handle_pre must succeed");

        let events = read_events(&storage);
        assert_eq!(events.len(), 1, "must emit exactly one event");
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
    }

    #[test]
    fn handle_pre_writes_temp_file() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let stdin = pre_stdin("ses_pre_temp", "toolu_002", "Read");
        handle_pre(&stdin, &storage).expect("handle_pre must succeed");

        let session_id = SessionId::from_claude_session_id("ses_pre_temp");
        let path = temp_path(
            storage.containment().as_path(),
            session_id.as_str(),
            "toolu_002",
        );
        assert!(
            path.exists(),
            "temp correlation file must exist after handle_pre"
        );

        let (run_id, pre_ms) = read_run_record(&path).unwrap();
        assert_eq!(run_id.len(), 26, "run_id must be a 26-char ULID");
        assert!(pre_ms > 0, "pre_ms must be a non-zero timestamp");
    }

    // --- handle_post after handle_pre (paired) ---

    #[test]
    fn handle_post_after_pre_pairs_correctly() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let session = "claude_session_pair";
        let tool_use_id = "toolu_pair_001";

        handle_pre(&pre_stdin(session, tool_use_id, "Bash"), &storage)
            .expect("handle_pre must succeed");
        handle_post(&post_stdin(session, tool_use_id, "Bash"), &storage)
            .expect("handle_post must succeed");

        let events = read_events(&storage);
        assert_eq!(
            events.len(),
            2,
            "must emit CommandStarted then CommandCompleted"
        );

        assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
        assert_eq!(events[1]["event"].as_str().unwrap(), "CommandCompleted");

        // run_ids must match.
        let started_run_id = events[0]["run_id"].as_str().unwrap();
        let completed_run_id = events[1]["run_id"].as_str().unwrap();
        assert_eq!(
            started_run_id, completed_run_id,
            "run_id must be the same in both events"
        );

        // session_ids must match.
        let started_session = events[0]["session_id"].as_str().unwrap();
        let completed_session = events[1]["session_id"].as_str().unwrap();
        assert_eq!(
            started_session, completed_session,
            "session_id must be the same in both events"
        );
        assert!(
            started_session.starts_with("ses_"),
            "session_id must start with ses_"
        );

        assert!(
            events[1]["success"].as_bool().unwrap(),
            "success must be true"
        );

        // Temp file must be cleaned up.
        let derived_session = SessionId::from_claude_session_id(session);
        let path = temp_path(
            storage.containment().as_path(),
            derived_session.as_str(),
            tool_use_id,
        );
        assert!(
            !path.exists(),
            "temp file must be deleted after handle_post"
        );
    }

    // --- handle_post without handle_pre (orphaned) ---

    #[test]
    fn handle_post_without_pre_synthesizes_started_plus_completed() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let stdin = post_stdin("claude_session_orphan", "toolu_orphan_001", "Read");
        handle_post(&stdin, &storage).expect("handle_post must succeed for orphaned event");

        let events = read_events(&storage);
        assert_eq!(
            events.len(),
            2,
            "orphaned PostToolUse must synthesize CommandStarted + CommandCompleted"
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

    // --- malformed JSON ---

    #[test]
    fn handle_pre_malformed_json_returns_err() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let result = handle_pre("this is not json", &storage);
        assert!(
            result.is_err(),
            "malformed JSON must return Err from handle_pre"
        );

        // No events must have been written.
        let events = read_events(&storage);
        assert!(
            events.is_empty(),
            "no events must be emitted on parse failure"
        );
    }

    #[test]
    fn handle_post_malformed_json_returns_err() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage(&dir);

        let result = handle_post("{invalid}", &storage);
        assert!(
            result.is_err(),
            "malformed JSON must return Err from handle_post"
        );
    }
}
