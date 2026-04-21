// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

pub static FIX_FAILING_TEST: Task = Task {
    name: "fix-failing-test",
    fixture: "agent-benchmark/fix-test-rust",
    prompt: "The test test_sum_range_inclusive is failing. Find the bug and fix it.",
    variables: &[],
};

pub static FIX_TEST_BASELINE: Scenario =
    Scenario::claude_baseline("fix-test-baseline", &FIX_FAILING_TEST);
pub static FIX_TEST_8V: Scenario = Scenario::claude_with_8v("fix-test-8v", &FIX_FAILING_TEST);

// Codex variants (experimental agent; paired separately in EXPERIMENT_FIX_TEST_CODEX)
pub static FIX_TEST_CODEX_BASELINE: Scenario =
    Scenario::codex_baseline("fix-test-codex-baseline", &FIX_FAILING_TEST);
pub static FIX_TEST_CODEX_8V: Scenario =
    Scenario::codex_with_8v("fix-test-codex-8v", &FIX_FAILING_TEST);

pub static EXPERIMENT_FIX_TEST: ExperimentConfig = ExperimentConfig {
    name: "fix-failing-test",
    task: &FIX_FAILING_TEST,
    control: &FIX_TEST_BASELINE,
    treatments: &[&FIX_TEST_8V],
    n: 6,
};

pub static EXPERIMENT_FIX_TEST_CODEX: ExperimentConfig = ExperimentConfig {
    name: "fix-failing-test-codex",
    task: &FIX_FAILING_TEST,
    control: &FIX_TEST_CODEX_BASELINE,
    treatments: &[&FIX_TEST_CODEX_8V],
    n: 3,
};
