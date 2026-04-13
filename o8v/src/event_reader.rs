// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Event reader — reads events.ndjson and returns typed events.
//!
//! Symmetric with StorageSubscriber (the writer).
//! Uses StorageDir for the path, o8v-fs for safe reads.

use o8v_core::events::Event;
use crate::workspace::StorageDir;

#[derive(Debug)]
pub enum EventReadError {
    /// File not found or cannot be read.
    Io(String),
    /// A line is not valid JSON.
    InvalidJson { line: usize, source: String },
    /// JSON object missing the "event" field.
    MissingEventField { line: usize },
    /// The "event" field is not a string.
    InvalidEventField { line: usize },
}

impl std::fmt::Display for EventReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "{msg}"),
            Self::InvalidJson { line, source } => write!(f, "line {line}: invalid JSON: {source}"),
            Self::MissingEventField { line } => write!(f, "line {line}: missing \"event\" field"),
            Self::InvalidEventField { line } => write!(f, "line {line}: \"event\" field is not a string"),
        }
    }
}

impl std::error::Error for EventReadError {}

/// Read all events from the event store.
///
/// Errors on corrupt data — no silent fallbacks.
/// Unknown event types are returned as Event::Unknown (forward compatibility).
/// Empty lines are skipped (valid in NDJSON).
pub fn read_events(storage: &StorageDir) -> Result<Vec<Event>, EventReadError> {
    let path = storage.events();
    let config = o8v_fs::FsConfig::default();
    let content = o8v_fs::safe_read(&path, storage.containment(), &config)
        .map_err(|e| EventReadError::Io(e.to_string()))?;

    parse_events(content.content())
}

/// Parse NDJSON content into typed events.
/// Separated from read_events for testability.
pub fn parse_events(content: &str) -> Result<Vec<Event>, EventReadError> {
    let mut events = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1; // 1-based for human display

        if line.trim().is_empty() {
            continue;
        }

        let raw: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| EventReadError::InvalidJson { line: line_num, source: e.to_string() })?;

        let event_type = raw.get("event")
            .ok_or(EventReadError::MissingEventField { line: line_num })?
            .as_str()
            .ok_or(EventReadError::InvalidEventField { line: line_num })?;

        let event = match event_type {
            "CommandStarted" => {
                let started: o8v_core::events::lifecycle::CommandStarted = serde_json::from_str(line)
                    .map_err(|e| EventReadError::InvalidJson { line: line_num, source: e.to_string() })?;
                Event::CommandStarted(started)
            }
            "CommandCompleted" => {
                let completed: o8v_core::events::lifecycle::CommandCompleted = serde_json::from_str(line)
                    .map_err(|e| EventReadError::InvalidJson { line: line_num, source: e.to_string() })?;
                Event::CommandCompleted(completed)
            }
            other => Event::Unknown { event_type: other.to_string(), raw },
        };

        events.push(event);
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::events::lifecycle::{CommandCompleted, CommandStarted};
    use o8v_core::caller::Caller;

    #[test]
    fn parse_valid_events() {
        let started = CommandStarted::new("r1".into(), Caller::Cli, "check .", None);
        let completed = CommandCompleted::new("r1".into(), 400, 50, true);
        let line1 = serde_json::to_string(&started).unwrap();
        let line2 = serde_json::to_string(&completed).unwrap();
        let content = format!("{line1}\n{line2}");

        let events = parse_events(&content).unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            Event::CommandStarted(s) => assert_eq!(s.run_id, "r1"),
            other => panic!("expected CommandStarted, got {other:?}"),
        }
        match &events[1] {
            Event::CommandCompleted(c) => {
                assert_eq!(c.run_id, "r1");
                assert!(c.success);
            }
            other => panic!("expected CommandCompleted, got {other:?}"),
        }
    }

    #[test]
    fn parse_empty_content() {
        let events = parse_events("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn parse_empty_lines_skipped() {
        let started = CommandStarted::new("r1".into(), Caller::Cli, "check .", None);
        let completed = CommandCompleted::new("r1".into(), 400, 50, true);
        let line1 = serde_json::to_string(&started).unwrap();
        let line2 = serde_json::to_string(&completed).unwrap();
        // Surround with blank lines — empty lines must be skipped.
        let content = format!("\n{line1}\n\n{line2}\n");

        let events = parse_events(&content).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn parse_corrupt_json_errors() {
        let content = r#"{"event":"CommandStarted","run_id":"r1","timestamp_ms":1000,"version":"0.1.0","caller":"cli","command":"check .","command_bytes":7,"command_token_estimate":1,"project_path":null}
not valid json
{"event":"CommandCompleted","run_id":"r1","timestamp_ms":1050,"output_bytes":400,"token_estimate":100,"duration_ms":50,"success":true}"#;
        let err = parse_events(content).unwrap_err();
        assert!(matches!(err, EventReadError::InvalidJson { line: 2, .. }));
    }

    #[test]
    fn parse_missing_event_field_errors() {
        let content = r#"{"run_id":"r1","timestamp_ms":1000}"#;
        let err = parse_events(content).unwrap_err();
        assert!(matches!(err, EventReadError::MissingEventField { line: 1 }));
    }

    #[test]
    fn parse_event_field_not_string_errors() {
        let content = r#"{"event":42,"run_id":"r1"}"#;
        let err = parse_events(content).unwrap_err();
        assert!(matches!(err, EventReadError::InvalidEventField { line: 1 }));
    }

    #[test]
    fn parse_unknown_event_type() {
        let content = r#"{"event":"FutureEvent","data":"something"}"#;
        let events = parse_events(content).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::Unknown { event_type, .. } if event_type == "FutureEvent"));
    }
}
