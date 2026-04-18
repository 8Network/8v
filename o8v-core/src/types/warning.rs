// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `Warning` — the single typed channel for non-fatal events that the user
//! must be able to see.
//!
//! Every silent fallback in the POC (canonicalize-failed-then-keep-raw,
//! reversed-clock, empty session_id, duplicate CommandCompleted, …) becomes
//! a typed variant here. Warnings are carried through every layer and
//! rendered in the report envelope. Dropping warnings at any layer is a bug.

use crate::types::{SessionId, TimestampMs};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    /// `fs::canonicalize` failed when normalizing an argv path; the raw path
    /// was kept in the shape. The aggregator groups may over-fragment in
    /// this case (e.g. `./a.rs` vs the canonical absolute path).
    CanonicalizeFailed { path: String, reason: String },

    /// Two `CommandCompleted` events shared the same `run_id`. The first is
    /// kept; later duplicates are dropped.
    DuplicateCompleted { run_id: String },

    /// Two `CommandStarted` events shared the same `run_id`. The first is
    /// kept; later duplicates are dropped.
    DuplicateStarted { run_id: String },

    /// A `CommandCompleted` event's timestamp precedes its matching
    /// `CommandStarted`. The command is still recorded but its duration is
    /// set to 0 and clusters skip it.
    ReversedTimestamps {
        session: SessionId,
        earlier: TimestampMs,
        later: TimestampMs,
    },

    /// An event's `session_id` failed validation. The event is dropped.
    EmptySessionId { at: TimestampMs, reason: String },

    /// A `--since` value resolved to a wall-clock time in the future.
    FutureSince { since_ms: i64, now_ms: i64 },

    /// A line in the event file could not be parsed.
    MalformedEventLine { line_no: u64, reason: String },

    /// A `CommandStarted` with no matching `CommandCompleted` inside the
    /// aggregation window.
    OrphanStarted { run_id: String },

    /// A `CommandCompleted` with no matching `CommandStarted`.
    OrphanCompleted { run_id: String },

    /// The argv normalizer could not determine a project-relative path for
    /// an argv token and fell back to the basename. Emitted once per session
    /// to avoid spam.
    NormalizerBasenameFallback { session: SessionId, path: String },

    /// A percentile request fell outside [0.0, 1.0] or was NaN.
    PercentileOutOfRange { p: f64 },

    /// A time-window flag (`--since`, `--until`) was ignored because `--session`
    /// takes precedence and pins the time span to the session's own range.
    FlagIgnoredForSession { flag: String },
}

impl Warning {
    /// Return the `SessionId` this warning is scoped to, if any.
    ///
    /// Returns `Some` only for variants that carry a session field
    /// (`NormalizerBasenameFallback`, `ReversedTimestamps`).  All other
    /// variants are considered global — they are not tied to a specific
    /// session and are always included regardless of the rendering limit.
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            Warning::NormalizerBasenameFallback { session, .. } => Some(session),
            Warning::ReversedTimestamps { session, .. } => Some(session),
            _ => None,
        }
    }

    /// Short machine tag, matching the serde `kind` field.
    pub fn kind(&self) -> &'static str {
        match self {
            Warning::CanonicalizeFailed { .. } => "canonicalize_failed",
            Warning::DuplicateCompleted { .. } => "duplicate_completed",
            Warning::DuplicateStarted { .. } => "duplicate_started",
            Warning::ReversedTimestamps { .. } => "reversed_timestamps",
            Warning::EmptySessionId { .. } => "empty_session_id",
            Warning::FutureSince { .. } => "future_since",
            Warning::MalformedEventLine { .. } => "malformed_event_line",
            Warning::OrphanStarted { .. } => "orphan_started",
            Warning::OrphanCompleted { .. } => "orphan_completed",
            Warning::NormalizerBasenameFallback { .. } => "normalizer_basename_fallback",
            Warning::PercentileOutOfRange { .. } => "percentile_out_of_range",
            Warning::FlagIgnoredForSession { .. } => "flag_ignored_for_session",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_has_kind_tag() {
        let w = Warning::DuplicateCompleted {
            run_id: "abc".into(),
        };
        let json = serde_json::to_value(&w).unwrap();
        assert_eq!(json["kind"], "duplicate_completed");
        assert_eq!(json["run_id"], "abc");
    }

    #[test]
    fn reversed_timestamps_serializes_session() {
        let w = Warning::ReversedTimestamps {
            session: SessionId::new(),
            earlier: TimestampMs::from_millis(100),
            later: TimestampMs::from_millis(50),
        };
        let json = serde_json::to_value(&w).unwrap();
        assert_eq!(json["kind"], "reversed_timestamps");
        assert!(json["session"].as_str().unwrap().starts_with("ses_"));
        assert_eq!(json["earlier"], 100);
        assert_eq!(json["later"], 50);
    }

    #[test]
    fn kind_helper_matches_serde_tag() {
        let cases: &[Warning] = &[
            Warning::CanonicalizeFailed {
                path: "/x".into(),
                reason: "io".into(),
            },
            Warning::FutureSince {
                since_ms: 10,
                now_ms: 5,
            },
            Warning::PercentileOutOfRange { p: 1.5 },
        ];
        for w in cases {
            let json = serde_json::to_value(w).unwrap();
            assert_eq!(json["kind"], w.kind());
        }
    }
}
