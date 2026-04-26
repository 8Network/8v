// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

pub static FIX_TYPESCRIPT: Task = Task {
    name: "fix-typescript",
    fixture: "agent-benchmark/fix-typescript",
    prompt: "Some tests in this TypeScript project are failing. Find the bugs and fix them.",
    variables: &[],
};

pub static FIX_TS_BASELINE: Scenario =
    Scenario::claude_baseline("fix-ts-baseline", &FIX_TYPESCRIPT);
pub static FIX_TS_8V: Scenario = Scenario::claude_with_8v("fix-ts-8v", &FIX_TYPESCRIPT);

pub static EXPERIMENT_FIX_TS: ExperimentConfig = ExperimentConfig {
    name: "fix-typescript",
    task: &FIX_TYPESCRIPT,
    control: &FIX_TS_BASELINE,
    treatments: &[&FIX_TS_8V],
    n: 6,
};
