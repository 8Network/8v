// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark tasks and scenarios.
//!
//! A Task is "what to do" — fixture + prompt.
//! A Scenario is "how" — task + environment configuration.
//!
//! Each scenario maps to a test in agent_benchmark.rs.

use o8v_testkit::benchmark::{Environment, ExperimentConfig, Scenario, Task};

// ── Tasks ────────────────────────────────────────────────────────────────────

/// Fix a failing test — off-by-one bug in sum_range.
/// The agent must find the bug, fix it, and verify with cargo test.
pub static FIX_FAILING_TEST: Task = Task {
    name: "fix-failing-test",
    fixture: "agent-benchmark/fix-test-rust",
    prompt: "The test test_sum_range_inclusive is failing. Find the bug and fix it. \
             Run the tests to verify your fix works.",
    variables: &[],
};

/// Diagnose code issues — clippy violation (needless &mut).
/// The agent must identify the issue without being told what's wrong.
pub static DIAGNOSE_ISSUES: Task = Task {
    name: "diagnose-issues",
    fixture: "agent-benchmark/diagnose-rust",
    prompt: "There's something wrong with my code. Find the issues and fix them.",
    variables: &[],
};

/// Fix a failing Python test — safe_join has three real security bugs
/// (parent-traversal via `..` components, absolute-path component escape,
/// false-positive on filenames containing `..`). The agent must find and
/// fix them so pytest passes.
pub static FIX_PYTHON_TRAVERSAL: Task = Task {
    name: "fix-python-traversal",
    fixture: "agent-benchmark/fix-test-python",
    prompt: "Some tests in this Python project are failing. Find the bugs, fix them, \
             and run pytest to verify all tests pass.",
    variables: &[],
};

/// Check a polyglot project for issues across all stacks.
/// The agent must discover multiple stacks and report all violations.
pub static CHECK_POLYGLOT: Task = Task {
    name: "check-polyglot",
    fixture: "agent-benchmark/check-polyglot",
    prompt: "Check this entire project for issues across all stacks. \
             Report everything that needs to be fixed — code quality, \
             formatting, type errors, lint warnings.",
    variables: &[],
};

// ── Environments ─────────────────────────────────────────────────────────────

/// Baseline: native tools available, MCP registered but agent chooses freely.
const BASELINE_ENV: Environment = Environment {
    setup_8v: false,
    permission_mode: "acceptEdits",
    blocked_tools: &[],
    extra_env: &[],
    claude_md: Some("# Project\n\n\
    Use native tools to complete the task.\n\n\
    ## Command Discovery\n\n\
    Start by understanding the project structure and available files:\n\n\
    - `Glob` with pattern `**/*` — list all files in the project.\n\
    - `Glob` with pattern `**/*.rs` — find files by extension.\n\
    - `Glob` with pattern `**/Cargo.toml` — find project manifests.\n\
    - `Bash` with `find . -name \"*.toml\"` — find files by name.\n\n\
    ## Reading Files\n\n\
    Read files to understand code structure before making changes:\n\n\
    - `Read` — read a full file or a specific line range.\n\
    - `Read` with `offset` and `limit` — read only the lines you need.\n\
    - Start with small reads to locate relevant sections, then expand.\n\n\
    ## Searching Files\n\n\
    Search across the codebase to find patterns, usages, and related code:\n\n\
    - `Grep` with a pattern — find content across files.\n\
    - `Grep` with `glob` filter — restrict to specific file types (e.g. `*.rs`).\n\
    - `Grep` with `-i` — case-insensitive search.\n\
    - `Grep` with context lines — see surrounding code.\n\n\
    ## Writing Files\n\n\
    Make targeted changes to files:\n\n\
    - `Edit` — replace specific text in a file (preferred for targeted changes).\n\
    - `Write` — write an entire file (use only when creating new files).\n\
    - Always `Read` the file before editing to confirm the exact text to replace.\n\n\
    ## Building\n\n\
    Compile the project with cargo:\n\n\
    - `Bash` with `cargo build` — build the project.\n\
    - `Bash` with `cargo build --release` — optimized build.\n\
    - `Bash` with `cargo check` — fast type-check without producing a binary.\n\n\
    ## Running Commands\n\n\
    Execute any command via Bash:\n\n\
    - `Bash` with `cargo run -- <args>` — run the project binary.\n\
    - `Bash` with any shell command — grep, find, cat, etc.\n\n\
    ## Testing and Verification\n\n\
    Run tests and checks to verify your work:\n\n\
    - `Bash` with `cargo test` — run all tests.\n\
    - `Bash` with `cargo test <name>` — run a specific test.\n\
    - `Bash` with `cargo clippy -- -D warnings` — lint (all warnings are errors).\n\
    - `Bash` with `cargo fmt` — auto-format all files.\n\
    - `Bash` with `cargo fmt --check` — check formatting without modifying files.\n\n\
    ## Recommended Workflow\n\n\
    1. `Glob` `**/*` — list all files to understand the project layout.\n\
    2. `Glob` `**/Cargo.toml` — find Rust project manifests.\n\
    3. `Grep` for relevant function or struct names — locate the code to change.\n\
    4. `Read` the relevant file — understand context before editing.\n\
    5. `Edit` — make your change with precision.\n\
    6. `Bash` `cargo test` — verify correctness.\n\
    7. `Bash` `cargo build` — confirm it compiles.\n\
    8. `Bash` `cargo clippy -- -D warnings` — run lint checks.\n\
    9. `Bash` `cargo fmt` — format before declaring done.\n"),
};

/// 8v available: full `8v init --yes`, native tools also available.
/// Measures whether the agent chooses 8v when both options exist.
const WITH_8V_ENV: Environment = Environment {
    setup_8v: true,
    permission_mode: "acceptEdits",
    blocked_tools: &[],
    extra_env: &[],
    claude_md: None, // 8v init writes CLAUDE.md
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
