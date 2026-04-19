// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! One row of stats — one observation set for a label (command, shape, or agent).

use serde::Serialize;

use super::duration_stats::DurationStats;

/// A single row in the stats table.
/// `label` holds the command name, argv_shape, or agent name depending on mode.
#[derive(Debug, Clone, Serialize)]
pub struct StatsRow {
    /// Command name, argv_shape, or agent name — depends on report mode.
    pub label: String,
    /// Total number of observations (command invocations).
    pub n: u64,
    /// Latency percentiles. None when n < MIN_SAMPLES_FOR_PERCENTILE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<DurationStats>,
    /// Arithmetic mean duration in ms. Present when n >= 1 (at least one completed record).
    /// Shown in plain text when n < MIN_SAMPLES_FOR_PERCENTILE; always emitted in JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_ms: Option<f64>,
    /// Fraction of completed commands that succeeded. None when no completed records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok_rate: Option<f64>,
    /// Average output bytes per call. None when no completed records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_bytes_per_call_mean: Option<f64>,
    /// Number of retry clusters involving this label.
    #[serde(rename = "retries")]
    pub retry_cluster_count: u64,
}
