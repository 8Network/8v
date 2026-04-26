// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Retry and failure cluster detection over command records.

use std::collections::HashMap;

use o8v_core::types::DurationMs;

use super::{sliding_windows, CommandRecord, FailureCluster, RetryCluster};

/// Group commands by `(command, argv_shape)`, apply `filter` to each record,
/// then emit sliding windows of ≥ `min_window` records within `window_ms` via
/// `build`. Returns a flat list of whatever `build` produces.
///
/// Shared kernel for [`detect_retry_clusters`] and [`detect_failure_clusters`].
pub(crate) fn detect_clusters<T>(
    commands: &[CommandRecord],
    retry_window_ms: u64,
    min_window: usize,
    filter: impl Fn(&CommandRecord) -> bool,
    build: impl Fn(&str, &str, &[&CommandRecord]) -> T,
) -> Vec<T> {
    let mut groups: HashMap<(String, String), Vec<&CommandRecord>> = HashMap::new();
    for cmd in commands {
        if filter(cmd) {
            groups
                .entry((cmd.started.command.clone(), cmd.argv_shape.clone()))
                .or_default()
                .push(cmd);
        }
    }

    let window = DurationMs::from_millis(retry_window_ms);
    let mut clusters = Vec::new();
    for ((command, argv_shape), records) in groups {
        if records.len() < min_window {
            continue;
        }
        for run in sliding_windows(&records, window, |r| r.started.timestamp_ms) {
            clusters.push(build(&command, &argv_shape, run));
        }
    }
    clusters
}

/// Detect retry clusters within a session.
///
/// A retry cluster is `(command, argv_shape)` with ≥ 2 occurrences where
/// the span from first to last `CommandStarted.timestamp_ms` ≤ `retry_window_ms`.
/// Interleaved events are allowed (§6 of log design: §6 rule).
pub(crate) fn detect_retry_clusters(
    commands: &[CommandRecord],
    retry_window_ms: u64,
) -> Vec<RetryCluster> {
    detect_clusters(
        commands,
        retry_window_ms,
        2,
        |_| true,
        |command, argv_shape, run| RetryCluster {
            command: command.to_string(),
            argv_shape: argv_shape.to_string(),
            run_ids: run.iter().map(|r| r.started.run_id.clone()).collect(),
            first_ms: run.first().expect("non-empty").started.timestamp_ms,
            last_ms: run.last().expect("non-empty").started.timestamp_ms,
        },
    )
}

/// Detect failure clusters within a session.
///
/// A failure cluster is `(command, argv_shape)` with `success=false` ≥ 2 times
/// within `retry_window_ms`.
pub(crate) fn detect_failure_clusters(
    commands: &[CommandRecord],
    retry_window_ms: u64,
) -> Vec<FailureCluster> {
    detect_clusters(
        commands,
        retry_window_ms,
        2,
        |cmd| cmd.success() == Some(false),
        |command, argv_shape, run| {
            let project_path = run.first().and_then(|r| r.started.project_path.clone());
            let run_ids = run.iter().map(|r| r.started.run_id.clone()).collect();
            let timestamps_ms = run.iter().map(|r| r.started.timestamp_ms).collect();
            FailureCluster {
                command: command.to_string(),
                argv_shape: argv_shape.to_string(),
                project_path,
                run_ids,
                timestamps_ms,
            }
        },
    )
}
