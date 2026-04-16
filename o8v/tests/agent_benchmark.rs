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

mod scenarios;

use o8v_testkit::benchmark::{run_experiment, run_scenario, Verification};

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

/// At least one of several gates must be `Some(true)`. Used for polyglot
/// fixtures where any green gate counts as success.
#[track_caller]
fn require_any_pass(v: &Verification, gates: &[(&str, Option<bool>)]) {
    if gates.iter().any(|(_, r)| *r == Some(true)) {
        return;
    }
    let report: Vec<String> = gates
        .iter()
        .map(|(n, r)| format!("{n}={:?}", r))
        .collect();
    panic!(
        "no gate passed in {report:?} — full verification: {v:?}",
        report = report.join(", "),
    );
}

// ── Fix failing test ─────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TEST_BASELINE, binary, true);

    require_pass("cargo test", record.verification.tests_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn fix_test_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::FIX_TEST_8V, binary, true);

    require_pass("cargo test", record.verification.tests_pass);
}

// ── Diagnose issues ──────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::DIAGNOSE_BASELINE, binary, true);

    require_pass("cargo clippy", record.verification.check_pass);
}

#[test]
#[ignore = "requires: `claude` in PATH (~60s, costs tokens)"]
fn diagnose_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::DIAGNOSE_8V, binary, true);

    require_pass("cargo clippy", record.verification.check_pass);
}

// ── Check polyglot ───────────────────────────────────────────────────────────

#[test]
#[ignore = "requires: `claude` in PATH, polyglot toolchains (~180s, costs tokens)"]
fn check_polyglot_baseline() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::CHECK_POLYGLOT_BASELINE, binary, true);

    require_any_pass(&record.verification, &[
        ("cargo clippy", record.verification.check_pass),
        ("cargo build", record.verification.build_pass),
    ]);
}

#[test]
#[ignore = "requires: `claude` in PATH, polyglot toolchains (~180s, costs tokens)"]
fn check_polyglot_8v() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let record = run_scenario(&scenarios::CHECK_POLYGLOT_8V, binary, true);

    require_any_pass(&record.verification, &[
        ("cargo clippy", record.verification.check_pass),
        ("cargo build", record.verification.build_pass),
    ]);
}

// ── Experiments (N=3 per condition) ──────────────────────────────────────────

#[test]
#[ignore = "experiment: baseline + 8v × N runs (~9 min, costs tokens)"]
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
#[ignore = "experiment: 2 conditions × 3 runs (~9 min, costs tokens)"]
fn experiment_fix_python_traversal() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_PYTHON, binary);

    // Every condition must actually fix the bugs — pytest must pass
    assert!(result.control.tests_pass_count() == result.n,
        "Control failed to fix bug in {}/{} runs", result.control.tests_pass_count(), result.n);
    for sample in &result.treatments {
        assert!(sample.tests_pass_count() == result.n,
            "{} failed to fix bug in {}/{} runs", sample.description, sample.tests_pass_count(), result.n);
    }
}

#[test]
#[ignore = "experiment: baseline + 8v × N runs (~27 min, costs tokens)"]
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

// ── Codex experiments ──────────────────────────────────────────────────────

#[test]
#[ignore = "agent benchmark — requires codex CLI, costs real API credits"]
fn experiment_fix_test_codex() {
    let binary = env!("CARGO_BIN_EXE_8v");
    let result = run_experiment(&scenarios::EXPERIMENT_FIX_TEST_CODEX, binary);

    assert!(result.control.tests_pass_count() > 0,
        "Codex control failed to fix the test in {}/{} runs", result.control.tests_pass_count(), result.n);
    for sample in &result.treatments {
        assert!(sample.tests_pass_count() > 0,
            "{} failed to fix the test in {}/{} runs", sample.description, sample.tests_pass_count(), result.n);
    }
}
