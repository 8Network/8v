// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Experiment runner — runs scenarios N times and produces structured comparisons.
//!
//! `run_experiment()` is the entry point. It:
//! 1. Runs the control scenario N times
//! 2. Runs each treatment scenario N times
//! 3. Computes statistics per sample
//! 4. Computes effects (treatment vs control)
//! 5. Renders the comparison table
//! 6. Persists the ExperimentResult
//! 7. Returns the result for assertions

use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

use super::pipeline::{current_git_commit, run_scenario, unix_ms};
use super::preflight::preflight_fixture;
use super::profiles::ToolProfile;
use super::store::BenchmarkStore;
use super::types::*;

/// Cartesian product of scenarios × profiles.
///
/// Every scenario in `scenarios` is run under each profile in `profiles`, producing
/// one `Sample` per (scenario, profile) pair. Use this when you want to sweep all
/// combinations rather than hand-wiring each pair.
///
/// # Example
///
/// ```rust,ignore
/// let matrix = ExperimentMatrix {
///     profiles: vec![ToolProfile::Native, ToolProfile::EightV],
/// };
/// let samples = matrix.run(&[&SCENARIO_A, &SCENARIO_B], n, binary);
/// ```
pub struct ExperimentMatrix {
    /// Profiles to sweep. Defaults to `[Native, EightV]` if empty.
    pub profiles: Vec<ToolProfile>,
}

impl Default for ExperimentMatrix {
    fn default() -> Self {
        Self {
            profiles: vec![ToolProfile::Native, ToolProfile::EightV],
        }
    }
}

impl ExperimentMatrix {
    /// Run every (scenario, profile) pair N times and return all samples.
    ///
    /// Samples are ordered: scenario 0 × all profiles, scenario 1 × all profiles, …
    pub fn run(&self, scenarios: &[&Scenario], n: usize, binary: &str) -> Vec<Sample> {
        let profiles = if self.profiles.is_empty() {
            &[ToolProfile::Native, ToolProfile::EightV][..]
        } else {
            &self.profiles[..]
        };

        let mut samples = Vec::with_capacity(scenarios.len() * profiles.len());
        for scenario in scenarios {
            for &profile in profiles {
                samples.push(run_sample_with_profile(scenario, n, binary, profile));
            }
        }
        samples
    }
}

/// Run an experiment with explicit profile control using `ExperimentMatrix`.
///
/// The first profile in the matrix is the control; the rest are treatments.
/// Runs `scenario` N times under each profile. All other pipeline steps
/// (preflight, compute_effects, render_table, write_structured_report,
/// persist_experiment) are identical to `run_experiment`.
///
/// Use this instead of `run_experiment` when you need profiles that cannot be
/// inferred from `scenario.env.setup_8v` (e.g. `ToolProfile::ToolSearch`).
pub fn run_experiment_with_matrix(
    name: &str,
    task_name: &str,
    scenario: &Scenario,
    matrix: &ExperimentMatrix,
    n: usize,
    binary: &str,
) -> ExperimentResult {
    assert!(
        n >= 1,
        "experiment requires at least 1 observation per scenario"
    );

    let profiles = if matrix.profiles.is_empty() {
        vec![ToolProfile::Native, ToolProfile::EightV]
    } else {
        matrix.profiles.clone()
    };

    assert!(
        profiles.len() >= 2,
        "matrix must have at least 2 profiles (control + 1 treatment)"
    );

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("EXPERIMENT: {} (N={})", name, n);
    eprintln!("Task: {}", task_name);
    eprintln!("Control: {:?}", profiles[0]);
    for p in &profiles[1..] {
        eprintln!("Treatment: {:?}", p);
    }
    eprintln!("{}\n", "=".repeat(70));

    // ── Preflight gate ──────────────────────────────────────────────────
    preflight_fixture(scenario);

    // ── Run control (first profile) ─────────────────────────────────────
    let control = run_sample_with_profile(scenario, n, binary, profiles[0]);

    // ── Run treatments (remaining profiles) ─────────────────────────────
    let treatments: Vec<Sample> = profiles[1..]
        .iter()
        .map(|&profile| run_sample_with_profile(scenario, n, binary, profile))
        .collect();

    // ── Compute effects ─────────────────────────────────────────────────
    let effects = compute_effects(&control, &treatments, n);

    // ── Build result ────────────────────────────────────────────────────
    let result = ExperimentResult {
        name: name.to_string(),
        task: task_name.to_string(),
        git_commit: current_git_commit(),
        timestamp_ms: unix_ms(),
        n,
        control,
        treatments,
        effects,
        provenance: None,
    };

    // ── Render table ────────────────────────────────────────────────────
    render_table(&result);

    // ── Open store once for report + persist ────────────────────────────
    let store = match BenchmarkStore::open() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("  [benchmark] warning: failed to open benchmark store: {e}");
            None
        }
    };

    // ── Structured report ───────────────────────────────────────────────
    let report = super::report::build_report(&result);
    write_structured_report(&report, name, store.as_ref());

    // ── Persist ─────────────────────────────────────────────────────────
    persist_experiment(&result, store.as_ref());

    result
}

/// Run an experiment: N observations per scenario, compare treatments against control.
pub fn run_experiment(config: &ExperimentConfig, binary: &str) -> ExperimentResult {
    assert!(
        config.n >= 1,
        "experiment requires at least 1 observation per scenario"
    );
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("EXPERIMENT: {} (N={})", config.name, config.n);
    eprintln!("Task: {}", config.task.name);
    eprintln!("Control: {}", config.control.description);
    for t in config.treatments {
        eprintln!("Treatment: {}", t.description);
    }
    eprintln!("{}\n", "=".repeat(70));

    // ── Preflight gate ──────────────────────────────────────────────────
    // Reject fixtures that already pass every verifier gate — a benchmark on
    // a green fixture measures noise. See docs/design/fixture-preflight-gate.md.
    preflight_fixture(config.control);

    // ── Run control ─────────────────────────────────────────────────────
    let control = run_sample(config.control, config.n, binary);

    // ── Run treatments ──────────────────────────────────────────────────
    let treatments: Vec<Sample> = config
        .treatments
        .iter()
        .map(|scenario| run_sample(scenario, config.n, binary))
        .collect();

    // ── Compute effects ─────────────────────────────────────────────────
    let effects = compute_effects(&control, &treatments, config.n);

    // ── Build result ────────────────────────────────────────────────────
    let result = ExperimentResult {
        name: config.name.to_string(),
        task: config.task.name.to_string(),
        git_commit: current_git_commit(),
        timestamp_ms: unix_ms(),
        n: config.n,
        control,
        treatments,
        effects,
        provenance: None,
    };

    // ── Render table ────────────────────────────────────────────────────
    render_table(&result);

    // ── Open store once for report + persist ────────────────────────────
    let store = match BenchmarkStore::open() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("  [benchmark] warning: failed to open benchmark store: {e}");
            None
        }
    };

    // ── Structured report ───────────────────────────────────────────────
    let report = super::report::build_report(&result);
    write_structured_report(&report, config.name, store.as_ref());

    // ── Persist ─────────────────────────────────────────────────────────
    persist_experiment(&result, store.as_ref());

    result
}

/// Run a scenario N times, collecting observations into a Sample.
fn run_sample(scenario: &Scenario, n: usize, binary: &str) -> Sample {
    let mut observations = Vec::with_capacity(n);

    for i in 0..n {
        eprintln!("\n--- {} ({}/{}) ---", scenario.description, i + 1, n);

        let profile = if scenario.env.setup_8v {
            ToolProfile::EightV
        } else {
            ToolProfile::Native
        };
        let observation = run_scenario(scenario, binary, false, profile, i as u32);
        observations.push(observation);
    }

    Sample {
        scenario: scenario.name.to_string(),
        description: scenario.description.to_string(),
        observations,
    }
}

/// Run a scenario N times with an explicit profile override.
fn run_sample_with_profile(
    scenario: &Scenario,
    n: usize,
    binary: &str,
    profile: ToolProfile,
) -> Sample {
    let mut observations = Vec::with_capacity(n);

    for i in 0..n {
        eprintln!("\n--- {} ({}/{}) ---", scenario.description, i + 1, n);
        let observation = run_scenario(scenario, binary, false, profile, i as u32);
        observations.push(observation);
    }

    Sample {
        scenario: scenario.name.to_string(),
        description: scenario.description.to_string(),
        observations,
    }
}

/// Compute effects: each treatment compared against the control.
fn mean_cost(sample: &Sample) -> Option<f64> {
    let costs: Vec<f64> = sample
        .observations
        .iter()
        .filter_map(|o| o.cost_usd)
        .collect();
    if costs.is_empty() {
        None
    } else {
        Some(costs.iter().sum::<f64>() / costs.len() as f64)
    }
}

fn compute_effects(control: &Sample, treatments: &[Sample], n: usize) -> Vec<Effect> {
    let control_tokens = control.require_mean(|o| o.total_tokens as f64);
    let control_cost = mean_cost(control);
    let control_tools = control.require_mean(|o| o.tool_names.len() as f64);

    treatments
        .iter()
        .map(|treatment| {
            let t_tokens = treatment.require_mean(|o| o.total_tokens as f64);
            let t_cost = mean_cost(treatment);
            let t_tools = treatment.require_mean(|o| o.tool_names.len() as f64);

            let token_delta_pct = if control_tokens > 0.0 {
                ((t_tokens - control_tokens) / control_tokens) * 100.0
            } else {
                0.0
            };

            let cost_delta_pct = match (control_cost, t_cost) {
                (Some(c), Some(t)) if c > 0.0 => Some(((t - c) / c) * 100.0),
                _ => None,
            };

            let tool_call_delta_pct = if control_tools > 0.0 {
                ((t_tools - control_tools) / control_tools) * 100.0
            } else {
                0.0
            };

            Effect {
                name: format!("{} vs {}", treatment.description, control.description),
                treatment: treatment.scenario.clone(),
                control: control.scenario.clone(),
                token_delta_pct,
                cost_delta_pct,
                tool_call_delta_pct,
                sufficient_n: n >= 5,
            }
        })
        .collect()
}

/// Render the comparison table to stderr.
fn render_table(result: &ExperimentResult) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // Header: metric name + one column per condition
    let mut header = vec!["".to_string(), result.control.description.clone()];
    for t in &result.treatments {
        header.push(t.description.clone());
    }
    table.set_header(header);

    // Tokens (mean)
    let mut row = vec!["Tokens (mean)".to_string()];
    row.push(format_tokens(
        result.control.require_mean(|o| o.total_tokens as f64),
    ));
    for t in &result.treatments {
        row.push(format_tokens(t.require_mean(|o| o.total_tokens as f64)));
    }
    table.add_row(row);

    // Tokens (stddev)
    let mut row = vec!["Tokens (stddev)".to_string()];
    row.push(format_tokens(
        result
            .control
            .stddev(|o| o.total_tokens as f64)
            .unwrap_or(0.0),
    ));
    for t in &result.treatments {
        row.push(format_tokens(
            t.stddev(|o| o.total_tokens as f64).unwrap_or(0.0),
        ));
    }
    table.add_row(row);

    // Cost (mean)
    let mut row = vec!["Cost (mean)".to_string()];
    row.push(match mean_cost(&result.control) {
        Some(c) => format_cost(c),
        None => "n/a".to_string(),
    });
    for t in &result.treatments {
        row.push(match mean_cost(t) {
            Some(c) => format_cost(c),
            None => "n/a".to_string(),
        });
    }
    table.add_row(row);

    // Tool calls (mean)
    let mut row = vec!["Tool calls (mean)".to_string()];
    row.push(format!(
        "{:.1}",
        result.control.require_mean(|o| o.tool_names.len() as f64)
    ));
    for t in &result.treatments {
        row.push(format!(
            "{:.1}",
            t.require_mean(|o| o.tool_names.len() as f64)
        ));
    }
    table.add_row(row);

    // 8v events (mean)
    let mut row = vec!["8v events (mean)".to_string()];
    row.push(format!(
        "{:.1}",
        result.control.require_mean(|o| o.event_count as f64)
    ));
    for t in &result.treatments {
        row.push(format!("{:.1}", t.require_mean(|o| o.event_count as f64)));
    }
    table.add_row(row);

    // Input Tokens (mean)
    let mut row = vec!["Input Tokens (mean)".to_string()];
    row.push(format!(
        "{:.0}",
        result.control.require_mean(|o| o.input_tokens as f64)
    ));
    for t in &result.treatments {
        row.push(format!("{:.0}", t.require_mean(|o| o.input_tokens as f64)));
    }
    table.add_row(row);

    // Output Tokens (mean)
    let mut row = vec!["Output Tokens (mean)".to_string()];
    row.push(format!(
        "{:.0}",
        result.control.require_mean(|o| o.output_tokens as f64)
    ));
    for t in &result.treatments {
        row.push(format!("{:.0}", t.require_mean(|o| o.output_tokens as f64)));
    }
    table.add_row(row);

    // Cache Read (mean)
    let mut row = vec!["Cache Read (mean)".to_string()];
    row.push(format!(
        "{:.0}",
        result
            .control
            .require_mean(|o| o.cache_read_input_tokens as f64)
    ));
    for t in &result.treatments {
        row.push(format!(
            "{:.0}",
            t.require_mean(|o| o.cache_read_input_tokens as f64)
        ));
    }
    table.add_row(row);

    // Cache Creation (mean)
    let mut row = vec!["Cache Creation (mean)".to_string()];
    row.push(format!(
        "{:.0}",
        result
            .control
            .require_mean(|o| o.cache_creation_input_tokens as f64)
    ));
    for t in &result.treatments {
        row.push(format!(
            "{:.0}",
            t.require_mean(|o| o.cache_creation_input_tokens as f64)
        ));
    }
    table.add_row(row);

    // Turns (mean)
    let mut row = vec!["Turns (mean)".to_string()];
    row.push(format!(
        "{:.1}",
        result.control.require_mean(|o| o.turn_count as f64)
    ));
    for t in &result.treatments {
        row.push(format!("{:.1}", t.require_mean(|o| o.turn_count as f64)));
    }
    table.add_row(row);

    // Schema Tax (init bytes) — system message size, which carries every MCP
    // tool's schema. Baseline (no MCP) shows bare cost; 8v conditions show
    // the tax the 8v MCP server adds on top.
    let mut row = vec!["Schema Tax (init bytes)".to_string()];
    row.push(format!(
        "{:.0}",
        result.control.require_mean(|o| o.init_message_bytes as f64)
    ));
    for t in &result.treatments {
        row.push(format!(
            "{:.0}",
            t.require_mean(|o| o.init_message_bytes as f64)
        ));
    }
    table.add_row(row);

    // Success rate
    let mut row = vec!["Tests Pass".to_string()];
    row.push(format!(
        "{}/{}",
        result.control.tests_pass_count(),
        result.control.n()
    ));
    for t in &result.treatments {
        row.push(format!("{}/{}", t.tests_pass_count(), t.n()));
    }
    table.add_row(row);

    // Check Pass
    let mut row = vec!["Check Pass".to_string()];
    row.push(format!(
        "{}/{}",
        result.control.check_pass_count(),
        result.control.n()
    ));
    for t in &result.treatments {
        row.push(format!("{}/{}", t.check_pass_count(), t.n()));
    }
    table.add_row(row);

    // Build Pass
    let mut row = vec!["Build Pass".to_string()];
    row.push(format!(
        "{}/{}",
        result.control.build_pass_count(),
        result.control.n()
    ));
    for t in &result.treatments {
        row.push(format!("{}/{}", t.build_pass_count(), t.n()));
    }
    table.add_row(row);

    // Separator row
    let col_count = 2 + result.treatments.len();
    let mut sep = vec!["─ Δ vs control ─".to_string(), "—".to_string()];
    for _ in &result.treatments {
        sep.push(String::new());
    }
    // Trim to exact column count
    sep.truncate(col_count);
    table.add_row(sep);

    // Δ Tokens row
    let mut row = vec!["Δ Tokens".to_string(), "—".to_string()];
    for effect in &result.effects {
        row.push(format_delta_pct(effect.token_delta_pct));
    }
    // Pad if fewer effects than treatments (shouldn't happen, but be safe)
    while row.len() < col_count {
        row.push(String::new());
    }
    row.truncate(col_count);
    table.add_row(row);

    // Δ Cost row
    let mut row = vec!["Δ Cost".to_string(), "—".to_string()];
    for effect in &result.effects {
        row.push(match effect.cost_delta_pct {
            Some(pct) => format_delta_pct(pct),
            None => "n/a".to_string(),
        });
    }
    while row.len() < col_count {
        row.push(String::new());
    }
    row.truncate(col_count);
    table.add_row(row);

    // Δ Tool calls row
    let mut row = vec!["Δ Tool calls".to_string(), "—".to_string()];
    for effect in &result.effects {
        row.push(format_delta_pct(effect.tool_call_delta_pct));
    }
    while row.len() < col_count {
        row.push(String::new());
    }
    row.truncate(col_count);
    table.add_row(row);

    // Confidence row
    let mut row = vec!["Confidence".to_string(), "—".to_string()];
    for effect in &result.effects {
        row.push(if effect.sufficient_n {
            "N≥5".to_string()
        } else {
            "N<5".to_string()
        });
    }
    while row.len() < col_count {
        row.push(String::new());
    }
    row.truncate(col_count);
    table.add_row(row);

    eprintln!("\n{table}\n");
}

fn format_tokens(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

fn format_cost(v: f64) -> String {
    format!("${:.4}", v)
}

fn format_delta_pct(pct: f64) -> String {
    format!("{:+.1}%", pct)
}

fn write_structured_report(
    report: &super::report::ReportJson,
    experiment_name: &str,
    store: Option<&BenchmarkStore>,
) {
    let Some(store) = store else { return };
    match store.write_report(experiment_name, report) {
        Ok(path) => eprintln!("  [benchmark] report: {}", path.display()),
        Err(e) => eprintln!("  [benchmark] warning: failed to write report: {e}"),
    }
}

fn persist_experiment(result: &ExperimentResult, store: Option<&BenchmarkStore>) {
    let Some(store) = store else { return };
    // Persist each observation individually
    for obs in &result.control.observations {
        if let Err(e) = store.append(obs) {
            eprintln!("  [benchmark] warning: failed to persist observation: {e}");
        }
    }
    for sample in &result.treatments {
        for obs in &sample.observations {
            if let Err(e) = store.append(obs) {
                eprintln!("  [benchmark] warning: failed to persist observation: {e}");
            }
        }
    }
    // Persist the ExperimentResult to experiments.ndjson
    if let Err(e) = store.append_experiment(result) {
        eprintln!("  [benchmark] warning: failed to persist experiment result: {e}");
    }
}
