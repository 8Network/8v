// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark tasks and scenarios.
//!
//! A Task is "what to do" — fixture + prompt.
//! A Scenario is "how" — task + environment configuration.
//!
//! Each scenario maps to a test in agent_benchmark.rs.

use o8v_testkit::benchmark::{Agent, Environment, ExperimentConfig, Scenario, Task};
use o8v_testkit::benchmark::PermissionMode;

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

pub static CHECK_POLYGLOT: Task = Task {
    name: "check-polyglot",
    fixture: "agent-benchmark/check-polyglot",
    prompt: "The CI pipeline is failing across multiple components. The Rust backend has compiler warnings, the Go service has a type mismatch in a format call, the TypeScript frontend won't compile, the Python automation script fails the linter, the Dockerfile has a pinning warning, and the Terraform config is not properly formatted. Fix all of it so every check passes.",
    variables: &[],
};

// ── Environments ─────────────────────────────────────────────────────────────

/// Baseline: no 8v, no instructions. Raw agent with default tools.
const BASELINE_ENV: Environment = Environment {
    agent: Agent::Claude,
    setup_8v: false,
    permission_mode: Some(PermissionMode::AcceptEdits),
    blocked_tools: &[],
    extra_env: &[],
    claude_md: None,
};

/// 8v available: full `8v init --yes`, native tools also available.
/// Measures whether the agent chooses 8v when both options exist.
const WITH_8V_ENV: Environment = Environment {
    agent: Agent::Claude,
    setup_8v: true,
    permission_mode: Some(PermissionMode::AcceptEdits),
    blocked_tools: &[],
    extra_env: &[],
    claude_md: None, // 8v init writes CLAUDE.md
};

// ── Codex environments ──────────────────────────────────────────────────────

/// Codex baseline: no 8v, no instructions. Raw agent with default tools.
const CODEX_BASELINE_ENV: Environment = Environment {
    agent: Agent::Codex,
    setup_8v: false,
    permission_mode: None,
    blocked_tools: &[],
    extra_env: &[],
    claude_md: None,
};

/// Codex with 8v: MCP server registered, AGENTS.md instructs to use 8v.
const CODEX_WITH_8V_ENV: Environment = Environment {
    agent: Agent::Codex,
    setup_8v: true,
    permission_mode: None,
    blocked_tools: &[],
    extra_env: &[],
    claude_md: None, // 8v init writes AGENTS.md
};

// ── Scenarios ────────────────────────────────────────────────────────────────

// Fix-failing-test scenarios
pub static FIX_TEST_BASELINE: Scenario = Scenario {
    name: "fix-test-baseline",
    description: "Native",
    task: &FIX_FAILING_TEST,
    env: BASELINE_ENV,
};

pub static FIX_TEST_8V: Scenario = Scenario {
    name: "fix-test-8v",
    description: "With 8v",
    task: &FIX_FAILING_TEST,
    env: WITH_8V_ENV,
};

// Diagnose-issues scenarios
pub static DIAGNOSE_BASELINE: Scenario = Scenario {
    name: "diagnose-baseline",
    description: "Native",
    task: &DIAGNOSE_ISSUES,
    env: BASELINE_ENV,
};

pub static DIAGNOSE_8V: Scenario = Scenario {
    name: "diagnose-8v",
    description: "With 8v",
    task: &DIAGNOSE_ISSUES,
    env: WITH_8V_ENV,
};

// Fix-python-traversal scenarios
pub static FIX_PYTHON_BASELINE: Scenario = Scenario {
    name: "fix-python-baseline",
    description: "Native",
    task: &FIX_PYTHON_TRAVERSAL,
    env: BASELINE_ENV,
};

pub static FIX_PYTHON_8V: Scenario = Scenario {
    name: "fix-python-8v",
    description: "With 8v",
    task: &FIX_PYTHON_TRAVERSAL,
    env: WITH_8V_ENV,
};

// Check-polyglot scenarios
pub static CHECK_POLYGLOT_BASELINE: Scenario = Scenario {
    name: "check-polyglot-baseline",
    description: "Native",
    task: &CHECK_POLYGLOT,
    env: BASELINE_ENV,
};

pub static CHECK_POLYGLOT_8V: Scenario = Scenario {
    name: "check-polyglot-8v",
    description: "With 8v",
    task: &CHECK_POLYGLOT,
    env: WITH_8V_ENV,
};

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

pub static EXPERIMENT_CHECK_POLYGLOT: ExperimentConfig = ExperimentConfig {
    name: "check-polyglot",
    task: &CHECK_POLYGLOT,
    control: &CHECK_POLYGLOT_BASELINE,
    treatments: &[&CHECK_POLYGLOT_8V],
    n: 6,
};

// ── Codex scenarios ────────────────────────────────────────────────────────

pub static FIX_TEST_CODEX_BASELINE: Scenario = Scenario {
    name: "fix-test-codex-baseline",
    description: "Codex Native",
    task: &FIX_FAILING_TEST,
    env: CODEX_BASELINE_ENV,
};

pub static FIX_TEST_CODEX_8V: Scenario = Scenario {
    name: "fix-test-codex-8v",
    description: "Codex + 8v",
    task: &FIX_FAILING_TEST,
    env: CODEX_WITH_8V_ENV,
};

// ── Codex experiments ──────────────────────────────────────────────────────

pub static EXPERIMENT_FIX_TEST_CODEX: ExperimentConfig = ExperimentConfig {
    name: "fix-failing-test-codex",
    task: &FIX_FAILING_TEST,
    control: &FIX_TEST_CODEX_BASELINE,
    treatments: &[&FIX_TEST_CODEX_8V],
    n: 3,
};
