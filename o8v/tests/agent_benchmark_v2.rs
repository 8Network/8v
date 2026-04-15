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
//! cargo test -p o8v --test agent_benchmark_v2 -- --ignored --nocapture
//!
//! # One scenario
//! cargo test -p o8v --test agent_benchmark_v2 fix_test_8v -- --ignored --nocapture
//! ```

mod scenarios;

use o8v_testkit::benchmark::{run_experiment, run_scenario};

// ── Fix failing test ─────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TEST_BASELINE, binary, true);

    // Baseline: agent should be able to fix the bug with native tools
    assert!(
        record.verification.tests_pass.unwrap_or(false),
        "Agent did not fix the bug (cargo test failed)"
    );
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TEST_8V, binary, true);

    // With 8v available: agent should fix the bug
    assert!(
        record.verification.tests_pass.unwrap_or(false),
        "Agent did not fix the bug (cargo test failed)"
    );
}

// ── Diagnose issues ──────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::DIAGNOSE_BASELINE, binary, true);

    assert!(
        record.verification.check_pass.unwrap_or(false),
        "Agent did not fix the issues (cargo check/clippy failed)"
    );
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::DIAGNOSE_8V, binary, true);

    assert!(
        record.verification.check_pass.unwrap_or(false),
        "Agent did not fix the issues (cargo check/clippy failed)"
    );
}

// ── Check polyglot ───────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, polyglot toolchains (~180s, costs tokens)"]
fn check_polyglot_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::CHECK_POLYGLOT_BASELINE, binary, true);

    assert!(
        record.verification.check_pass.unwrap_or(false) || record.verification.build_pass.unwrap_or(false),
        "Agent did not fix the issues (cargo check/clippy failed)"
    );
}

#[test]
#[ignore = "requires: `claude` in PATH, polyglot toolchains (~180s, costs tokens)"]
fn check_polyglot_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::CHECK_POLYGLOT_8V, binary, true);

    assert!(
        record.verification.check_pass.unwrap_or(false) || record.verification.build_pass.unwrap_or(false),
        "Agent did not fix the issues (cargo check/clippy failed)"
    );
}

// ── Experiments (N=3 per condition) ──────────────────────────────────────────

#[test]
#[ignore = "experiment: 3 conditions × 3 runs (~9 min, costs tokens)"]
fn experiment_fix_test() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TEST, binary);

    // Every condition must actually fix the bug
    assert!(result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs", result.control.tests_pass_count(), result.n);
    for sample in &result.treatments {
        assert!(sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs", sample.description, sample.tests_pass_count(), result.n);
    }
}

#[test]
#[ignore = "experiment: 2 conditions × 3 runs (~6 min, costs tokens)"]
fn experiment_diagnose() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_DIAGNOSE, binary);

    // Agent must fix the issues — cargo check/clippy must pass
    assert!(result.control.check_pass_count() > 0,
        "Control failed to fix issues in {}/{} runs", result.control.check_pass_count(), result.n);
    for sample in &result.treatments {
        assert!(sample.check_pass_count() > 0,
            "{} failed to fix issues in {}/{} runs", sample.description, sample.check_pass_count(), result.n);
    }
}

#[test]
#[ignore = "experiment: 3 conditions × 3 runs (~27 min, costs tokens)"]
fn experiment_check_polyglot() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_CHECK_POLYGLOT, binary);

    // All conditions must fix the issues — cargo check/clippy or build must pass
    assert!(result.control.check_pass_count() > 0 || result.control.build_pass_count() > 0,
        "Control failed to fix issues in {}/{} runs", result.control.check_pass_count(), result.n);
    for sample in &result.treatments {
        assert!(sample.check_pass_count() > 0 || sample.build_pass_count() > 0,
            "{} failed to fix issues in {}/{} runs", sample.description, sample.check_pass_count(), result.n);
    }
}
