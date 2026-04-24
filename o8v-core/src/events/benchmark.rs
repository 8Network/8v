// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark lifecycle events — emitted by the benchmark pipeline to
//! `~/.8v/events.ndjson` so benchmark runs are correlatable with the
//! normal user event stream.

use crate::types::TimestampMs;
use serde::{Deserialize, Serialize};

/// The 8v version string embedded in every event.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Emitted immediately before the agent is launched for one benchmark arm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunStarted {
    /// Event kind discriminator — always `"BenchmarkRunStarted"`.
    pub event: String,
    /// UUID scoped to this single benchmark arm run.
    pub run_id: String,
    /// Unix milliseconds when this event was emitted.
    pub timestamp_ms: TimestampMs,
    /// 8v version that produced this event.
    pub version: String,
    /// Benchmark scenario name (e.g. `"fix-go/8v"`).
    pub scenario: String,
    /// Task name (e.g. `"fix-go"`).
    pub task_name: String,
    /// Arm under test — `"8v"` or `"baseline"`.
    pub arm: String,
    /// Zero-based index within the repeat loop.
    pub run_idx: u32,
    /// Unix milliseconds when the overall scenario started (before setup).
    pub started_at_ms: i64,
    /// Full provenance snapshot — serialized as a nested object.
    pub provenance: serde_json::Value,
}

impl BenchmarkRunStarted {
    pub fn new(
        run_id: impl Into<String>,
        scenario: impl Into<String>,
        task_name: impl Into<String>,
        arm: impl Into<String>,
        run_idx: u32,
        started_at_ms: i64,
        provenance: serde_json::Value,
    ) -> Self {
        Self {
            event: "BenchmarkRunStarted".to_string(),
            run_id: run_id.into(),
            timestamp_ms: TimestampMs::now(),
            version: VERSION.to_string(),
            scenario: scenario.into(),
            task_name: task_name.into(),
            arm: arm.into(),
            run_idx,
            started_at_ms,
            provenance,
        }
    }
}

/// Emitted after the agent finishes and the Observation record is built,
/// before the record is persisted to `BenchmarkStore`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunFinished {
    /// Event kind discriminator — always `"BenchmarkRunFinished"`.
    pub event: String,
    /// Matches the [`BenchmarkRunStarted::run_id`] for this arm.
    pub run_id: String,
    /// Unix milliseconds when this event was emitted.
    pub timestamp_ms: TimestampMs,
    /// 8v version that produced this event.
    pub version: String,
    /// Wall-clock duration of the agent run in milliseconds.
    pub duration_ms: u64,
    /// Agent process exit code (0 = clean exit; non-zero = error/timeout).
    pub exit_code: i64,
    /// Whether the post-run verification step passed.
    pub tests_pass: bool,
    /// Estimated cost in USD as reported by the agent.
    pub cost_usd: f64,
    /// Total tokens consumed (input + output, including cache hits).
    pub total_tokens: u64,
    /// Tokens written to the prompt cache in this turn.
    pub cache_creation_input_tokens: u64,
    /// Tokens read from the prompt cache in this turn.
    pub cache_read_input_tokens: u64,
    /// Number of tool calls made by the agent.
    pub tool_calls: u32,
    /// Number of conversation turns.
    pub turns: u32,
}

impl BenchmarkRunFinished {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        duration_ms: u64,
        exit_code: i64,
        tests_pass: bool,
        cost_usd: f64,
        total_tokens: u64,
        cache_creation_input_tokens: u64,
        cache_read_input_tokens: u64,
        tool_calls: u32,
        turns: u32,
    ) -> Self {
        Self {
            event: "BenchmarkRunFinished".to_string(),
            run_id: run_id.into(),
            timestamp_ms: TimestampMs::now(),
            version: VERSION.to_string(),
            duration_ms,
            exit_code,
            tests_pass,
            cost_usd,
            total_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            tool_calls,
            turns,
        }
    }
}
