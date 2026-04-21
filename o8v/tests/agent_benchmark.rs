// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Agent benchmark tests — new infrastructure.
//!
//! Each test picks a scenario and asserts on the result.
//! The pipeline handles everything: setup, run, collect, verify, persist.
//!
//! ## Running
//!
//! ```sh
//! # All benchmarks (~5-10 min, costs tokens)
//! cargo test -p o8v --test agent_benchmark -- --ignored --nocapture
//!
//! # One scenario
//! cargo test -p o8v --test agent_benchmark fix_test_8v -- --ignored --nocapture
//! ```
//!
//! ## IMPORTANT: run experiments sequentially
//!
//! Experiments share `~/.8v/events.ndjson`. Running them in parallel produces
//! corrupted measurements. Always pass `--test-threads=1`:
//!
//! ```sh
//! cargo test --test agent_benchmark experiment_fix_test -- --ignored --nocapture --test-threads=1
//! ```

mod scenarios;

use o8v_testkit::benchmark::{
    run_experiment, run_experiment_with_matrix, run_scenario, ExperimentMatrix, ToolProfile,
};

/// Panic with a clear message if the named gate is not `Some(true)`.
///
/// Distinguishes "gate didn't run" (`None`) from "gate ran and failed"
/// (`Some(false)`) — both were previously collapsed to the same error by
/// `.unwrap_or(false)`.
#[track_caller]
fn require_pass(gate: &str, result: Option<bool>) {
    match result {
        Some(true) => {}
        Some(false) => panic!("{gate} ran and FAILED — agent did not fix the problem"),
        None => panic!("{gate} did NOT RUN — verification is broken or fixture mismatched"),
    }
}

// ── Fix failing test ─────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::FIX_TEST_BASELINE,
        binary,
        true,
        ToolProfile::Native,
    );

    require_pass("cargo test", record.verification.tests_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TEST_8V, binary, true, ToolProfile::EightV);

    require_pass("cargo test", record.verification.tests_pass);
}

// ── Diagnose issues ──────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::DIAGNOSE_BASELINE,
        binary,
        true,
        ToolProfile::Native,
    );

    require_pass("cargo clippy", record.verification.check_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::DIAGNOSE_8V, binary, true, ToolProfile::EightV);

    require_pass("cargo clippy", record.verification.check_pass);
}

// ── Fix Python (traversal) ───────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, `python3` + `pytest` (~60s, costs tokens)"]
fn fix_python_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::FIX_PYTHON_BASELINE,
        binary,
        true,
        ToolProfile::Native,
    );

    require_pass("pytest", record.verification.tests_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH, `python3` + `pytest` (~60s, costs tokens)"]
fn fix_python_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_PYTHON_8V, binary, true, ToolProfile::EightV);

    require_pass("pytest", record.verification.tests_pass);
}

// ── Fix Go ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, `go` toolchain (~60s, costs tokens)"]
fn fix_go_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::FIX_GO_BASELINE,
        binary,
        true,
        ToolProfile::Native,
    );

    require_pass("go test ./...", record.verification.tests_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH, `go` toolchain (~60s, costs tokens)"]
fn fix_go_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_GO_8V, binary, true, ToolProfile::EightV);

    require_pass("go test ./...", record.verification.tests_pass);
}

// ── Fix TypeScript ───────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, `tsc` toolchain (~60s, costs tokens)"]
fn fix_typescript_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::FIX_TS_BASELINE,
        binary,
        true,
        ToolProfile::Native,
    );

    require_pass("tsc --noEmit", record.verification.tests_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH, `tsc` toolchain (~60s, costs tokens)"]
fn fix_typescript_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TS_8V, binary, true, ToolProfile::EightV);

    require_pass("tsc --noEmit", record.verification.tests_pass);
}

// ── Caveman profile ──────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_caveman() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(
        &scenarios::FIX_TEST_BASELINE,
        binary,
        true,
        ToolProfile::Caveman,
    );

    require_pass("cargo test", record.verification.tests_pass);
}

// ── Experiments (N=3 per condition) ──────────────────────────────────────────

#[test]
#[ignore = "experiment: baseline + 8v × N runs (~9 min, costs tokens)"]
fn experiment_fix_test() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TEST, binary);

    // Every condition must actually fix the bug
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

#[test]
#[ignore = "experiment: baseline + 8v × N=9 runs (~13 min, costs tokens) — use when N=6 CI exceeds publishability threshold"]
fn experiment_fix_test_n9() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TEST_N9, binary);

    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

#[test]
#[ignore = "experiment: 2 conditions × 3 runs (~6 min, costs tokens)"]
fn experiment_diagnose() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_DIAGNOSE, binary);

    // Agent must fix the issues — cargo check/clippy must pass in every run.
    // Uses the same 100%-pass gate as all other experiments: anything less
    // means the agent is not reliably solving the task.
    assert!(
        result.control.check_pass_count() == result.n,
        "Control failed to fix issues in {}/{} runs",
        result.control.check_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.check_pass_count() == result.n,
            "{} failed to fix issues in {}/{} runs",
            sample.description,
            sample.check_pass_count(),
            result.n
        );
    }
}

#[test]
#[ignore = "experiment: 2 conditions × 3 runs (~9 min, costs tokens)"]
fn experiment_fix_python_traversal() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_PYTHON, binary);

    // Every condition must actually fix the bugs — pytest must pass
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

#[test]
#[ignore = "experiment: baseline + 8v × N runs (~9 min, costs tokens)"]
fn experiment_fix_go() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_GO, binary);

    // Every condition must actually fix the bugs — go test must pass
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

#[test]
#[ignore = "experiment: baseline + 8v × N runs (~9 min, costs tokens)"]
fn experiment_fix_typescript() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TS, binary);

    // Every condition must actually fix the bugs — tsc --noEmit must pass
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

// ── Tool Search experiments ───────────────────────────────────────────────

#[test]
#[ignore = "experiment: native + tool-search × N runs (~9 min, costs tokens)"]
fn experiment_fix_test_tool_search() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let matrix = ExperimentMatrix {
        profiles: vec![ToolProfile::Native, ToolProfile::ToolSearch],
    };
    let result = run_experiment_with_matrix(
        "tool-search-vs-native",
        "fix-test-rust",
        &scenarios::FIX_TEST_BASELINE,
        &matrix,
        6,
        binary,
    );

    // Every condition must actually fix the bug
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

// ── mcp2cli experiments ───────────────────────────────────────────────────

#[test]
#[ignore = "experiment: native + mcp2cli × N runs (~9 min, costs tokens)"]
fn experiment_fix_test_mcp2cli() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let matrix = ExperimentMatrix {
        profiles: vec![ToolProfile::Native, ToolProfile::Mcp2cli],
    };
    let result = run_experiment_with_matrix(
        "mcp2cli-vs-native",
        "fix-test-rust",
        &scenarios::FIX_TEST_BASELINE,
        &matrix,
        6,
        binary,
    );

    // Every condition must actually fix the bug
    assert!(
        result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}

// ── Codex experiments ──────────────────────────────────────────────────────

#[test]
#[ignore = "agent benchmark — requires codex CLI, costs real API credits"]
fn experiment_fix_test_codex() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TEST_CODEX, binary);

    // Codex is an experimental agent: MCP sandbox constraints mean it cannot
    // always complete the task. The gate here requires 100% pass (same as all
    // other experiments) — a weaker gate would let this test pass even if
    // Codex never reliably solves the task.
    assert!(
        result.control.tests_pass_count() == result.n,
        "Codex control failed to fix the test in {}/{} runs",
        result.control.tests_pass_count(),
        result.n
    );
    for sample in &result.treatments {
        assert!(
            sample.tests_pass_count() == result.n,
            "{} failed to fix the test in {}/{} runs",
            sample.description,
            sample.tests_pass_count(),
            result.n
        );
    }
}
