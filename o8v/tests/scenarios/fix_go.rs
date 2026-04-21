// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

pub static FIX_GO: Task = Task {
    name: "fix-go",
    fixture: "agent-benchmark/fix-go",
    prompt: "Some tests in this Go project are failing. Find the bugs and fix them.",
    variables: &[],
};

pub static FIX_GO_BASELINE: Scenario = Scenario::claude_baseline("fix-go-baseline", &FIX_GO);
pub static FIX_GO_8V: Scenario = Scenario::claude_with_8v("fix-go-8v", &FIX_GO);

pub static EXPERIMENT_FIX_GO: ExperimentConfig = ExperimentConfig {
    name: "fix-go",
    task: &FIX_GO,
    control: &FIX_GO_BASELINE,
    treatments: &[&FIX_GO_8V],
    n: 6,
};
