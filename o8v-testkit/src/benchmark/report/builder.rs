// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Builder logic — pure functions that convert `ExperimentResult` → `ReportJson`.

use std::collections::BTreeMap;

use crate::benchmark::types::{ExperimentResult, Observation, Sample};

use super::types::{
    ConditionReport, Confidence, DeltaReport, GateCount, LandmineReport, ReportJson, RunRecord,
    StatBlock, TaskInfo, TokenBreakdown, VerificationSummary,
};

// ── Builder ─────────────────────────────────────────────────────────────────

pub fn build_report(result: &ExperimentResult) -> ReportJson {
    let first_obs = result.control.observations.first();
    let agent_name = first_obs.and_then(|o| o.agent_name.clone());
    let agent_version = first_obs.and_then(|o| o.agent_version.clone());
    let model_id = first_obs.and_then(|o| o.model.clone());
    let mcp_protocol_version = first_obs.and_then(|o| o.mcp_protocol_version.clone());
    let version_8v = first_obs.map(|o| o.version.clone());

    let prompt_text = result.task.clone();
    let prompt_sha = hash_hex(&prompt_text);

    let task_name = result
        .control
        .observations
        .first()
        .map(|o| o.task_name.clone());

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
        conditions
            .iter()
            .map(|c| c.cost_usd.cv)
            .fold(0.0_f64, f64::max)
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
            task_name,
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
            cache_read_tokens: obs.cache_read_input_tokens,
            cache_creation_tokens: obs.cache_creation_input_tokens,
            tool_calls_detail: obs.tool_calls_detail.clone(),
            profile: obs.profile,
            profile_version: obs.profile_version.clone(),
        });
    }

    let tokens = stat_block(sample, |o| o.total_tokens as f64);
    let cost_usd = {
        let costs: Vec<f64> = sample
            .observations
            .iter()
            .filter_map(|o| o.cost_usd)
            .collect();
        if costs.is_empty() {
            StatBlock {
                mean: 0.0,
                stddev: 0.0,
                min: 0.0,
                max: 0.0,
                cv: 0.0,
            }
        } else {
            let mean = costs.iter().sum::<f64>() / costs.len() as f64;
            let variance =
                costs.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / costs.len() as f64;
            let stddev = variance.sqrt();
            let min = costs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let cv = if mean.abs() > f64::EPSILON {
                stddev / mean
            } else {
                0.0
            };
            StatBlock {
                mean,
                stddev,
                min,
                max,
                cv,
            }
        }
    };
    let turns = stat_block(sample, |o| o.turn_count as f64);
    let tool_calls = stat_block(sample, |o| o.tool_names.len() as f64);

    let tokens_by_category = TokenBreakdown {
        input: stat_block(sample, |o| o.input_tokens as f64),
        output: stat_block(sample, |o| o.output_tokens as f64),
        cache_read: stat_block(sample, |o| o.cache_read_input_tokens as f64),
        cache_creation: stat_block(sample, |o| o.cache_creation_input_tokens as f64),
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

    let schema_tax_init_bytes = stat_block(sample, |o| o.init_message_bytes as f64);

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
        schema_tax_init_bytes,
    }
}

pub(super) fn stat_block(sample: &Sample, f: impl Fn(&Observation) -> f64 + Copy) -> StatBlock {
    let mean = sample.require_mean(f);
    let stddev = sample.stddev(f).unwrap_or(0.0);
    let cv = if mean.abs() > f64::EPSILON {
        stddev / mean
    } else {
        0.0
    };
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
    counts
        .into_iter()
        .map(|(name, count)| (name, count as f64 / n as f64))
        .collect()
}

pub(super) fn detect_landmines(sample: &Sample) -> LandmineReport {
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

fn has_stuck_loop(details: &[crate::benchmark::types::ToolCallDetail]) -> bool {
    // Strict: 3 consecutive calls with same name AND same input.
    // Catches perfect loops ("retry the exact same tool with the exact same args").
    if details.len() >= 3 {
        for window in details.windows(3) {
            let all_same = window[0].name == window[1].name
                && window[1].name == window[2].name
                && window[0].input == window[1].input
                && window[1].input == window[2].input;
            if all_same {
                return true;
            }
        }
    }

    // Loose: 5+ consecutive calls with same name (inputs may vary).
    // Catches "thrashing" — agent fixated on one tool, varying args without
    // progressing. Observed in polyglot Run 6: 100 turns, 81 tool calls,
    // massive cost outlier, not caught by strict check.
    //
    // MCP tools (name starts with "mcp__") are excluded: when 8v is active,
    // all operations route through a single MCP tool by design. Sequential
    // calls are expected and do not indicate a stuck loop.
    let mut run_len = 1usize;
    for pair in details.windows(2) {
        if pair[0].name == pair[1].name && !pair[0].name.starts_with("mcp__") {
            run_len += 1;
            if run_len >= 5 {
                return true;
            }
        } else {
            run_len = 1;
        }
    }

    false
}

fn has_error_storm(details: &[crate::benchmark::types::ToolCallDetail]) -> bool {
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
    let ctrl_tokens = control.require_mean(|o| o.total_tokens as f64);
    let ctrl_cost = {
        let costs: Vec<f64> = control
            .observations
            .iter()
            .filter_map(|o| o.cost_usd)
            .collect();
        if costs.is_empty() {
            None
        } else {
            Some(costs.iter().sum::<f64>() / costs.len() as f64)
        }
    };
    let ctrl_turns = control.require_mean(|o| o.turn_count as f64);
    let ctrl_calls = control.require_mean(|o| o.tool_names.len() as f64);

    treatments
        .iter()
        .map(|t| {
            let t_tokens = t.require_mean(|o| o.total_tokens as f64);
            let t_cost = {
                let costs: Vec<f64> = t.observations.iter().filter_map(|o| o.cost_usd).collect();
                if costs.is_empty() {
                    None
                } else {
                    Some(costs.iter().sum::<f64>() / costs.len() as f64)
                }
            };
            let t_turns = t.require_mean(|o| o.turn_count as f64);
            let t_calls = t.require_mean(|o| o.tool_names.len() as f64);

            let cost_delta_ratio = match (ctrl_cost, t_cost) {
                (Some(c), Some(t)) if c > 0.0 => Some(pct_delta(c, t)),
                _ => None,
            };

            DeltaReport {
                condition: t.description.clone(),
                tokens_delta_ratio: pct_delta(ctrl_tokens, t_tokens),
                cost_delta_ratio,
                turns_delta_ratio: pct_delta(ctrl_turns, t_turns),
                calls_delta_ratio: pct_delta(ctrl_calls, t_calls),
            }
        })
        .collect()
}

pub(super) fn pct_delta(control: f64, treatment: f64) -> f64 {
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
    (
        true,
        format!(
            "N={n}, max CV={:.1}%, 95% CI half-width {:.1}%",
            max_cv * 100.0,
            half_width_pct * 100.0
        ),
    )
}

fn hash_hex(s: &str) -> String {
    // FNV-1a: stable across Rust versions and machines (unlike DefaultHasher).
    // Produces a deterministic 64-bit hash of the prompt text, used as a
    // short fingerprint (first 8 hex chars) to identify prompt versions.
    fn fnv1a(s: &str) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for b in s.bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
    let full = format!("{:016x}", fnv1a(s));
    full[..8].to_string()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .expect("system clock")
}
