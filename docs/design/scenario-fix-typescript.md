# Design — fix-typescript scenario

**Status:** DRAFT — awaiting review.
**Date:** 2026-04-16.
**Parent:** `docs/design/polyglot-benchmark-redesign.md`

## Goal

Per-language token-efficiency benchmark for TypeScript, matching the
shape of `fix-failing-test` (rust) and `fix-python-traversal` (python).
One small fixture, one failing check, problem-shaped prompt.

## Fixture

Path: `o8v/tests/fixtures/agent-benchmark/fix-typescript/`
Size target: ~50–100 lines across 3 files. Small.
Contents:

```
fix-typescript/
├── package.json         # minimal, no deps
├── tsconfig.json        # strict, noEmit target
├── src/lib.ts           # module under test (contains the bug)
└── src/lib.test.ts      # failing test using node:test
```

No `node_modules`. No `npm install` step. The gate uses `tsc` only —
or `tsc` + `node --test` against compiled output if we keep a test.
See runner decision below.

## Task

```rust
pub static FIX_TYPESCRIPT: Task = Task {
    name: "fix-typescript",
    fixture: "agent-benchmark/fix-typescript",
    prompt: "Some tests in this TypeScript project are failing. Find the bugs and fix them.",
    variables: &[],
};
```

Prompt mirrors rust/python. Problem-shaped. No bug enumeration.

## Bug design

One seeded bug. Realistic shape — candidates:

- Off-by-one in array slice (caught by failing test).
- Wrong comparison operator (`<=` vs `<`).
- Null/undefined not handled on optional field (caught by `strict`
  mode in tsc + a test case hitting the path).

Preference: a bug that fails **both** `tsc --noEmit` (type-level) and
the test runner, so either gate catches it. This keeps the runner
choice decoupled from detection.

Discovery required — no README, no CLAUDE.md enumerating the bug.

## Success gate

Single deterministic command, same for baseline + 8v:

```
tsc --noEmit && node --test --import tsx src/lib.test.ts
```

Exit 0 = pass. Reported on `tests_pass` in `BenchmarkResult` (reuse
the existing field; TS gate covers both type check and test run).

### Runner decision

Options considered:
- (a) `node --test` only — builtin since Node 20, zero install. Needs
  TS transpile step or `.mjs` tests.
- (b) `vitest` — popular but requires `node_modules`.
- (c) `tsx` — compile-free TS exec; Node 20+ can also use
  `--experimental-strip-types` on 22+.
- (d) `tsc --noEmit` only — no test runner, type errors are the
  signal.

**Chosen:** (d) for first pass. Simplest gate, no Node-version
sensitivity, no install. Measures "can the agent fix a type error" —
a real and frequent TS skill.

If (d) leaves signal ambiguous at N=6, upgrade to (a) by adding a
`.test.mjs` test alongside the TS source.

## Preflight

Before running the scenario, verify:

- `tsc --version` resolves (either global, npx-available, or vendored).
- If option (a) ever adopted: `node --version` ≥ 20.

Preflight failures must error clearly — never silently skip, never
fall through to a passing gate.

## Scenario + experiment wiring

In `o8v/tests/scenarios.rs`:

```rust
pub static FIX_TS_BASELINE: Scenario =
    Scenario::claude_baseline("fix-ts-baseline", &FIX_TYPESCRIPT);
pub static FIX_TS_8V: Scenario =
    Scenario::claude_with_8v("fix-ts-8v", &FIX_TYPESCRIPT);

pub static EXPERIMENT_FIX_TS: ExperimentConfig = ExperimentConfig {
    name: "fix-typescript",
    task: &FIX_TYPESCRIPT,
    control: &FIX_TS_BASELINE,
    treatments: &[&FIX_TS_8V],
    n: 6,
};
```

Test harness entry in `o8v/tests/agent_benchmark.rs`:

```rust
#[test]
fn fix_typescript_baseline() { ... }
#[test]
fn fix_typescript_8v() { ... }
#[test]
fn experiment_fix_typescript() { ... }
```

Same pattern as the rust/python tests.

## Reporting

Per-language row alongside rust/python in the report. Never folded
into a cross-language mean.

## Open questions

- **`tsc` availability.** Is `tsc` present on the benchmark machine?
  Node usually is; TypeScript often isn't without `npm i -g
  typescript` or `npx tsc`. If we go `npx tsc`, the first invocation
  hits the network (one-time) — either accept that or require global
  install in the benchmark env setup.
- **Do we need a second TS scenario for runtime/test-shape bugs?**
  Defer. Ship type-error scenario first; add a test-runner scenario
  only if N=1 leaves signal ambiguous.
- **Where does the failing test go if we skip option (d)?** Answered
  above — upgrade path is `.test.mjs` + `node --test`.

## Non-goals

- Framework fixtures (React, Vue, Next, etc.).
- Build-tool fixtures (webpack, vite, esbuild).
- Monorepo / workspace fixtures.
- Testing the npm ecosystem.
- Multi-file refactors. Single-file fix.

## Migration

- Net-new scenario. Does not replace anything.
- Depends on `polyglot-benchmark-redesign.md` being approved first
  (so the old `check-polyglot` is retired before TS lands — avoids
  two concurrent "polyglot-ish" scenarios).
- No fixture built, no code written, until this doc is approved.
