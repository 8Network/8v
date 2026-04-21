// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Report schema types — serializable structs that define the JSON shape.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::benchmark::profiles::{default_profile_version, ToolProfile};

// ── Report schema types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportJson {
    pub schema_version: u32,
    pub experiment: String,
    pub commit: String,
    pub version_8v: Option<String>,
    pub started_ms: i64,
    pub finished_ms: i64,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub model_id: Option<String>,
    pub mcp_protocol_version: Option<String>,
    pub task: TaskInfo,
    pub conditions: Vec<ConditionReport>,
    pub deltas_vs_control: Vec<DeltaReport>,
    pub confidence: Confidence,
    pub runs: Vec<RunRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskInfo {
    pub name: String,
    pub task_name: Option<String>,
    pub prompt_sha: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatBlock {
    pub mean: f64,
    pub stddev: f64,
    pub cv: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub input: StatBlock,
    pub output: StatBlock,
    pub cache_read: StatBlock,
    pub cache_creation: StatBlock,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub tests_pass: GateCount,
    pub build_pass: GateCount,
    pub check_pass: GateCount,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GateCount {
    pub passed: usize,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LandmineReport {
    pub stuck_loop_runs: usize,
    pub is_error_storms: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConditionReport {
    pub name: String,
    pub description: String,
    pub n: usize,
    pub tokens: StatBlock,
    pub cost_usd: StatBlock,
    pub tokens_by_category: TokenBreakdown,
    pub turns: StatBlock,
    pub tool_calls: StatBlock,
    pub tools_histogram: BTreeMap<String, f64>,
    pub verification: VerificationSummary,
    pub landmines: LandmineReport,
    /// Schema tax: bytes in the initial system message, which carries every
    /// registered MCP tool's JSON schema. Baseline (no MCP) shows the bare
    /// cost; 8v conditions show the tax the 8v MCP server adds. Reported
    /// per-condition so the cost is visible and compared alongside savings.
    pub schema_tax_init_bytes: StatBlock,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaReport {
    pub condition: String,
    pub cost_delta_ratio: Option<f64>,
    pub tokens_delta_ratio: f64,
    pub turns_delta_ratio: f64,
    pub calls_delta_ratio: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Confidence {
    pub n_per_condition: usize,
    pub publishable: bool,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_index: usize,
    pub condition: String,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub turns: u32,
    pub tool_calls: usize,
    pub tests_pass: Option<bool>,
    pub build_pass: Option<bool>,
    pub check_pass: Option<bool>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    /// Full tool call sequence — name, input args, output size, error flag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls_detail: Vec<crate::benchmark::types::ToolCallDetail>,
    #[serde(default)]
    pub profile: ToolProfile,
    #[serde(default = "default_profile_version")]
    pub profile_version: String,
}
