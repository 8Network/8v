// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_testkit::benchmark::{ExperimentConfig, Scenario, Task};

pub static FIX_PYTHON_TRAVERSAL: Task = Task {
    name: "fix-python-traversal",
    fixture: "agent-benchmark/fix-test-python",
    prompt: "Some tests in this Python project are failing. Find the bugs and fix them. Use `make test` to run tests.",
    variables: &[],
};

pub static FIX_PYTHON_BASELINE: Scenario =
    Scenario::claude_baseline("fix-python-baseline", &FIX_PYTHON_TRAVERSAL);
pub static FIX_PYTHON_8V: Scenario =
    Scenario::claude_with_8v("fix-python-8v", &FIX_PYTHON_TRAVERSAL);

pub static EXPERIMENT_FIX_PYTHON: ExperimentConfig = ExperimentConfig {
    name: "fix-python-traversal",
    task: &FIX_PYTHON_TRAVERSAL,
    control: &FIX_PYTHON_BASELINE,
    treatments: &[&FIX_PYTHON_8V],
    n: 6,
};
