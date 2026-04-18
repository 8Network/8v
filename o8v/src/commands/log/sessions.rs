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
        warnings,
    }
}
