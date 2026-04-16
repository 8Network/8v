// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Structured benchmark report — JSON builder.
//!
//! Pure function: `ExperimentResult → ReportJson`. No IO.
//! See docs/design/structured-benchmark-report.md.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::types::{ExperimentResult, Observation, Sample};

// ── Report schema types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportJson {
    pub schema_version: u32,
    pub experiment: String,
    pub commit: String,
    pub version_8v: String,
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
    pub fixture: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaReport {
    pub condition: String,
    pub cost_delta_pct: Option<f64>,
    pub tokens_delta_pct: f64,
    pub turns_delta_pct: f64,
    pub calls_delta_pct: f64,
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
}

// ── Builder ─────────────────────────────────────────────────────────────────

pub fn build_report(result: &ExperimentResult) -> ReportJson {
    let first_obs = result.control.observations.first();
    let agent_name = first_obs.and_then(|o| o.agent_name.clone());
    let agent_version = first_obs.and_then(|o| o.agent_version.clone());
    let model_id = first_obs.and_then(|o| o.model.clone());
    let mcp_protocol_version = first_obs.and_then(|o| o.mcp_protocol_version.clone());
    let version_8v = first_obs.map(|o| o.version.clone()).unwrap_or_default();

    let prompt_text = result.task.clone();
    let prompt_sha = sha256_hex(&prompt_text);

    let fixture = result.control.observations.first()
        .map(|o| o.task_name.clone())
        .unwrap_or_default();

    let mut conditions = Vec::new();
    let mut runs = Vec::new();

    let control_condition = build_condition(&result.control, &mut runs);
    conditions.push(control_condition);

    for treatment in &result.treatments {
        let condition = build_condition(treatment, &mut runs);
        conditions.push(condition);
    }

    let deltas_vs_control = build_deltas(&result.control, &result.treatments);

    let cv = if !conditions.is_empty() {
        conditions.iter().map(|c| c.cost_usd.cv).fold(0.0_f64, f64::max)
    } else {
        0.0
    };
    let (publishable, reason) = assess_confidence(result.n, cv);

    ReportJson {
        schema_version: 1,
        experiment: result.name.clone(),
        commit: result.git_commit.clone(),
        version_8v,
        started_ms: result.timestamp_ms,
        finished_ms: now_ms(),
        agent_name,
        agent_version,
        model_id,
        mcp_protocol_version,
        task: TaskInfo {
            name: result.task.clone(),
            fixture,
            prompt_sha,
        },
        conditions,
        deltas_vs_control,
        confidence: Confidence {
            n_per_condition: result.n,
            publishable,
            reason,
        },
        runs,
    }
}

fn build_condition(sample: &Sample, runs: &mut Vec<RunRecord>) -> ConditionReport {
    let n = sample.n();
    let start_idx = runs.len();

    for (i, obs) in sample.observations.iter().enumerate() {
        runs.push(RunRecord {
            run_index: start_idx + i,
            condition: sample.description.clone(),
            tokens: obs.total_tokens,
            cost_usd: obs.cost_usd,
            turns: obs.turn_count,
            tool_calls: obs.tool_names.len(),
            tests_pass: obs.verification.tests_pass,
            build_pass: obs.verification.build_pass,
            check_pass: obs.verification.check_pass,
            input_tokens: obs.input_tokens,
            output_tokens: obs.output_tokens,
            cache_read_tokens: obs.cache_read_tokens,
            cache_creation_tokens: obs.cache_creation_tokens,
        });
    }

    let tokens = stat_block(sample, |o| o.total_tokens as f64);
    let cost_usd = stat_block(sample, |o| o.cost_usd.unwrap_or(0.0));
    let turns = stat_block(sample, |o| o.turn_count as f64);
    let tool_calls = stat_block(sample, |o| o.tool_names.len() as f64);

    let tokens_by_category = TokenBreakdown {
        input: stat_block(sample, |o| o.input_tokens as f64),
        output: stat_block(sample, |o| o.output_tokens as f64),
        cache_read: stat_block(sample, |o| o.cache_read_tokens as f64),
        cache_creation: stat_block(sample, |o| o.cache_creation_tokens as f64),
    };

    let tools_histogram = build_tools_histogram(sample);

    let verification = VerificationSummary {
        tests_pass: GateCount {
            passed: sample.tests_pass_count(),
            total: n,
        },
        build_pass: GateCount {
            passed: sample.build_pass_count(),
            total: n,
        },
        check_pass: GateCount {
            passed: sample.check_pass_count(),
            total: n,
        },
    };

    let landmines = detect_landmines(sample);

    ConditionReport {
        name: sample.scenario.clone(),
        description: sample.description.clone(),
        n,
        tokens,
        cost_usd,
        tokens_by_category,
        turns,
        tool_calls,
        tools_histogram,
        verification,
        landmines,
    }
}

fn stat_block(sample: &Sample, f: impl Fn(&Observation) -> f64 + Copy) -> StatBlock {
    let mean = sample.mean(f);
    let stddev = sample.stddev(f);
    let cv = if mean.abs() > f64::EPSILON { stddev / mean } else { 0.0 };
    let values: Vec<f64> = sample.observations.iter().map(f).collect();
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    StatBlock {
        mean,
        stddev,
        cv,
        min: if min.is_infinite() { 0.0 } else { min },
        max: if max.is_infinite() { 0.0 } else { max },
    }
}

fn build_tools_histogram(sample: &Sample) -> BTreeMap<String, f64> {
    let n = sample.observations.len();
    if n == 0 {
        return BTreeMap::new();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for obs in &sample.observations {
        for name in &obs.tool_names {
            *counts.entry(name.clone()).or_default() += 1;
        }
    }
    counts.into_iter()
        .map(|(name, count)| (name, count as f64 / n as f64))
        .collect()
}

fn detect_landmines(sample: &Sample) -> LandmineReport {
    let mut stuck_loop_runs = 0;
    let mut is_error_storms = 0;

    for obs in &sample.observations {
        if has_stuck_loop(&obs.tool_calls_detail) {
            stuck_loop_runs += 1;
        }
        if has_error_storm(&obs.tool_calls_detail) {
            is_error_storms += 1;
        }
    }

    LandmineReport {
        stuck_loop_runs,
        is_error_storms,
    }
}

fn has_stuck_loop(details: &[super::types::ToolCallDetail]) -> bool {
    if details.len() < 3 {
        return false;
    }
    for window in details.windows(3) {
        let all_same = window[0].name == window[1].name
            && window[1].name == window[2].name
            && window[0].input == window[1].input
            && window[1].input == window[2].input;
        if all_same {
            return true;
        }
    }
    false
}

fn has_error_storm(details: &[super::types::ToolCallDetail]) -> bool {
    let mut consecutive_errors = 0;
    for d in details {
        if d.is_error {
            consecutive_errors += 1;
            if consecutive_errors >= 5 {
                return true;
            }
        } else {
            consecutive_errors = 0;
        }
    }
    false
}

fn build_deltas(control: &Sample, treatments: &[Sample]) -> Vec<DeltaReport> {
    let ctrl_tokens = control.mean(|o| o.total_tokens as f64);
    let ctrl_cost = control.mean(|o| o.cost_usd.unwrap_or(0.0));
    let ctrl_turns = control.mean(|o| o.turn_count as f64);
    let ctrl_calls = control.mean(|o| o.tool_names.len() as f64);

    treatments.iter().map(|t| {
        let t_tokens = t.mean(|o| o.total_tokens as f64);
        let t_cost = t.mean(|o| o.cost_usd.unwrap_or(0.0));
        let t_turns = t.mean(|o| o.turn_count as f64);
        let t_calls = t.mean(|o| o.tool_names.len() as f64);

        DeltaReport {
            condition: t.description.clone(),
            tokens_delta_pct: pct_delta(ctrl_tokens, t_tokens),
            cost_delta_pct: if ctrl_cost > 0.0 { Some(pct_delta(ctrl_cost, t_cost)) } else { None },
            turns_delta_pct: pct_delta(ctrl_turns, t_turns),
            calls_delta_pct: pct_delta(ctrl_calls, t_calls),
        }
    }).collect()
}

fn pct_delta(control: f64, treatment: f64) -> f64 {
    if control.abs() < f64::EPSILON {
        return 0.0;
    }
    (treatment - control) / control
}

fn assess_confidence(n: usize, max_cv: f64) -> (bool, String) {
    if n < 3 {
        return (false, format!("N={n} — minimum 3 required"));
    }
    // At 95% CI, half-width = t * stddev / sqrt(n). For the headline delta to be
    // meaningful, the CI half-width must be < |delta|/2. With CV=max_cv and N=n:
    // half-width ≈ 1.96 * CV * mean / sqrt(n) = 1.96 * CV / sqrt(n) of the mean.
    let half_width_pct = 1.96 * max_cv / (n as f64).sqrt();
    if half_width_pct > 0.15 {
        return (false, format!(
            "95% CI half-width {:.1}% exceeds 15% — need more runs or lower variance (N={n}, max CV={:.1}%)",
            half_width_pct * 100.0, max_cv * 100.0,
        ));
    }
    (true, format!("N={n}, max CV={:.1}%, 95% CI half-width {:.1}%", max_cv * 100.0, half_width_pct * 100.0))
}

fn sha256_hex(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::types::*;

    fn make_observation(tokens: u64, cost: f64, turns: u32, tools: Vec<&str>) -> Observation {
        Observation {
            scenario: "test".into(),
            task_name: "test-task".into(),
            timestamp_ms: 1000,
            git_commit: "abc".into(),
            version: "0.1.0".into(),
            total_tokens: tokens,
            cost_usd: Some(cost),
            exit_code: 0,
            tool_names: tools.into_iter().map(String::from).collect(),
            turns: vec![],
            init_message_bytes: 0,
            response_text: String::new(),
            model: Some("test-model".into()),
            session_id: None,
            stop_reason: None,
            is_error: false,
            cache_read_tokens: tokens / 2,
            cache_creation_tokens: tokens / 4,
            input_tokens: 10,
            output_tokens: tokens / 8,
            turn_count: turns,
            event_count: 0,
            event_output_bytes: 0,
            event_command_bytes: 0,
            event_total_duration_ms: 0,
            agent_name: Some("claude-code".into()),
            agent_version: Some("1.0.0".into()),
            mcp_protocol_version: Some("2025-03-26".into()),
            agent_capabilities: vec![],
            verification: Verification {
                tests_pass: Some(true),
                check_pass: Some(false),
                build_pass: Some(true),
            },
            feedback: None,
            tool_calls_detail: vec![],
        }
    }

    fn make_sample(desc: &str, observations: Vec<Observation>) -> Sample {
        Sample {
            scenario: desc.into(),
            description: desc.into(),
            observations,
        }
    }

    fn make_result() -> ExperimentResult {
        let control = make_sample("Native", vec![
            make_observation(100000, 0.20, 20, vec!["Bash", "Read", "Read"]),
            make_observation(120000, 0.25, 24, vec!["Bash", "Read", "Glob"]),
            make_observation(110000, 0.22, 22, vec!["Bash", "Read", "Read"]),
        ]);
        let treatment = make_sample("With 8v", vec![
            make_observation(60000, 0.12, 12, vec!["mcp__8v__8v", "mcp__8v__8v"]),
            make_observation(70000, 0.14, 14, vec!["mcp__8v__8v", "mcp__8v__8v"]),
            make_observation(65000, 0.13, 13, vec!["mcp__8v__8v", "mcp__8v__8v"]),
        ]);

        ExperimentResult {
            name: "test-experiment".into(),
            task: "test-task".into(),
            git_commit: "abc123".into(),
            timestamp_ms: 1000,
            n: 3,
            control,
            treatments: vec![treatment],
            effects: vec![],
        }
    }

    #[test]
    fn build_report_has_correct_schema_version() {
        let result = make_result();
        let report = build_report(&result);
        assert_eq!(report.schema_version, 1);
    }

    #[test]
    fn build_report_extracts_agent_identity() {
        let result = make_result();
        let report = build_report(&result);
        assert_eq!(report.agent_name.as_deref(), Some("claude-code"));
        assert_eq!(report.agent_version.as_deref(), Some("1.0.0"));
        assert_eq!(report.model_id.as_deref(), Some("test-model"));
        assert_eq!(report.mcp_protocol_version.as_deref(), Some("2025-03-26"));
    }

    #[test]
    fn build_report_has_two_conditions() {
        let result = make_result();
        let report = build_report(&result);
        assert_eq!(report.conditions.len(), 2);
        assert_eq!(report.conditions[0].description, "Native");
        assert_eq!(report.conditions[1].description, "With 8v");
    }

    #[test]
    fn build_report_computes_means() {
        let result = make_result();
        let report = build_report(&result);
        let control = &report.conditions[0];
        let expected_mean = (100000.0 + 120000.0 + 110000.0) / 3.0;
        assert!((control.tokens.mean - expected_mean).abs() < 1.0);
    }

    #[test]
    fn build_report_computes_cost_delta() {
        let result = make_result();
        let report = build_report(&result);
        assert_eq!(report.deltas_vs_control.len(), 1);
        let delta = &report.deltas_vs_control[0];
        let ctrl_cost = (0.20 + 0.25 + 0.22) / 3.0;
        let treat_cost = (0.12 + 0.14 + 0.13) / 3.0;
        let expected = (treat_cost - ctrl_cost) / ctrl_cost;
        assert!((delta.cost_delta_pct.unwrap() - expected).abs() < 0.001);
        assert!(delta.cost_delta_pct.unwrap() < 0.0, "treatment should be cheaper");
    }

    #[test]
    fn build_report_has_per_run_records() {
        let result = make_result();
        let report = build_report(&result);
        assert_eq!(report.runs.len(), 6);
        assert_eq!(report.runs[0].condition, "Native");
        assert_eq!(report.runs[3].condition, "With 8v");
    }

    #[test]
    fn build_report_tools_histogram() {
        let result = make_result();
        let report = build_report(&result);
        let control = &report.conditions[0];
        assert!(control.tools_histogram.contains_key("Bash"));
        assert!(control.tools_histogram.contains_key("Read"));
        assert!((control.tools_histogram["Bash"] - 1.0).abs() < 0.01);
    }

    #[test]
    fn build_report_verification_counts() {
        let result = make_result();
        let report = build_report(&result);
        let control = &report.conditions[0];
        assert_eq!(control.verification.tests_pass.passed, 3);
        assert_eq!(control.verification.tests_pass.total, 3);
        assert_eq!(control.verification.check_pass.passed, 0);
        assert_eq!(control.verification.build_pass.passed, 3);
    }

    #[test]
    fn build_report_confidence_small_n() {
        let result = make_result();
        let report = build_report(&result);
        assert!(!report.confidence.publishable || report.confidence.n_per_condition >= 3);
    }

    #[test]
    fn build_report_serializes_to_json() {
        let result = make_result();
        let report = build_report(&result);
        let json = serde_json::to_string_pretty(&report).expect("serialize");
        assert!(json.contains("schema_version"));
        assert!(json.contains("conditions"));
        assert!(json.contains("deltas_vs_control"));
        assert!(json.contains("claude-code"));
    }

    #[test]
    fn stat_block_cv_zero_mean() {
        let sample = make_sample("test", vec![make_observation(0, 0.0, 0, vec![])]);
        let block = stat_block(&sample, |_| 0.0);
        assert_eq!(block.cv, 0.0);
    }

    #[test]
    fn landmine_stuck_loop_detected() {
        let mut obs = make_observation(100, 0.01, 1, vec!["Read", "Read", "Read"]);
        obs.tool_calls_detail = vec![
            ToolCallDetail { name: "Read".into(), input: "{\"file\":\"x\"}".into(), output_bytes: 10, is_error: false },
            ToolCallDetail { name: "Read".into(), input: "{\"file\":\"x\"}".into(), output_bytes: 10, is_error: false },
            ToolCallDetail { name: "Read".into(), input: "{\"file\":\"x\"}".into(), output_bytes: 10, is_error: false },
        ];
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(landmines.stuck_loop_runs, 1);
    }

    #[test]
    fn landmine_error_storm_detected() {
        let mut obs = make_observation(100, 0.01, 1, vec!["Write"; 5]);
        obs.tool_calls_detail = (0..5).map(|_| ToolCallDetail {
            name: "Write".into(), input: "{}".into(), output_bytes: 0, is_error: true,
        }).collect();
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(landmines.is_error_storms, 1);
    }

    #[test]
    fn pct_delta_negative_means_cheaper() {
        let d = pct_delta(100.0, 60.0);
        assert!((d - (-0.4)).abs() < 0.001);
    }

    #[test]
    fn pct_delta_zero_control() {
        let d = pct_delta(0.0, 50.0);
        assert_eq!(d, 0.0);
    }
}
