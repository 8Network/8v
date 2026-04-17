# Design — Per-language benchmarks (replacing polyglot)

**Status:** DRAFT — awaiting review.
**Date:** 2026-04-16.
**Author:** Claude (per founder direction).

## Problem

The current `check-polyglot` task is fundamentally the wrong shape. The
founder's point, verbatim:

> "The goal is not to benchmark the tool coverage. Our goal is to
> benchmark the efficiency of token usage. We need to make a task with
> the project. One Rust project, one TypeScript, one Python. Different
> languages. We can run our benchmark per language. The polyglot here
> is not beneficial. The fundamental issue is the entire scenario. The
> solution to that requirement is wrong."

"Polyglot" conflates two different things: *tool coverage* (does the
tool detect and run the right checker for each stack?) and *token
efficiency* (does the agent spend fewer tokens getting to the fix?).
A benchmark that mixes both measures neither.

### Evidence from dogfooding (2026-04-16)

- `8v ls` on the 6-stack fixture detects **3 of 6** stacks (rust,
  python, dockerfile). Go, TS, Terraform are silently dropped even
  with the tools installed. The 8v condition literally runs fewer
  checks than the baseline.
- Native baseline N=3: 428K / 107K / 83K tokens. CV ≈95%. Signal is
  noise at this variance.
- The scenario prompt enumerates all six bugs by stack — a roadmap,
  not a task. Eliminates discovery, which is the phase 8v's batching
  is supposed to shine on.

## Goal

Measure per-language token efficiency on realistic developer tasks.
One fixture per language, one task each, measured independently.
Results reported per language — never aggregated into a single
"polyglot number."

## Principles

1. **One language per scenario.** No multi-stack fixtures. If we want
   a TypeScript number, run a TypeScript task on a TypeScript project.
2. **Problem-shaped prompt.** "CI is failing, find and fix" or "this
   test fails, fix it." Never enumerate the bugs.
3. **Small fixture.** File-read overhead must not dominate signal. A
   few hundred lines total is plenty.
4. **Deterministic exit-code gate.** Same post-hoc command run against
   both baseline and 8v conditions. `cargo test` / `pytest` /
   `tsc --noEmit` / `go test`. Exit 0 = pass.
5. **Ecosystem-native tools only.** Each scenario uses whatever the
   language community uses. No hadolint, no tflint — those measure
   infra tools, not code-editing efficiency.

## Proposed scenarios

| Language   | Scenario              | Status    | Task shape                |
|------------|-----------------------|-----------|---------------------------|
| Rust       | fix-failing-test      | existing  | failing unit test         |
| Rust       | diagnose-issues       | existing  | broken build + lint       |
| Python     | fix-python-traversal  | existing  | failing pytest            |
| TypeScript | fix-ts-* (new)        | to design | failing tsc + test        |
| Go         | fix-go-* (new)        | to design | failing go test / go vet  |

Rust and Python are already per-language. TypeScript and Go are net
new per-language scenarios. Each new scenario gets its own design doc
before a fixture is built.

## What's cut

- **`check-polyglot` scenario** (task, scenario pair, experiment).
- **Fixture** at `o8v/tests/fixtures/agent-benchmark/check-polyglot/`.
- **"Polyglot" framing** in reports, memory, and docs. We measure
  per-language and say so.
- **Dockerfile / Terraform** as benchmark targets. Scope is
  code-editing efficiency, not infra-tool coverage.

## Success gate per scenario

One command. Same for both baseline and 8v conditions. Exit code is
the pass/fail signal:

| Language   | Gate command                      |
|------------|-----------------------------------|
| Rust       | `cargo test`                      |
| Python     | `pytest`                          |
| TypeScript | `tsc --noEmit && <test runner>`   |
| Go         | `go test ./... && go vet ./...`   |

The harness already runs post-hoc gates for rust/python. TS/Go add
two more — straightforward.

## Reporting

Per-language table. Never one headline "polyglot" number.

| Scenario   | Condition | Tokens | Cost | Turn-1 cc | Pass rate |
|------------|-----------|-------:|-----:|----------:|----------:|

Compare baseline vs 8v *within* a language. Do not compute a
cross-language mean.

## Open questions

- **TypeScript dependencies.** Does the fixture need `npm install`?
  That adds seconds and variance. Options: (a) vendor `node_modules`
  into the fixture, (b) use only `tsc` against a no-deps file, (c)
  use Deno/Bun with zero install. Lean toward (b) for first pass.
- **Go modules.** Vendor deps into the fixture to avoid network
  during the run. Same principle as rust/python: no network.
- **Scenario count per language.** Start with 1 each. Add a second
  shape only if N=1 leaves signal ambiguous.
- **TS and Go test runner.** Which one? Avoid heavy frameworks. For
  TS, consider `node --test` (built-in since 20) to avoid a runner
  dependency entirely. For Go, stdlib `go test` covers it.

## Non-goals

- Testing how many stacks 8v detects. That's a coverage check, not a
  benchmark. If coverage is broken, file a bug.
- Realism of cross-language work. Developers rarely touch 4 stacks in
  one task. A per-language benchmark is honest about what it measures.
- Building a polyglot fixture of any kind.

## Migration

1. Remove `CHECK_POLYGLOT`, `CHECK_POLYGLOT_BASELINE`, `CHECK_POLYGLOT_8V`,
   `EXPERIMENT_CHECK_POLYGLOT` from `o8v/tests/scenarios.rs`.
2. Remove `check_polyglot_baseline` / `check_polyglot_8v` /
   `experiment_check_polyglot` from `o8v/tests/agent_benchmark.rs`.
3. Delete fixture dir `o8v/tests/fixtures/agent-benchmark/check-polyglot/`.
4. Update memory: mark `polyglot_benchmark_invalid.md` as superseded
   by this design.
5. Design per-language TS + Go scenarios in separate design docs
   before building fixtures. No implementation until those designs are
   also reviewed.
