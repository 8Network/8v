// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Event reader — reads events.ndjson and returns typed events.
//!
//! Symmetric with StorageSubscriber (the writer).
//! Uses StorageDir for the path, o8v-fs for safe reads.

use crate::workspace::StorageDir;
use o8v_core::events::Event;
use o8v_core::types::{Warning, WarningSink};

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
            Self::InvalidEventField { line } => {
                write!(f, "line {line}: \"event\" field is not a string")
            }
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
    // Use raw fs instead of safe_read: events.ndjson is written by 8v itself
    // (not user-supplied), so the safe_read size cap does not apply.
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(EventReadError::Io(e.to_string())),
    };
    parse_events(&content)
}

/// Parse NDJSON content into typed events — lenient mode.
///
/// In lenient mode (`strict = false`): skips malformed lines, collecting a warning
/// string per skipped line into the returned Vec<String>.
/// In strict mode (`strict = true`): hard-fails on the first malformed line,
/// identical to `parse_events`.
///
/// The existing `parse_events` keeps its hard-fail contract unchanged.
pub fn parse_events_lenient(
    content: &str,
    strict: bool,
    warnings: &mut WarningSink,
) -> Result<Vec<Event>, EventReadError> {
    let mut events = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;

        if line.trim().is_empty() {
            continue;
        }

        let raw: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let err = EventReadError::InvalidJson {
                    line: line_num,
                    source: e.to_string(),
                };
                if strict {
                    return Err(err);
                }
                warnings.push(Warning::MalformedEventLine {
                    line_no: line_num as u64,
                    reason: err.to_string(),
                });
                continue;
            }
        };

        let event_type = match raw.get("event") {
            Some(v) => match v.as_str() {
                Some(s) => s,
                None => {
                    let err = EventReadError::InvalidEventField { line: line_num };
                    if strict {
                        return Err(err);
                    }
                    warnings.push(Warning::MalformedEventLine {
                        line_no: line_num as u64,
                        reason: err.to_string(),
                    });
                    continue;
                }
            },
            None => {
                let err = EventReadError::MissingEventField { line: line_num };
                if strict {
                    return Err(err);
                }
                warnings.push(Warning::MalformedEventLine {
                    line_no: line_num as u64,
                    reason: err.to_string(),
                });
                continue;
            }
        };

        let event = match event_type {
            "CommandStarted" => {
                match serde_json::from_str::<o8v_core::events::lifecycle::CommandStarted>(line) {
                    Ok(started) => Event::CommandStarted(started),
                    Err(e) => {
                        let err = EventReadError::InvalidJson {
                            line: line_num,
                            source: e.to_string(),
                        };
                        if strict {
                            return Err(err);
                        }
                        warnings.push(Warning::MalformedEventLine {
                            line_no: line_num as u64,
                            reason: err.to_string(),
                        });
                        continue;
                    }
                }
            }
            "CommandCompleted" => {
                match serde_json::from_str::<o8v_core::events::lifecycle::CommandCompleted>(line) {
                    Ok(completed) => Event::CommandCompleted(completed),
                    Err(e) => {
                        let err = EventReadError::InvalidJson {
                            line: line_num,
                            source: e.to_string(),
                        };
                        if strict {
                            return Err(err);
                        }
                        warnings.push(Warning::MalformedEventLine {
                            line_no: line_num as u64,
                            reason: err.to_string(),
                        });
                        continue;
                    }
                }
            }
            other => Event::Unknown {
                event_type: other.to_string(),
                raw,
            },
        };

        events.push(event);
    }

    Ok(events)
}

/// Read all events from the event store — lenient mode.
///
/// In lenient mode (`strict = false`): malformed lines are skipped with a
/// warning string per line. In strict mode: hard-fails on first malformed line.
pub fn read_events_lenient(
    storage: &StorageDir,
    strict: bool,
    warnings: &mut WarningSink,
) -> Result<Vec<Event>, EventReadError> {
    let path = storage.events();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return parse_events_lenient("", strict, warnings)
        }
        Err(e) => return Err(EventReadError::Io(e.to_string())),
    };

    parse_events_lenient(&content, strict, warnings)
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

        let raw: serde_json::Value =
            serde_json::from_str(line).map_err(|e| EventReadError::InvalidJson {
                line: line_num,
                source: e.to_string(),
            })?;

        let event_type = raw
            .get("event")
            .ok_or(EventReadError::MissingEventField { line: line_num })?
            .as_str()
            .ok_or(EventReadError::InvalidEventField { line: line_num })?;

        let event = match event_type {
            "CommandStarted" => {
                let started: o8v_core::events::lifecycle::CommandStarted =
                    serde_json::from_str(line).map_err(|e| EventReadError::InvalidJson {
                        line: line_num,
                        source: e.to_string(),
                    })?;
                Event::CommandStarted(started)
            }
            "CommandCompleted" => {
                let completed: o8v_core::events::lifecycle::CommandCompleted =
                    serde_json::from_str(line).map_err(|e| EventReadError::InvalidJson {
                        line: line_num,
                        source: e.to_string(),
                    })?;
                Event::CommandCompleted(completed)
            }
            other => Event::Unknown {
                event_type: other.to_string(),
                raw,
            },
        };

        events.push(event);
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::caller::Caller;
    use o8v_core::events::lifecycle::{CommandCompleted, CommandStarted};

    #[test]
    fn parse_valid_events() {
        let started = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check .",
            vec!["check".into(), ".".into()],
            None,
        );
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
        let started = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check .",
            vec!["check".into(), ".".into()],
            None,
        );
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
        let content = r#"{"event":"CommandStarted","run_id":"r1","timestamp_ms":1000,"version":"0.1.0","caller":"cli","command":"check .","command_bytes":7,"command_token_estimate":1,"argv":["check","."],"project_path":null,"session_id":"ses_01HZZZZZZZZZZZZZZZZZZZZZZ"}
not valid json
{"event":"CommandCompleted","run_id":"r1","timestamp_ms":1050,"output_bytes":400,"token_estimate":100,"duration_ms":50,"success":true,"session_id":"ses_01HZZZZZZZZZZZZZZZZZZZZZZ"}"#;
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
        assert!(
            matches!(&events[0], Event::Unknown { event_type, .. } if event_type == "FutureEvent")
        );
    }

    // ─── Lenient parser tests ─────────────────────────────────────────────────

    #[test]
    fn lenient_skips_corrupt_line() {
        let started = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check .",
            vec!["check".into(), ".".into()],
            None,
        );
        let line1 = serde_json::to_string(&started).unwrap();
        let content = format!("{line1}\nnot valid json\n");

        let mut sink = WarningSink::new();
        let events = parse_events_lenient(&content, false, &mut sink).unwrap();
        let warnings = sink.into_inner();
        assert_eq!(events.len(), 1, "good line must be kept");
        assert_eq!(warnings.len(), 1, "corrupt line must produce one warning");
        match &warnings[0] {
            Warning::MalformedEventLine { line_no, .. } => assert_eq!(*line_no, 2),
            other => panic!("expected MalformedEventLine, got {other:?}"),
        }
    }

    #[test]
    fn strict_fails_on_corrupt_line() {
        let content = r#"not valid json"#;
        let mut sink = WarningSink::new();
        let result = parse_events_lenient(content, true, &mut sink);
        assert!(
            matches!(result, Err(EventReadError::InvalidJson { line: 1, .. })),
            "strict mode must hard-fail; got: {result:?}"
        );
    }

    #[test]
    fn lenient_skips_missing_event_field() {
        let content = r#"{"run_id":"r1","timestamp_ms":1000}"#;
        let mut sink = WarningSink::new();
        let events = parse_events_lenient(content, false, &mut sink).unwrap();
        let warnings = sink.into_inner();
        assert!(events.is_empty());
        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            Warning::MalformedEventLine { reason, .. } => {
                assert!(reason.contains("missing \"event\" field"));
            }
            other => panic!("expected MalformedEventLine, got {other:?}"),
        }
    }

    #[test]
    fn lenient_returns_warnings_for_multiple_bad_lines() {
        let started = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check .",
            vec!["check".into(), ".".into()],
            None,
        );
        let good = serde_json::to_string(&started).unwrap();
        let content = format!("bad1\n{good}\nbad2\n");

        let mut sink = WarningSink::new();
        let events = parse_events_lenient(&content, false, &mut sink).unwrap();
        let warnings = sink.into_inner();
        assert_eq!(events.len(), 1, "good line must be kept");
        assert_eq!(warnings.len(), 2, "two bad lines → two warnings");
        let lines: Vec<u64> = warnings
            .iter()
            .map(|w| match w {
                Warning::MalformedEventLine { line_no, .. } => *line_no,
                other => panic!("expected MalformedEventLine, got {other:?}"),
            })
            .collect();
        assert_eq!(lines, vec![1, 3]);
    }

    #[test]
    fn lenient_strict_true_equals_parse_events_behavior() {
        // strict=true must behave identically to parse_events for valid content
        let started = CommandStarted::new(
            "r1".into(),
            Caller::Cli,
            "check .",
            vec!["check".into(), ".".into()],
            None,
        );
        let content = serde_json::to_string(&started).unwrap();
        let mut sink = WarningSink::new();
        let events = parse_events_lenient(&content, true, &mut sink).unwrap();
        assert_eq!(events.len(), 1);
        assert!(
            sink.is_empty(),
            "no warnings on valid content in strict mode"
        );
    }

    // ─── Counterexample tests (POC-regression pins) ───────────────────────────

    #[test]
    fn lenient_commandstarted_body_missing_required_fields_emits_warning_not_panic() {
        // POC: serde deserialization of CommandStarted was called via unwrap(); a body
        // with the correct "event" type but missing required fields caused a panic.
        // Now: the Err arm emits Warning::MalformedEventLine and continues — no panic.
        let content = r#"{"event":"CommandStarted","run_id":"r1"}"#;
        let mut sink = WarningSink::new();
        let result = parse_events_lenient(content, false, &mut sink);
        assert!(result.is_ok(), "lenient mode must not return Err");
        let events = result.unwrap();
        let warnings = sink.into_inner();
        assert!(
            events.is_empty(),
            "malformed body must not produce an Event"
        );
        assert_eq!(warnings.len(), 1, "must emit exactly one warning");
        assert!(
            matches!(&warnings[0], Warning::MalformedEventLine { line_no: 1, .. }),
            "expected MalformedEventLine at line 1, got {:?}",
            warnings[0]
        );
    }

    #[test]
    fn lenient_first_bad_line_reports_line_number_one_not_zero() {
        // POC: line counter used the enumerate index `i` directly (0-based), so the
        // first bad line was reported as line_no=0 instead of line_no=1.
        // Now: line_num = i + 1, so the first line is always reported as 1.
        let content = "not valid json\n";
        let mut sink = WarningSink::new();
        let _ = parse_events_lenient(content, false, &mut sink).unwrap();
        let warnings = sink.into_inner();
        assert_eq!(warnings.len(), 1);
        assert!(
            matches!(&warnings[0], Warning::MalformedEventLine { line_no: 1, .. }),
            "first bad line must be line_no=1, got {:?}",
            warnings[0]
        );
    }

    #[test]
    fn lenient_blank_lines_do_not_shift_reported_line_numbers() {
        // POC: blank lines were counted in the line index before being discarded,
        // causing subsequent bad lines to be reported at the wrong (higher) line number.
        // Now: blank lines are skipped with `continue` AFTER incrementing the counter,
        // so the physical line position is still accurately reported for bad lines.
        // Layout: line 1 = blank, line 2 = bad JSON, line 3 = blank, line 4 = bad JSON.
        let content = "\nnot valid json\n\nalso bad\n";
        let mut sink = WarningSink::new();
        let _ = parse_events_lenient(content, false, &mut sink).unwrap();
        let warnings = sink.into_inner();
        assert_eq!(warnings.len(), 2, "two bad lines must produce two warnings");
        let line_nos: Vec<u64> = warnings
            .iter()
            .map(|w| match w {
                Warning::MalformedEventLine { line_no, .. } => *line_no,
                other => panic!("expected MalformedEventLine, got {other:?}"),
            })
            .collect();
        // Physical line positions: bad JSON is at line 2 and line 4.
        assert_eq!(
            line_nos,
            vec![2, 4],
            "blank lines must not shift the reported line numbers"
        );
    }
}
