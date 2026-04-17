// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Fixture preflight gate — rejects benchmark fixtures that are already "green".
//!
//! Called once per experiment, before any observation runs. Materializes the
//! control scenario's fixture into a scratch dir, runs the same verification
//! gates (`cargo test`, `cargo clippy -D warnings`, `cargo build`) that the
//! benchmark uses, and panics if ALL applicable gates already pass.
//!
//! A green fixture is a harness bug: the agent has nothing to fix, so any
//! measured delta is noise. See docs/design/fixture-preflight-gate.md.

use super::pipeline::run_verification;
use super::types::Scenario;
use crate::scaffold::{fixture_path, TempProject};

/// Materialize the scenario's fixture and verify its initial state.
///
/// Panics if all applicable verification gates pass — a green fixture means
/// the agent has nothing to fix, and any measurement would be pure noise.
pub fn preflight_fixture(scenario: &Scenario) {
    let fixture = fixture_path("o8v", scenario.task.fixture);
    let project = TempProject::from_fixture(&fixture);
    let verification = run_verification(project.path(), "");

    eprintln!(
        "[benchmark] preflight `{}` (fixture: `{}`): tests={:?} check={:?} build={:?}",
        scenario.name,
        scenario.task.fixture,
        verification.tests_pass,
        verification.check_pass,
        verification.build_pass,
    );

    let tests_pass = matches!(verification.tests_pass, Some(true));
    let check_pass = matches!(verification.check_pass, Some(true));
    let build_pass = matches!(verification.build_pass, Some(true));

    let applicable_gates: Vec<bool> = [
        verification.tests_pass.map(|_| tests_pass),
        verification.check_pass.map(|_| check_pass),
        verification.build_pass.map(|_| build_pass),
    ]
    .into_iter()
    .flatten()
    .collect();

    let all_pass = !applicable_gates.is_empty() && applicable_gates.iter().all(|&p| p);

    if all_pass {
        panic!(
            "[benchmark] preflight FAILED: fixture `{}` (scenario `{}`) already passes \
             all applicable verification gates (tests={:?}, check={:?}, build={:?}). \
             A benchmark on a green fixture measures noise, not tool behavior. \
             Rebuild the fixture with real violations the verifier detects, or change \
             the task shape. Pin the toolchain with rust-toolchain.toml so lint drift \
             doesn't silently flip a red fixture green. \
             See docs/design/fixture-preflight-gate.md.",
            scenario.task.fixture,
            scenario.name,
            verification.tests_pass,
            verification.check_pass,
            verification.build_pass,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{Agent, Environment, Scenario, Task};
    use super::*;

    // Fixture with a known failing test (`test_sum_range_inclusive`). Its
    // build+check gates typically pass, but the test gate fails — so preflight
    // must accept it.
    static HAPPY_TASK: Task = Task {
        name: "preflight-happy",
        fixture: "agent-benchmark/fix-test-rust",
        prompt: "",
        variables: &[],
    };
    static HAPPY_SCENARIO: Scenario = Scenario {
        name: "preflight-happy",
        description: "preflight happy path",
        task: &HAPPY_TASK,
        env: Environment {
            agent: Agent::Claude,
            setup_8v: false,
            permission_mode: None,
        },
    };

    // Fixture known to pass every gate. Preflight must reject it.
    static GREEN_TASK: Task = Task {
        name: "preflight-green",
        fixture: "agent-benchmark/clean-rust",
        prompt: "",
        variables: &[],
    };
    static GREEN_SCENARIO: Scenario = Scenario {
        name: "preflight-green",
        description: "preflight green fixture",
        task: &GREEN_TASK,
        env: Environment {
            agent: Agent::Claude,
            setup_8v: false,
            permission_mode: None,
        },
    };

    #[test]
    #[ignore = "runs cargo test/clippy/build — slow, opt-in"]
    fn preflight_accepts_fixture_with_failing_gate() {
        preflight_fixture(&HAPPY_SCENARIO);
    }

    #[test]
    #[ignore = "runs cargo test/clippy/build — slow, opt-in"]
    #[should_panic(expected = "preflight FAILED")]
    fn preflight_rejects_green_fixture() {
        preflight_fixture(&GREEN_SCENARIO);
    }
}
