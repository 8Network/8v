// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark tasks and scenarios.
//!
//! A Task is "what to do" — fixture + prompt.
//! A Scenario is "how" — task + environment configuration.
//!
//! Each scenario maps to a test in agent_benchmark.rs.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

// ── Tasks ────────────────────────────────────────────────────────────────────
// Task prompts are what a real user would say. No tool names, no workflow
// instructions, no hints about how to work. Just the intent.

pub static FIX_FAILING_TEST: Task = Task {
    name: "fix-failing-test",
    fixture: "agent-benchmark/fix-test-rust",
    prompt: "The test test_sum_range_inclusive is failing. Find the bug and fix it.",
    variables: &[],
};

pub static DIAGNOSE_ISSUES: Task = Task {
    name: "diagnose-issues",
    fixture: "agent-benchmark/diagnose-rust",
    prompt: "There's something wrong with my code. Find the issues and fix them.",
    variables: &[],
};

pub static FIX_PYTHON_TRAVERSAL: Task = Task {
    name: "fix-python-traversal",
    fixture: "agent-benchmark/fix-test-python",
    prompt: "Some tests in this Python project are failing. Find the bugs and fix them.",
    variables: &[],
};

pub static FIX_GO: Task = Task {
    name: "fix-go",
    fixture: "agent-benchmark/fix-go",
    prompt: "Some tests in this Go project are failing. Find the bugs and fix them.",
    variables: &[],
};

pub static FIX_TYPESCRIPT: Task = Task {
    name: "fix-typescript",
    fixture: "agent-benchmark/fix-typescript",
    prompt: "Some tests in this TypeScript project are failing. Find the bugs and fix them.",
    variables: &[],
};

// ── Scenarios ────────────────────────────────────────────────────────────────
// Scenarios are declared as (baseline, treatment) pairs using the const-fn
// constructors on `Scenario`. This is the only environment knob — setup_8v
// on/off — because that is the only variable we measure.

// Fix-failing-test
pub static FIX_TEST_BASELINE: Scenario =
    Scenario::claude_baseline("fix-test-baseline", &FIX_FAILING_TEST);
pub static FIX_TEST_8V: Scenario = Scenario::claude_with_8v("fix-test-8v", &FIX_FAILING_TEST);

// Diagnose-issues
pub static DIAGNOSE_BASELINE: Scenario =
    Scenario::claude_baseline("diagnose-baseline", &DIAGNOSE_ISSUES);
pub static DIAGNOSE_8V: Scenario = Scenario::claude_with_8v("diagnose-8v", &DIAGNOSE_ISSUES);

// Fix-python-traversal
pub static FIX_PYTHON_BASELINE: Scenario =
    Scenario::claude_baseline("fix-python-baseline", &FIX_PYTHON_TRAVERSAL);
pub static FIX_PYTHON_8V: Scenario =
    Scenario::claude_with_8v("fix-python-8v", &FIX_PYTHON_TRAVERSAL);

// Fix-go
pub static FIX_GO_BASELINE: Scenario = Scenario::claude_baseline("fix-go-baseline", &FIX_GO);
pub static FIX_GO_8V: Scenario = Scenario::claude_with_8v("fix-go-8v", &FIX_GO);

// Fix-typescript
pub static FIX_TS_BASELINE: Scenario =
    Scenario::claude_baseline("fix-ts-baseline", &FIX_TYPESCRIPT);
pub static FIX_TS_8V: Scenario = Scenario::claude_with_8v("fix-ts-8v", &FIX_TYPESCRIPT);

// Codex
pub static FIX_TEST_CODEX_BASELINE: Scenario =
    Scenario::codex_baseline("fix-test-codex-baseline", &FIX_FAILING_TEST);
pub static FIX_TEST_CODEX_8V: Scenario =
    Scenario::codex_with_8v("fix-test-codex-8v", &FIX_FAILING_TEST);

// ── Experiments ──────────────────────────────────────────────────────────────

pub static EXPERIMENT_FIX_TEST: ExperimentConfig = ExperimentConfig {
    name: "fix-failing-test",
    task: &FIX_FAILING_TEST,
    control: &FIX_TEST_BASELINE,
    treatments: &[&FIX_TEST_8V],
    n: 6,
};

pub static EXPERIMENT_DIAGNOSE: ExperimentConfig = ExperimentConfig {
    name: "diagnose-issues",
    task: &DIAGNOSE_ISSUES,
    control: &DIAGNOSE_BASELINE,
    treatments: &[&DIAGNOSE_8V],
    n: 3,
};

pub static EXPERIMENT_FIX_PYTHON: ExperimentConfig = ExperimentConfig {
    name: "fix-python-traversal",
    task: &FIX_PYTHON_TRAVERSAL,
    control: &FIX_PYTHON_BASELINE,
    treatments: &[&FIX_PYTHON_8V],
    n: 6,
};

pub static EXPERIMENT_FIX_GO: ExperimentConfig = ExperimentConfig {
    name: "fix-go",
    task: &FIX_GO,
    control: &FIX_GO_BASELINE,
    treatments: &[&FIX_GO_8V],
    n: 6,
};

pub static EXPERIMENT_FIX_TS: ExperimentConfig = ExperimentConfig {
    name: "fix-typescript",
    task: &FIX_TYPESCRIPT,
    control: &FIX_TS_BASELINE,
    treatments: &[&FIX_TS_8V],
    n: 6,
};

pub static EXPERIMENT_FIX_TEST_CODEX: ExperimentConfig = ExperimentConfig {
    name: "fix-failing-test-codex",
    task: &FIX_FAILING_TEST,
    control: &FIX_TEST_CODEX_BASELINE,
    treatments: &[&FIX_TEST_CODEX_8V],
    n: 3,
};
