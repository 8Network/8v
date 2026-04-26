// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! p50/p95/p99 duration quantiles for a set of command invocations.

use serde::Serialize;

/// Latency percentiles in milliseconds.
/// Present only when `n ≥ MIN_SAMPLES_FOR_PERCENTILE` (currently 5).
#[derive(Debug, Clone, Serialize)]
pub struct DurationStats {
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
}
