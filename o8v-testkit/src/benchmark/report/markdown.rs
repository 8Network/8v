// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Markdown renderer — converts `ReportJson` to human-readable markdown.

use std::collections::BTreeMap;

use super::types::ReportJson;

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
    let version_summary = report
        .version_8v
        .as_deref()
        .map(summarize_8v_version)
        .unwrap_or_else(|| "unknown".into());
    md.push_str(&format!(
        "**8v:** {}  |  **Benchmark commit:** `{}`  |  **Runs:** N={} per condition\n",
        version_summary,
        &report.commit[..8.min(report.commit.len())],
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

    // Plain-English summary for first-time readers
    if report.conditions.len() >= 2 {
        let control = &report.conditions[0];
        let treatment = &report.conditions[1];
        let cost_delta = report
            .deltas_vs_control
            .first()
            .and_then(|d| d.cost_delta_ratio)
            .map(|d| d * 100.0);
        let turns_delta_pct = report
            .deltas_vs_control
            .first()
            .map(|d| d.turns_delta_ratio * 100.0);
        let all_pass = treatment.verification.tests_pass.passed
            == treatment.verification.tests_pass.total
            && treatment.verification.tests_pass.total > 0;

        md.push_str("## Summary\n\n");
        if let Some(delta) = cost_delta {
            let direction = if delta < 0.0 { "reduced" } else { "increased" };
            md.push_str(&format!(
                "8v {} cost by **{:.1}%** ({:.1} turns vs {:.1}).",
                direction,
                delta.abs(),
                treatment.turns.mean,
                control.turns.mean,
            ));
        }
        if let Some(turns_pct) = turns_delta_pct {
            if turns_pct < -10.0 {
                md.push_str(&format!(
                    " Fewer turns means less back-and-forth ({:.0}% reduction).",
                    turns_pct.abs()
                ));
            }
        }
        if all_pass {
            md.push_str(" All verification runs passed on both conditions.");
        }
        md.push_str(if report.confidence.publishable {
            " Result is publishable."
        } else {
            " **Result is not yet publishable** — run more iterations to narrow the confidence interval."
        });
        md.push_str("\n\n");
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

/// Condense the multi-line `8v --version` output into a single readable token.
///
/// Input (example):
/// ```text
/// 8v 0.1.0
/// commit:       c43e772 (dirty, 61 files modified)
/// commit_date:  2026-04-21 16:11:42 UTC
/// profile:      release
/// ...
/// ```
/// Output: `v0.1.0 @ c43e772 (dirty)` or `v0.1.0 @ c43e772 (clean)`.
fn summarize_8v_version(raw: &str) -> String {
    let mut version = None::<&str>;
    let mut commit_short = None::<&str>;
    let mut dirty = false;

    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("8v ") && version.is_none() {
            version = line.strip_prefix("8v ");
        } else if let Some(rest) = line.strip_prefix("commit:") {
            let val = rest.trim();
            // val looks like "c43e772 (dirty, 61 files modified)" or just "c43e772"
            commit_short = Some(val.split_whitespace().next().unwrap_or(val));
            dirty = val.contains("dirty");
        }
    }

    let ver = version.unwrap_or("0.1.0");
    let sha = commit_short.unwrap_or("unknown");
    let state = if dirty { "dirty" } else { "clean" };
    format!("v{ver} @ {sha} ({state})")
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
