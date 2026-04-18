// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Per-session aggregation data model.

use o8v_core::events::{CommandCompleted, CommandStarted};
use o8v_core::types::TimestampMs;
use std::collections::HashMap;

use crate::aggregator::sliding_window::percentile_from_sorted;

// ─── Data model ──────────────────────────────────────────────────────────────

/// A joined record of one command invocation.
///
/// `completed` is `None` when the matching `CommandCompleted` was never
/// written (process crash, timeout, etc.) — the "incomplete" bucket.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub started: CommandStarted,
    /// `None` → orphan / incomplete.
    pub completed: Option<CommandCompleted>,
    /// Normalized argv shape (§6.1).
    pub argv_shape: String,
}

impl CommandRecord {
    pub fn is_complete(&self) -> bool {
        self.completed.is_some()
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.completed.as_ref().map(|c| c.duration_ms)
    }

    pub fn success(&self) -> Option<bool> {
        self.completed.as_ref().map(|c| c.success)
    }
}

/// A retry cluster: the same `(command, argv_shape)` appeared ≥ 2 times
/// within `retry_window_ms`, potentially across different runs.
#[derive(Debug, Clone)]
pub struct RetryCluster {
    pub command: String,
    pub argv_shape: String,
    /// `run_id`s participating in this cluster.
    pub run_ids: Vec<String>,
    /// First occurrence timestamp (ms).
    pub first_ms: TimestampMs,
    /// Last occurrence timestamp (ms).
    pub last_ms: TimestampMs,
}

/// A failure cluster: same `(command, argv_shape)` with `success=false` ≥ 2
/// times within `retry_window_ms`.
#[derive(Debug, Clone)]
pub struct FailureCluster {
    pub command: String,
    pub argv_shape: String,
    /// The project path from the first event in the cluster.
    pub project_path: Option<String>,
    /// `run_id`s that failed.
    pub run_ids: Vec<String>,
    /// Timestamps of failures (ms).
    pub timestamps_ms: Vec<TimestampMs>,
}

/// All aggregated data for one session.
#[derive(Debug, Clone)]
pub struct SessionAggregate {
    /// The session ID — a validated ULID string (`ses_<26 chars>`).
    pub session_id: String,
    /// Commands in chronological order (by `CommandStarted.timestamp_ms`).
    pub commands: Vec<CommandRecord>,
    /// Retry clusters detected within this session.
    pub retry_clusters: Vec<RetryCluster>,
    /// Failure clusters detected within this session.
    pub failure_clusters: Vec<FailureCluster>,
}

impl SessionAggregate {
    pub fn has_failures(&self) -> bool {
        self.commands.iter().any(|c| c.success() == Some(false))
    }

    pub fn has_retries(&self) -> bool {
        !self.retry_clusters.is_empty()
    }

    pub fn incomplete_count(&self) -> usize {
        self.commands
            .iter()
            .filter(|c| c.completed.is_none())
            .count()
    }

    /// Minimum samples required for a percentile to be statistically meaningful.
    /// Shared with the stats histogram so every caller sees the same threshold.
    pub const MIN_PERCENTILE_SAMPLES: usize = 5;

    /// Session-level duration percentile via sort-and-index.
    ///
    /// `p` is a whole percentile in `[1, 99]`. Returns `None` when the sample
    /// count is below `MIN_PERCENTILE_SAMPLES`.
    pub fn duration_percentile(&self, p: u32) -> Option<u64> {
        let mut durations: Vec<u64> = self
            .commands
            .iter()
            .filter_map(|c| c.duration_ms())
            .collect();
        if durations.len() < Self::MIN_PERCENTILE_SAMPLES {
            return None;
        }
        percentile_from_sorted(&mut durations, p)
    }

    /// Per-command p95, sorted by command name. Commands with fewer than
    /// `MIN_PERCENTILE_SAMPLES` samples are skipped.
    pub fn per_command_p95(&self) -> Vec<(String, u64)> {
        let mut by_cmd: HashMap<&str, Vec<u64>> = HashMap::new();
        for cmd in &self.commands {
            if let Some(d) = cmd.duration_ms() {
                by_cmd.entry(&cmd.started.command).or_default().push(d);
            }
        }
        let mut out: Vec<(String, u64)> = by_cmd
            .into_iter()
            .filter_map(|(cmd, mut durs)| {
                if durs.len() < Self::MIN_PERCENTILE_SAMPLES {
                    return None;
                }
                percentile_from_sorted(&mut durs, 95).map(|p| (cmd.to_string(), p))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Top commands by invocation count, highest first, ties broken by name.
    pub fn top_commands(&self, limit: usize) -> Vec<(String, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for cmd in &self.commands {
            *counts.entry(&cmd.started.command).or_default() += 1;
        }
        let mut out: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(c, n)| (c.to_string(), n))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.truncate(limit);
        out
    }
}
