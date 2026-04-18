// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Builds `SearchResults` for `8v log search <query>`.

use crate::aggregator::SessionAggregate;
use o8v_core::render::log_report::{SearchResultRow, SearchResults};
use std::collections::HashSet;

pub fn build_search_results(
    sessions: &[SessionAggregate],
    query: &str,
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

    SearchResults {
        query: query.to_string(),
        rows,
        session_count,
        total_matches,
    }
}
