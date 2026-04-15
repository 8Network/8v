# Fixture Pre-flight Gate — Design Note

**Status:** FROZEN 2026-04-15, pending implementation. Adversarial review complete — see "Review findings" at bottom. Do NOT edit further without explicit approval.
**Author:** Claude, at Soheil's direction (autonomous loop, 2026-04-15).
**Scope:** benchmark harness only. One new function + one call site in `run_experiment`.

---

## The problem

Entry 18 (learnings log, 2026-04-15) discovered that `agent-benchmark/violated-rust`, the fixture used by the `diagnose` experiment, passes `cargo check`, `cargo clippy -- -D warnings`, and `cargo clippy -W pedantic` on the unmodified source. The fixture name is a lie.

Consequence: three benchmark entries (Entry 7, Entry 14, Entry 16) measured "how long does the agent take to conclude nothing is wrong?" instead of "how well does the agent diagnose real issues?" — and were retracted.

Broken fixtures are not a hypothetical risk. They shipped. A reader citing "+7.4% on diagnose" would have believed a result that is pure harness artifact. The gate is the structural fix that prevents the next fixture from doing the same.

## Non-goals

- Rebuilding the diagnose fixture (E2.50, separate work).
- Per-task-shape verification (P35, separate design).
- Auto-detection of fixture quality beyond "initial-state-vs-verifier".

## Design

Add one function, `preflight_fixture`, called once per experiment before the control arm runs. It:

1. Materializes the fixture in a scratch directory (same mechanism as `run_scenario`, minus the agent and events).
2. Runs the scenario's verifier (`run_verification` or a task-shape-aware equivalent, see below).
3. Panics if the fixture passes all three gates.

Panic message:

> "fixture `{fixture}` already passes all verification gates (tests+check+build). A benchmark on a green fixture measures noise, not tool behavior. Either (a) fix the fixture to have real violations the verifier detects, or (b) change the task shape to one where a green start is correct (e.g. report-only). See docs/design/fixture-preflight-gate.md."

### Task-shape sensitivity

Current `run_verification` is one-size-fits-all: tests + clippy + build. For the three existing tasks:

| Task       | Shape    | Expected initial state                     | Gate behavior |
|------------|----------|--------------------------------------------|---------------|
| fix-test   | targeted | `cargo test` fails (off-by-one)            | Accept — at least one fails |
| check-polyglot | report | clippy fails (violations present)         | Accept — at least one fails |
| diagnose   | targeted | should have clippy violations              | **Reject — all pass** |

The gate is correct as "reject if ALL pass at start" for all three. No task-shape branching needed yet.

Future task shapes (report-only with no verifiable failure, e.g. "summarize this codebase") need per-shape logic. Out of scope for this change.

### Where it lives

`o8v-testkit/src/benchmark/experiment.rs::run_experiment`, immediately after the `EXPERIMENT:` header, before `run_sample(config.control, ...)`:

```rust
preflight_fixture(config.control);
```

Implementation file: `o8v-testkit/src/benchmark/preflight.rs` (new).

### The function

```rust
pub fn preflight_fixture(scenario: &Scenario) {
    let tempdir = /* same setup as run_scenario, minus claude + events */;
    let verification = run_verification(tempdir.path(), "");
    let all_pass = matches!(verification.tests_pass, Some(true))
        && matches!(verification.check_pass, Some(true))
        && matches!(verification.build_pass, Some(true));
    if all_pass {
        panic!(
            "[benchmark] fixture `{}` already passes verification gates. \
             Rebuild the fixture with real violations. See docs/design/fixture-preflight-gate.md.",
            scenario.task.fixture
        );
    }
    eprintln!("[benchmark] preflight: fixture `{}` has failing gate(s) as expected", scenario.task.fixture);
}
```

### Cost

One extra verification run per experiment. For rust projects, ~3-5 s. Negligible compared to a ~6-9 min benchmark.

## Test plan

1. **Unit test — happy path.** Pass a `Scenario` whose fixture is `rust-violated` (fix-test fixture). `preflight_fixture` returns without panic.
2. **Unit test — panics on green fixture.** Pass a `Scenario` whose fixture is `clean-rust` (already exists, known-passing). Assert panic with the expected message.
3. **Integration — sweep still runs.** After landing the gate, re-run `experiment_fix_test` and `experiment_check_polyglot`. Both must still execute end-to-end without the gate tripping.
4. **Integration — diagnose is blocked.** Re-run `experiment_diagnose` against the current `violated-rust` fixture. Expect panic at preflight. Once a real fixture ships (E2.50), this test flips to "must pass preflight".

## Open questions (review needed)

1. **Should the gate also verify *expected* post-fix state?** E.g., for fix-test, after applying a golden-fix patch, all gates must pass. Extra safety, extra mechanism. Defer?
2. **Print-only vs panic?** A print-only warning is less disruptive but lets broken fixtures ship. Panic is consistent with rule #4 (no silent fallbacks).
3. **Fixture-shape annotation?** Should `Task` gain a `expected_initial: FailingGates` field (which of tests/check/build must fail)? More explicit, more boilerplate. Defer until we have a fixture where "one gate fails" is the correct state but the current "any fails" logic is too permissive.

## Not in this change

- Rebuilding violated-rust (E2.50).
- Recording the initial-state verification in each observation's NDJSON (useful audit trail, but separate).

---

When Soheil approves, implement as described and land behind the same gate as the structured-report pipeline (both pending review).

---

## Review findings (2026-04-15)

Adversarial agent review surfaced gaps. Address before implementation:

1. **"Any gate fails" is necessary but not sufficient.** A diagnose fixture with a broken doctest and zero clippy violations passes the gate but still measures noise. Mitigation: log *which* gate(s) failed in the eprintln; tighten the panic message to name the task shape ("diagnose/targeted-fix fixtures") rather than implying universality.
2. **Promote Open Question 3 to v1.** Add `Task.expected_initial: FailingGates` (bitset of which of tests/check/build must fail) as a data-only change. Even if implementation defers the per-gate enforcement, declaring intent per fixture prevents the next coincidental-pass bug.
3. **Restrict preflight to deterministic gates.** `cargo test` can flake on timing/network. Run preflight against `cargo check` and `cargo build` only, or run the test gate twice and require both to fail.
4. **Toolchain pinning is a prerequisite, not optional.** Clippy lints change across rustc versions. A fixture that fails clippy today may pass on a newer rustc, silently flipping to green. Document `rust-toolchain.toml` per fixture as a hard requirement in the gate's panic message.
5. **Report-shape false positive.** A future report task ("summarize this codebase") with a clean fixture would trip the gate incorrectly. Acceptable for the current three tasks; document the constraint.
