// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Builds the `SessionsTable` for `8v log` (no subcommand).

use crate::aggregator::SessionAggregate;
use o8v_core::render::log_report::{SessionRow, SessionsTable};

pub fn build_sessions_table(
    sessions: &[SessionAggregate],
    limit: usize,
    warnings: Vec<o8v_core::types::Warning>,
) -> SessionsTable {
    let total_count = sessions.len();
    // Collect the session IDs that will actually be rendered (most-recent `limit`).
    let rendered_ids: std::collections::HashSet<&str> = sessions
        .iter()
        .rev()
        .take(limit)
        .map(|s| s.session_id.as_str())
        .collect();

    // Keep only warnings that are either:
    //   (a) global (no session affinity — always shown), or
    //   (b) scoped to a session that is within the rendered window.
    let scoped_warnings: Vec<o8v_core::types::Warning> = warnings
        .into_iter()
        .filter(|w| {
            w.session_id()
                .map(|sid| rendered_ids.contains(sid.as_str()))
                .unwrap_or(true) // global warning — keep
        })
        .collect();

    let rows: Vec<SessionRow> = sessions
        .iter()
        .rev()
        .take(limit)
        .map(|s| {
            let started_ms = s.commands.first().map(|c| c.started.timestamp_ms);
            let ended_ms = s
                .commands
                .last()
                .and_then(|c| c.completed.as_ref().map(|cc| cc.timestamp_ms));
            // `checked_sub` returns None on reversed clocks — a subtle bug
            // the POC hid with `(en - st) as u64` that wrapped to a giant value.
            let duration_ms = match (started_ms, ended_ms) {
                (Some(st), Some(en)) => en.checked_sub(st).map(|d| d.as_millis()),
                _ => None,
            };
            let fail_count = s
                .commands
                .iter()
                .filter(|c| c.success() == Some(false))
                .count();
            let output_bytes: u64 = s
                .commands
                .iter()
                .filter_map(|c| c.completed.as_ref().map(|cc| cc.output_bytes))
                .sum();
            let token_estimate: u64 = s
                .commands
                .iter()
                .filter_map(|c| c.completed.as_ref().map(|cc| cc.token_estimate))
                .sum();
            let agent = s.commands.iter().find_map(|c| {
                c.started
                    .agent_info
                    .as_ref()
                    .map(|ai| format!("{} {}", ai.name, ai.version))
            });
            SessionRow {
                session_id: s.session_id.clone(),
                started_ms,
                duration_ms,
                command_count: s.commands.len(),
                fail_count,
                output_bytes,
                token_estimate,
                agent,
            }
        })
        .collect();
    SessionsTable {
        rows,
        total_count,
        limit,
        warnings: scoped_warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::{CommandRecord, SessionAggregate};
    use o8v_core::events::{CommandCompleted, CommandStarted};
    use o8v_core::types::{SessionId, TimestampMs, Warning};

    /// Build a minimal `CommandStarted` with the given session_id string.
    fn make_started(session_id: &str) -> CommandStarted {
        let mut s = CommandStarted::new(
            format!("run_{session_id}"),
            o8v_core::caller::Caller::Cli,
            "read",
            vec![],
            None,
        );
        // Override the process-lifetime session_id with the test value.
        s.session_id = SessionId::from_raw_unchecked(session_id.to_string());
        s.timestamp_ms = TimestampMs::from_millis(1000);
        s
    }

    fn make_completed(session_id: &str) -> CommandCompleted {
        let mut c = CommandCompleted::new(format!("run_{session_id}"), 100, 10, true);
        c.session_id = SessionId::from_raw_unchecked(session_id.to_string());
        c.timestamp_ms = TimestampMs::from_millis(1010);
        c
    }

    fn make_session(session_id: &str) -> SessionAggregate {
        let record = CommandRecord {
            started: make_started(session_id),
            completed: Some(make_completed(session_id)),
            argv_shape: String::new(),
        };
        SessionAggregate {
            session_id: session_id.to_string(),
            commands: vec![record],
            retry_clusters: vec![],
            failure_clusters: vec![],
        }
    }

    /// F3 regression: warnings scoped to an excluded session must NOT appear
    /// in the rendered output when `limit` hides that session.
    ///
    /// Pre-fix behaviour: `build_sessions_table` passed ALL warnings through,
    /// so the test below would assert `is_empty()` but find the excluded warning
    /// still present — causing a test failure.  Post-fix the warning is dropped.
    #[test]
    fn f3_warnings_from_excluded_sessions_are_dropped() {
        // Three sessions: oldest = "ses_old", rendered = "ses_a", "ses_b".
        let sessions = vec![
            make_session("ses_old"),
            make_session("ses_a"),
            make_session("ses_b"),
        ];

        // A warning scoped to the *excluded* oldest session.
        let excluded_warning = Warning::NormalizerBasenameFallback {
            session: SessionId::from_raw_unchecked("ses_old".to_string()),
            path: "./excluded.rs".to_string(),
        };
        // A warning scoped to a *rendered* session — must survive.
        let included_warning = Warning::NormalizerBasenameFallback {
            session: SessionId::from_raw_unchecked("ses_a".to_string()),
            path: "./included.rs".to_string(),
        };
        // A global warning (no session) — must always survive.
        let global_warning = Warning::FutureSince {
            since_ms: 9999,
            now_ms: 1,
        };

        let table = build_sessions_table(
            &sessions,
            2, // limit: only ses_a + ses_b are rendered
            vec![excluded_warning, included_warning, global_warning],
        );

        // Exactly 2 rows rendered (ses_a, ses_b — most-recent first).
        assert_eq!(table.rows.len(), 2, "expected 2 rendered rows");

        // The excluded warning must be absent.
        let has_excluded = table.warnings.iter().any(|w| {
            matches!(
                w,
                Warning::NormalizerBasenameFallback { session, .. }
                    if session.as_str() == "ses_old"
            )
        });
        assert!(
            !has_excluded,
            "warning from excluded session ses_old must be dropped; warnings: {:?}",
            table.warnings
        );

        // The included session-scoped warning must be present.
        let has_included = table.warnings.iter().any(|w| {
            matches!(
                w,
                Warning::NormalizerBasenameFallback { session, .. }
                    if session.as_str() == "ses_a"
            )
        });
        assert!(
            has_included,
            "warning from rendered session ses_a must be kept; warnings: {:?}",
            table.warnings
        );

        // The global warning must be present.
        let has_global = table
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::FutureSince { .. }));
        assert!(
            has_global,
            "global warning (FutureSince) must always be kept; warnings: {:?}",
            table.warnings
        );
    }
}
