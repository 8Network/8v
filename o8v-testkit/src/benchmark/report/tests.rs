// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::builder::{build_report, detect_landmines, pct_delta, stat_block};
use super::markdown::render_markdown;
use super::types::{ReportJson, RunRecord};
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
    assert!(md.contains("**8v:**"));
    assert!(md.contains("**Benchmark commit:**"));
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
