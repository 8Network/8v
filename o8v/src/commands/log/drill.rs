// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Builds the `DrillReport` for `8v log last` / `8v log show <id>`.

use crate::aggregator::SessionAggregate;
use o8v_core::render::log_report::{
    ClusterEntry, ClusterKind, DrillReport, PerCommandP95, TopCommand,
};

pub fn build_drill_report(
    session: &SessionAggregate,
    warnings: Vec<o8v_core::types::Warning>,
    retry_window_ms: u64,
) -> DrillReport {
    let started_ms = session.commands.first().map(|c| c.started.timestamp_ms);
    let ended_ms = session
        .commands
        .last()
        .and_then(|c| c.completed.as_ref().map(|cc| cc.timestamp_ms));

    let caller = session
        .commands
        .first()
        .map(|c| format!("{:?}", c.started.caller).to_lowercase())
        .unwrap_or_default();

    let agent = session.commands.iter().find_map(|c| {
        c.started
            .agent_info
            .as_ref()
            .map(|ai| format!("{} {}", ai.name, ai.version))
    });

    let project_path = session
        .commands
        .iter()
        .find_map(|c| c.started.project_path.clone());

    let command_count = session.commands.len();
    let ok_count = session
        .commands
        .iter()
        .filter(|c| c.success() == Some(true))
        .count();
    let fail_count = session
        .commands
        .iter()
        .filter(|c| c.success() == Some(false))
        .count();
    let incomplete_count = session.incomplete_count();

    let output_bytes_total: u64 = session
        .commands
        .iter()
        .filter_map(|c| c.completed.as_ref().map(|cc| cc.output_bytes))
        .sum();

    // Aggregator owns percentile + top-command computation (Layer 2 consolidation).
    let p50_ms = session.duration_percentile(50);
    let p95_ms = session.duration_percentile(95);
    let per_command_p95: Vec<PerCommandP95> = session
        .per_command_p95()
        .into_iter()
        .map(|(command, p95_ms)| PerCommandP95 { command, p95_ms })
        .collect();
    let top_commands: Vec<TopCommand> = session
        .top_commands(5)
        .into_iter()
        .map(|(command, count)| TopCommand { command, count })
        .collect();

    // Clusters — retry first, then failure.
    let mut clusters: Vec<ClusterEntry> = Vec::new();
    for rc in &session.retry_clusters {
        clusters.push(ClusterEntry {
            kind: ClusterKind::Retry,
            command: rc.command.clone(),
            argv_shape: rc.argv_shape.clone(),
            count: rc.run_ids.len(),
            path_hint: None,
        });
    }
    for fc in &session.failure_clusters {
        clusters.push(ClusterEntry {
            kind: ClusterKind::Failure,
            command: fc.command.clone(),
            argv_shape: fc.argv_shape.clone(),
            count: fc.run_ids.len(),
            path_hint: fc.project_path.clone(),
        });
    }

    DrillReport {
        session_id: session.session_id.clone(),
        caller,
        agent,
        started_ms,
        ended_ms,
        project_path,
        command_count,
        ok_count,
        fail_count,
        incomplete_count,
        p50_ms,
        p95_ms,
        output_bytes_total,
        per_command_p95,
        top_commands,
        clusters,
        warnings,
        retry_window_ms,
    }
}
