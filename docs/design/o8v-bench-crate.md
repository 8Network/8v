# Design — `o8v-bench` crate

**Status:** DRAFT — awaiting review.
**Author:** Claude (per founder direction, 2026-04-16).
**Owner:** Soheil.

## Problem

Benchmark infrastructure lives in `o8v-testkit::benchmark`. That's wrong for
three reasons:

1. **Wrong crate.** `o8v-testkit` is meant to be generic test infra (fixture
   scaffolding, safe-fs helpers). The benchmark module is ~3000 lines of
   a completely different concern — running agent CLIs, collecting events,
   rendering reports — and it dominates the crate.
2. **No public API.** Scenarios are currently plain structs with public
   fields. There's no builder, no entry point, no sense of "how to use this
   as a library." You have to copy the `run_scenario`/`run_experiment`
   free functions and hope.
3. **Not shippable.** If a third party wants to benchmark their own
   AI-coding-tool MCP server against a baseline, they cannot take a
   dependency on `o8v-testkit` and just use it. The shape is "internal
   test helpers," not "public library."

Founder's goal: ship `o8v-bench` as a reusable library for benchmarking
AI coding agents. Clean structure, builder pattern, explicit execution —
"like running on a main function."

## Non-goals

- Changing the scenario definitions themselves. Our tasks
  (fix-failing-test, diagnose-issues, etc.) stay defined in
  `o8v/tests/scenarios.rs`. This doc is about the *library* they consume.
- Changing what is measured (tokens, cost, turns, verification gates,
  events, landmines). Those are settled.
- Adding new CLI commands. `8v` is feature-frozen (per memory).
- Supporting agents beyond Claude + Codex in v1.

## Crate layout

```
oss/8v/o8v-bench/
├── Cargo.toml
├── src/
│   ├── lib.rs              # re-exports
│   ├── task.rs             # Task struct
│   ├── scenario.rs         # Scenario + ScenarioBuilder
│   ├── environment.rs      # Agent, PermissionMode, Environment
│   ├── experiment.rs       # Experiment + ExperimentBuilder
│   ├── observation.rs      # Observation, Verification, TurnRecord, ToolCallDetail
│   ├── sample.rs           # Sample (stats over Observations)
│   ├── assertion.rs        # require_pass, require_any_pass (public helpers)
│   ├── driver/
│   │   ├── mod.rs          # Driver trait
│   │   ├── claude.rs       # Claude CLI driver
│   │   └── codex.rs        # Codex CLI driver
│   ├── verification.rs     # run_verification (cargo/pytest/...)
│   ├── events.rs           # events.ndjson collector
│   ├── pipeline.rs         # run_scenario — private orchestration (deleted public fn)
│   ├── report.rs           # ReportJson, ConditionReport, render_markdown
│   └── store.rs            # BenchmarkStore (persistence)
├── tests/
│   └── builder_api.rs      # acceptance tests for the public builder API
```

Dependencies: `serde`, `serde_json`, `toml`, `tempfile`, `comfy-table`,
`o8v-fs` (containment root), `o8v-core` (sanitize). **No dep on the `o8v`
CLI crate** — third parties should not have to pull in the full CLI.

## Public API

### Scenario — single run

```rust
use o8v_bench::{Scenario, Task};

static FIX_FAILING_TEST: Task = Task::new("fix-failing-test")
    .fixture("agent-benchmark/fix-test-rust")
    .prompt("The test test_sum_range_inclusive is failing. Fix it.");

fn main() {
    let scenario = Scenario::new("fix-test-8v")
        .task(&FIX_FAILING_TEST)
        .claude()
        .with_8v()
        .description("With 8v");

    let binary = "/usr/local/bin/8v";
    let observation = scenario.run(binary);

    assert_eq!(observation.verification.tests_pass, Some(true));
    println!("tokens: {}", observation.total_tokens);
}
```

Chainable state:
- `.task(&Task)` — required
- `.claude()` / `.codex()` — agent selection (required; no default)
- `.with_8v()` / `.without_8v()` — environment toggle
- `.permission_mode(mode)` — optional, defaults to `AcceptEdits` for
  Claude, `None` for Codex
- `.description(s)` — optional human label

Terminal methods:
- `.build() -> Scenario` — for static storage
- `.run(binary: &str) -> Observation` — execute once, persist, return
- `.run_silent(binary: &str) -> Observation` — execute without persist

### Experiment — N runs × M conditions

```rust
use o8v_bench::{Experiment, Scenario};

fn main() {
    let result = Experiment::new("fix-failing-test")
        .task(&FIX_FAILING_TEST)
        .control(Scenario::new("baseline").task(&FIX_FAILING_TEST).claude().without_8v())
        .treatment(Scenario::new("with-8v").task(&FIX_FAILING_TEST).claude().with_8v())
        .runs(6)
        .execute("/usr/local/bin/8v");

    let report = result.build_report();
    println!("{}", report.render_markdown());
}
```

### Assertions (for use in `#[test]` fns)

```rust
use o8v_bench::{require_pass, require_any_pass};

#[test]
fn fix_test_8v() {
    let obs = Scenario::new("fix-test-8v")
        .task(&FIX_FAILING_TEST).claude().with_8v()
        .run(binary());
    require_pass("cargo test", obs.verification.tests_pass);
}
```

### Task — const-constructible

```rust
pub struct Task {
    pub name: &'static str,
    pub fixture: &'static str,
    pub prompt: &'static str,
    pub variables: &'static [(&'static str, &'static str)],
}

impl Task {
    pub const fn new(name: &'static str) -> TaskBuilder { ... }
}
```

`TaskBuilder` is `const fn` all the way so tasks can be `static`.

## Module boundaries

| Concern            | Module            | Public? |
|--------------------|-------------------|---------|
| Task definition    | `task`            | yes     |
| Environment config | `environment`     | yes     |
| Scenario builder   | `scenario`        | yes     |
| Experiment builder | `experiment`      | yes     |
| Driver trait       | `driver`          | yes (trait only) |
| Claude/Codex impls | `driver::claude`, `driver::codex` | crate-private |
| Pipeline (setup→run→verify→persist) | `pipeline` | crate-private |
| Verification       | `verification`    | crate-private |
| Event collection   | `events`          | crate-private |
| Observation types  | `observation`     | yes     |
| Sample (stats)     | `sample`          | yes     |
| Report types       | `report`          | yes     |
| Persistence        | `store`           | yes     |
| Assertions         | `assertion`       | yes     |

Private modules are `pub(crate)` — third parties get a clean surface, we
keep freedom to refactor internals.

## What stays in `o8v-testkit`

- `scaffold::{TempProject, fixture_path}` — generic fixture helpers.
  `o8v-bench` depends on `o8v-testkit` for these.
- Anything not in `src/benchmark/`.

## What moves

Everything under `o8v-testkit/src/benchmark/` → `o8v-bench/src/`, split
per the layout above.

## Migration plan

**Step 1 — Create the crate.** New `o8v-bench/` with `Cargo.toml`,
`lib.rs`, empty modules. Add to workspace. Do not touch `o8v-testkit` yet.
Verify workspace still builds.

**Step 2 — Copy-and-split.** Copy `o8v-testkit/src/benchmark/*.rs` into
`o8v-bench/src/` and split per the layout (types.rs → task.rs +
scenario.rs + environment.rs + observation.rs + sample.rs; pipeline.rs
stays pipeline.rs + events.rs + verification.rs; claude.rs + codex.rs →
driver/). Keep the old copy under `o8v-testkit/src/benchmark/` untouched
so workspace builds.

**Step 3 — Add the builder API.** `Scenario::new().task(…).claude()
.with_8v()` etc. Const `Task::new(…)` constructor. Keep the existing
direct-struct-construction API so old scenarios still compile.

**Step 4 — Switch the consumer.** `o8v/tests/scenarios.rs` and
`o8v/tests/agent_benchmark.rs` switch imports from
`o8v_testkit::benchmark` to `o8v_bench`. Rewrite scenarios in the
builder style. Re-run all tests (full 1626-test suite).

**Step 5 — Delete the old module.** `o8v-testkit/src/benchmark/` goes
away. Any remaining re-exports in `o8v-testkit/src/lib.rs` are removed.
Verify no other crate referenced `o8v_testkit::benchmark`.

Each step is a commit. Revert is trivial at each step.

## Breaking-change surface

All breakage is internal to this workspace — `o8v-bench` has no external
users yet. `o8v-testkit::benchmark` has none either. So: zero external
breaking change.

## What's deliberately NOT in v1

- **Sweep type** (Item 6 from the prior plan). Comes after v1 ships and
  we have the builder shape settled. Sweeps are a higher-order
  abstraction over `Experiment`.
- **Parallel runs.** Benchmarks still run sequentially (events.ndjson
  is a global file). Fixing that is a separate piece of work.
- **Examples directory.** Until a third party asks, the `o8v/tests/`
  usage is the canonical example.

## Open questions

None blocking. Ready to implement on approval.

## Review checklist

Before implementing, the following must be true:

- [ ] Founder has read this doc
- [ ] Crate name `o8v-bench` confirmed
- [ ] Public API shape (builder signatures) approved
- [ ] Module layout approved
- [ ] Migration plan (5 steps, one commit each) approved
- [ ] No additional scope added after review
