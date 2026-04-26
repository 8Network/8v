// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

pub static DIAGNOSE_ISSUES: Task = Task {
    name: "diagnose-issues",
    fixture: "agent-benchmark/diagnose-rust",
    prompt: "There's something wrong with my code. Find the issues and fix them.",
    variables: &[],
};

pub static DIAGNOSE_BASELINE: Scenario =
    Scenario::claude_baseline("diagnose-baseline", &DIAGNOSE_ISSUES);
pub static DIAGNOSE_8V: Scenario = Scenario::claude_with_8v("diagnose-8v", &DIAGNOSE_ISSUES);

pub static EXPERIMENT_DIAGNOSE: ExperimentConfig = ExperimentConfig {
    name: "diagnose-issues",
    task: &DIAGNOSE_ISSUES,
    control: &DIAGNOSE_BASELINE,
    treatments: &[&DIAGNOSE_8V],
    n: 3,
};
