// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Builds `SearchResults` for `8v log search <query>`.

use crate::aggregator::SessionAggregate;
use o8v_core::caller::Caller;
use o8v_core::render::log_report::{SearchResultRow, SearchResults};
use std::collections::HashSet;

pub fn build_search_results(
    sessions: &[SessionAggregate],
    query: &str,
    limit: usize,
    // SearchResults has no warnings field — accepted but discarded.
    _warnings: Vec<o8v_core::types::Warning>,
) -> SearchResults {
    let query_lower = query.to_lowercase();
    let mut rows: Vec<SearchResultRow> = Vec::new();
    let mut matched_sessions: HashSet<&str> = HashSet::new();

    for session in sessions {
        for cmd in &session.commands {
            if cmd.started.command.to_lowercase().contains(&query_lower)
                || cmd.argv_shape.to_lowercase().contains(&query_lower)
            {
                matched_sessions.insert(&session.session_id);
                rows.push(SearchResultRow {
                    session_id: session.session_id.clone(),
                    timestamp_ms: cmd.started.timestamp_ms,
                    command: cmd.started.command.clone(),
                    argv_shape: cmd.argv_shape.clone(),
                    success: cmd.success(),
                });
            }
        }
    }

    let total_matches = rows.len();
    let session_count = matched_sessions.len();
    let has_hook_events = sessions
        .iter()
        .any(|s| s.commands.iter().any(|c| c.started.caller == Caller::Hook));

    let rows = if limit > 0 {
        rows.into_iter().take(limit).collect()
    } else {
        rows
    };

    SearchResults {
        query: query.to_string(),
        rows,
        session_count,
        total_matches,
        has_hook_events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::{CommandRecord, SessionAggregate};
    use o8v_core::caller::Caller;
    use o8v_core::events::CommandStarted;

    fn make_session(session_id: &str, commands: &[&str]) -> SessionAggregate {
        SessionAggregate {
            session_id: session_id.to_string(),
            commands: commands
                .iter()
                .enumerate()
                .map(|(i, &cmd)| CommandRecord {
                    started: CommandStarted::new(
                        format!("run-{i}"),
                        Caller::Cli,
                        cmd,
                        vec![],
                        None,
                    ),
                    argv_shape: cmd.to_string(),
                    completed: None,
                })
                .collect(),
            retry_clusters: vec![],
            failure_clusters: vec![],
        }
    }

    #[test]
    fn build_search_results_respects_limit() {
        let sessions = vec![
            make_session("s1", &["read", "read", "read"]),
            make_session("s2", &["read", "read"]),
        ];
        // 5 matching commands total; limit to 2
        let result = build_search_results(&sessions, "read", 2, vec![]);
        assert_eq!(
            result.rows.len(),
            2,
            "limit=2 must truncate rows to 2, got {}",
            result.rows.len()
        );
        // total_matches should still reflect the untruncated count
        assert_eq!(result.total_matches, 5);
    }
}
