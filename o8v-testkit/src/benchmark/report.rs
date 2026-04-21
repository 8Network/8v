// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Structured benchmark report — JSON builder.
//!
//! Pure function: `ExperimentResult → ReportJson`. No IO.
//! See docs/design/structured-benchmark-report.md.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::profiles::{default_profile_version, ToolProfile};
use super::types::{ExperimentResult, Observation, Sample};

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
    pub tool_calls_detail: Vec<super::types::ToolCallDetail>,
    #[serde(default)]
    pub profile: ToolProfile,
    #[serde(default = "default_profile_version")]
    pub profile_version: String,
}

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

fn stat_block(sample: &Sample, f: impl Fn(&Observation) -> f64 + Copy) -> StatBlock {
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

// ── Markdown renderer ───────────────────────────────────────────────────────

pub fn render_markdown(report: &ReportJson) -> String {
    if report.conditions.is_empty() {
        return format!(
            "# Benchmark — {}\n\n**Agent:** {}  |  **Runs:** 0\n\nNo conditions recorded.\n",
            report.experiment,
            report.agent_name.as_deref().unwrap_or("unknown")
        );
    }

    let mut md = String::new();

    // Header
    md.push_str(&format!("# Benchmark — {}\n\n", report.experiment));
    md.push_str(&format!(
        "**Commit:** {}  |  **8v version:** {}  |  **Runs:** N={} per condition\n",
        report.commit,
        report.version_8v.as_deref().unwrap_or("unknown"),
        report.confidence.n_per_condition,
    ));
    md.push_str(&format!(
        "**Task:** {}  |  **Fixture:** {}\n",
        report.task.name,
        report.task.task_name.as_deref().unwrap_or("unknown"),
    ));
    if let Some(agent) = &report.agent_name {
        let ver = report.agent_version.as_deref().unwrap_or("?");
        let model = report.model_id.as_deref().unwrap_or("unknown");
        md.push_str(&format!(
            "**Agent:** {} v{}  |  **Model:** {}\n",
            agent, ver, model
        ));
    }
    md.push('\n');

    // Headline table
    md.push_str("## Headline\n\n");
    md.push_str(
        "| Condition | Cost (mean) | Cost vs control | Tokens (mean) | Turns | Verification |\n",
    );
    md.push_str(
        "|-----------|------------:|----------------:|--------------:|------:|:-------------|\n",
    );

    for (i, cond) in report.conditions.iter().enumerate() {
        let cost_str = format!("${:.4}", cond.cost_usd.mean);
        let delta_str = if i == 0 {
            "\u{2014}".to_string()
        } else {
            report
                .deltas_vs_control
                .get(i - 1)
                .and_then(|d| d.cost_delta_ratio)
                .map(|d| format!("**{:+.1}%**", d * 100.0))
                .unwrap_or_else(|| "n/a".into())
        };
        let tokens_str = format_number(cond.tokens.mean as u64);
        let turns_str = format!("{:.1}", cond.turns.mean);
        let verify = format!(
            "tests {}/{}",
            cond.verification.tests_pass.passed, cond.verification.tests_pass.total,
        );
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            cond.description, cost_str, delta_str, tokens_str, turns_str, verify,
        ));
    }
    md.push('\n');

    // Always show confidence — readers need this to assess reliability.
    if report.confidence.publishable {
        md.push_str(&format!(
            "> Confidence: {}. Results are publishable.\n\n",
            report.confidence.reason
        ));
    } else {
        md.push_str(&format!("> {}\n\n", report.confidence.reason));
    }

    // Token breakdown
    if report.conditions.len() >= 2 {
        md.push_str("## Token breakdown (means)\n\n");
        md.push_str(
            "> `cache_read` dominates — most input is served from the prompt cache. \
`input` (non-cached) is minimal by design.\n\n",
        );
        md.push_str("| Category | ");
        for cond in &report.conditions {
            md.push_str(&format!("{} | ", cond.description));
        }
        md.push_str("\n|----------|");
        for _ in &report.conditions {
            md.push_str("--------:|");
        }
        md.push('\n');

        for (label, values) in [
            (
                "input",
                report
                    .conditions
                    .iter()
                    .map(|c| c.tokens_by_category.input.mean)
                    .collect::<Vec<_>>(),
            ),
            (
                "output",
                report
                    .conditions
                    .iter()
                    .map(|c| c.tokens_by_category.output.mean)
                    .collect::<Vec<_>>(),
            ),
            (
                "cache_read",
                report
                    .conditions
                    .iter()
                    .map(|c| c.tokens_by_category.cache_read.mean)
                    .collect::<Vec<_>>(),
            ),
            (
                "cache_creation",
                report
                    .conditions
                    .iter()
                    .map(|c| c.tokens_by_category.cache_creation.mean)
                    .collect::<Vec<_>>(),
            ),
        ] {
            md.push_str(&format!("| {} |", label));
            for val in values {
                md.push_str(&format!(" {} |", format_number(val as u64)));
            }
            md.push('\n');
        }
        md.push('\n');
    }

    // Variance
    md.push_str("## Variance\n\n");
    md.push_str("| Metric | ");
    for cond in &report.conditions {
        md.push_str(&format!("{} | ", cond.description));
    }
    md.push_str("\n|--------|");
    for _ in &report.conditions {
        md.push_str("--------:|");
    }
    md.push('\n');
    md.push_str("| Tokens CV% |");
    for cond in &report.conditions {
        md.push_str(&format!(" {:.1}% |", cond.tokens.cv * 100.0));
    }
    md.push('\n');
    md.push_str("| Cost CV% |");
    for cond in &report.conditions {
        md.push_str(&format!(" {:.1}% |", cond.cost_usd.cv * 100.0));
    }
    md.push_str("\n\n");

    // Tools histogram
    if report.conditions.len() >= 2 {
        let mut all_tools: BTreeMap<String, ()> = BTreeMap::new();
        for cond in &report.conditions {
            for key in cond.tools_histogram.keys() {
                all_tools.insert(key.clone(), ());
            }
        }
        if !all_tools.is_empty() {
            md.push_str("## Mechanism — tools histogram (per-run means)\n\n");
            md.push_str("| Tool |");
            for cond in &report.conditions {
                md.push_str(&format!(" {} |", cond.description));
            }
            md.push_str("\n|------|");
            for _ in &report.conditions {
                md.push_str("--------:|");
            }
            md.push('\n');
            for tool in all_tools.keys() {
                md.push_str(&format!("| {} |", tool));
                for cond in &report.conditions {
                    let val = cond.tools_histogram.get(tool).copied().unwrap_or(0.0);
                    md.push_str(&format!(" {:.1} |", val));
                }
                md.push('\n');
            }
            md.push('\n');
        }
    }

    // Landmines
    md.push_str("## Landmines\n\n");
    let any_landmines = report
        .conditions
        .iter()
        .any(|c| c.landmines.stuck_loop_runs > 0 || c.landmines.is_error_storms > 0);
    if any_landmines {
        for cond in &report.conditions {
            if cond.landmines.stuck_loop_runs > 0 {
                md.push_str(&format!(
                    "- **{}**: {} run(s) with stuck loop\n",
                    cond.description, cond.landmines.stuck_loop_runs,
                ));
            }
            if cond.landmines.is_error_storms > 0 {
                md.push_str(&format!(
                    "- **{}**: {} run(s) with error storm\n",
                    cond.description, cond.landmines.is_error_storms,
                ));
            }
        }
    } else {
        md.push_str("*No landmines detected.*\n");
    }
    md.push('\n');

    // Per-run raw data
    md.push_str("## Per-run raw data\n\n");
    md.push_str("> ✔ = passed, ✘ = failed, N/A = gate not applicable to this task type.\n\n");
    md.push_str("| Run | Condition | Tokens | Cost | Turns | Tools | Tests | Build | Check |\n");
    md.push_str("|-----|-----------|-------:|-----:|------:|------:|:-----:|:-----:|:-----:|\n");
    for run in &report.runs {
        let cost_str = run
            .cost_usd
            .map(|c| format!("${:.4}", c))
            .unwrap_or_else(|| "n/a".into());
        let check = |v: Option<bool>| match v {
            Some(true) => "✔",
            Some(false) => "✘",
            None => "N/A", // gate not applicable to this task
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            run.run_index,
            run.condition,
            format_number(run.tokens),
            cost_str,
            run.turns,
            run.tool_calls,
            check(run.tests_pass),
            check(run.build_pass),
            check(run.check_pass),
        ));
    }

    md
}

fn format_number(n: u64) -> String {
    if n == 0 {
        return "0".into();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
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
            cache_read_input_tokens: tokens / 2,
            cache_creation_input_tokens: tokens / 4,
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
            profile: Default::default(),
            profile_version: crate::benchmark::profiles::default_profile_version(),
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
        let control = make_sample(
            "Native",
            vec![
                make_observation(100000, 0.20, 20, vec!["Bash", "Read", "Read"]),
                make_observation(120000, 0.25, 24, vec!["Bash", "Read", "Glob"]),
                make_observation(110000, 0.22, 22, vec!["Bash", "Read", "Read"]),
            ],
        );
        let treatment = make_sample(
            "With 8v",
            vec![
                make_observation(60000, 0.12, 12, vec!["mcp__8v__8v", "mcp__8v__8v"]),
                make_observation(70000, 0.14, 14, vec!["mcp__8v__8v", "mcp__8v__8v"]),
                make_observation(65000, 0.13, 13, vec!["mcp__8v__8v", "mcp__8v__8v"]),
            ],
        );

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
        assert!((delta.cost_delta_ratio.unwrap() - expected).abs() < 0.001);
        assert!(
            delta.cost_delta_ratio.unwrap() < 0.0,
            "treatment should be cheaper"
        );
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
            ToolCallDetail {
                name: "Read".into(),
                input: "{\"file\":\"x\"}".into(),
                output_bytes: 10,
                is_error: false,
            },
            ToolCallDetail {
                name: "Read".into(),
                input: "{\"file\":\"x\"}".into(),
                output_bytes: 10,
                is_error: false,
            },
            ToolCallDetail {
                name: "Read".into(),
                input: "{\"file\":\"x\"}".into(),
                output_bytes: 10,
                is_error: false,
            },
        ];
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(landmines.stuck_loop_runs, 1);
    }

    #[test]
    fn landmine_thrash_same_name_varying_inputs() {
        // Agent calls Read 5x in a row with different file paths — not caught
        // by strict "identical" rule but is a real stuck-loop signal.
        let mut obs = make_observation(100, 0.01, 1, vec!["Read"; 5]);
        obs.tool_calls_detail = (0..5)
            .map(|i| ToolCallDetail {
                name: "Read".into(),
                input: format!(r#"{{"file":"f{i}"}}"#),
                output_bytes: 10,
                is_error: false,
            })
            .collect();
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(
            landmines.stuck_loop_runs, 1,
            "5 consecutive same-name calls should be flagged as stuck loop"
        );
    }

    #[test]
    fn landmine_no_false_positive_on_diverse_sequence() {
        // Read, Grep, Edit, Read, Grep — varied, should NOT flag.
        let mut obs = make_observation(100, 0.01, 1, vec!["Read", "Grep", "Edit", "Read", "Grep"]);
        obs.tool_calls_detail = ["Read", "Grep", "Edit", "Read", "Grep"]
            .iter()
            .map(|n| ToolCallDetail {
                name: (*n).into(),
                input: "{}".into(),
                output_bytes: 10,
                is_error: false,
            })
            .collect();
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(landmines.stuck_loop_runs, 0);
    }

    #[test]
    fn landmine_mcp_sequential_not_flagged() {
        // 8v routes all operations through mcp__8v__8v — 5+ consecutive calls
        // are expected (ls → read → write → write → test) and must NOT trigger
        // the loose stuck-loop detector.
        let mut obs = make_observation(100, 0.01, 1, vec!["mcp__8v__8v"; 6]);
        obs.tool_calls_detail = [
            ("mcp__8v__8v", r#"{"command":"8v ls --tree"}"#),
            ("mcp__8v__8v", r#"{"command":"8v read lib.go --full"}"#),
            ("mcp__8v__8v", r#"{"command":"8v write lib.go:12 \"fix\""}"#),
            (
                "mcp__8v__8v",
                r#"{"command":"8v write lib.go:10-11 --delete"}"#,
            ),
            ("mcp__8v__8v", r#"{"command":"8v test ."}"#),
        ]
        .iter()
        .map(|(name, input)| ToolCallDetail {
            name: (*name).into(),
            input: (*input).into(),
            output_bytes: 10,
            is_error: false,
        })
        .collect();
        let sample = make_sample("test", vec![obs]);
        let landmines = detect_landmines(&sample);
        assert_eq!(
            landmines.stuck_loop_runs, 0,
            "sequential mcp__8v__8v calls are not a stuck loop"
        );
    }

    #[test]
    fn landmine_error_storm_detected() {
        let mut obs = make_observation(100, 0.01, 1, vec!["Write"; 5]);
        obs.tool_calls_detail = (0..5)
            .map(|_| ToolCallDetail {
                name: "Write".into(),
                input: "{}".into(),
                output_bytes: 0,
                is_error: true,
            })
            .collect();
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

    // ── Markdown rendering tests ──────────────────────────────────────

    #[test]
    fn render_markdown_has_header_and_metadata() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.starts_with("# Benchmark — test-experiment\n"));
        assert!(md.contains("**Commit:**"));
        assert!(md.contains("**Task:** test-task"));
        assert!(md.contains("**Agent:** claude-code v1.0.0"));
    }

    #[test]
    fn render_markdown_has_headline_table() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Headline"));
        assert!(md.contains("| Condition | Cost (mean)"));
        assert!(md.contains("| Native |"));
        assert!(md.contains("| With 8v |"));
    }

    #[test]
    fn render_markdown_has_token_breakdown() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Token breakdown"));
        assert!(md.contains("| input |"));
        assert!(md.contains("| output |"));
        assert!(md.contains("| cache_read |"));
        assert!(md.contains("| cache_creation |"));
    }

    #[test]
    fn render_markdown_has_variance() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Variance"));
        assert!(md.contains("Tokens CV%"));
        assert!(md.contains("Cost CV%"));
    }

    #[test]
    fn render_markdown_has_tools_histogram() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Mechanism — tools histogram"));
        assert!(md.contains("| Bash |"));
        assert!(md.contains("| mcp__8v__8v |"));
    }

    #[test]
    fn render_markdown_has_landmines_section() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Landmines"));
        assert!(md.contains("No landmines detected"));
    }

    #[test]
    fn render_markdown_has_per_run_data() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        assert!(md.contains("## Per-run raw data"));
        // Count rows in the per-run table by looking for "| 0 |" through "| 5 |"
        let in_per_run = md.split("## Per-run raw data").nth(1).unwrap();
        let data_rows: Vec<&str> = in_per_run
            .lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Run") && !l.starts_with("|--"))
            .collect();
        assert_eq!(
            data_rows.len(),
            6,
            "expected 6 per-run rows, got: {}",
            data_rows.len()
        );
    }

    #[test]
    fn render_markdown_includes_confidence_info() {
        let report = build_report(&make_result());
        let md = render_markdown(&report);
        if !report.confidence.publishable {
            assert!(
                md.contains(&report.confidence.reason),
                "non-publishable must show reason"
            );
        }
        // Either way, the headline section must reference conditions
        assert!(md.contains("N="), "markdown must mention sample size");
    }

    // ── Acceptance: round-trip through BenchmarkStore ────────────────

    #[test]
    fn acceptance_report_round_trip_through_store() {
        let result = make_result();
        let report = build_report(&result);

        let tmp = tempfile::tempdir().unwrap();
        let store = crate::benchmark::store::BenchmarkStore::at(tmp.path()).unwrap();
        let dir = store.write_report("acceptance-test", &report).unwrap();

        // report.json: parseable and schema matches
        let json_path = dir.join("report.json");
        assert!(json_path.exists());
        let json_str = std::fs::read_to_string(&json_path).unwrap();
        let parsed: ReportJson = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.experiment, "test-experiment");
        assert_eq!(parsed.conditions.len(), 2);
        assert_eq!(parsed.deltas_vs_control.len(), 1);
        assert_eq!(parsed.runs.len(), 6);
        assert_eq!(parsed.confidence.n_per_condition, 3);

        // report.md: valid markdown with all sections
        let md_path = dir.join("report.md");
        assert!(md_path.exists());
        let md = std::fs::read_to_string(&md_path).unwrap();
        let expected_sections = [
            "# Benchmark",
            "## Headline",
            "## Token breakdown",
            "## Variance",
            "## Mechanism",
            "## Landmines",
            "## Per-run raw data",
        ];
        for section in &expected_sections {
            assert!(md.contains(section), "missing section: {section}");
        }

        // JSON and markdown agree on key facts
        assert!(md.contains(&parsed.experiment));
        assert!(md.contains(&parsed.commit));
        for cond in &parsed.conditions {
            assert!(
                md.contains(&cond.description),
                "markdown missing condition: {}",
                cond.description
            );
        }
    }

    #[test]
    fn run_record_roundtrip_with_profile() {
        use crate::benchmark::profiles::ToolProfile;
        let rec = RunRecord {
            run_index: 0,
            condition: "test".to_string(),
            tokens: 0,
            cost_usd: None,
            turns: 0,
            tool_calls: 0,
            tests_pass: None,
            build_pass: None,
            check_pass: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            tool_calls_detail: vec![],
            profile: ToolProfile::Caveman,
            profile_version: "stub-v0".to_string(),
        };
        let json = serde_json::to_string(&rec).unwrap();
        let parsed: RunRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.profile, ToolProfile::Caveman);
        assert_eq!(parsed.profile_version, "stub-v0");
    }

    #[test]
    fn run_record_backcompat_no_profile_fields() {
        use crate::benchmark::profiles::ToolProfile;
        let json = r#"{
            "run_index": 0,
            "condition": "test",
            "tokens": 0,
            "turns": 0,
            "tool_calls": 0,
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_read_tokens": 0,
            "cache_creation_tokens": 0
        }"#;
        let parsed: RunRecord = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.profile, ToolProfile::Native);
        assert_eq!(parsed.profile_version, "pre-2026-04");
    }
}
