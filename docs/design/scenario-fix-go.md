# Design — fix-go scenario

**Status:** DRAFT — awaiting review.
**Date:** 2026-04-16.
**Parent:** `docs/design/polyglot-benchmark-redesign.md`

## Goal

Per-language token-efficiency benchmark for Go, matching the shape of
`fix-failing-test` (rust) and `fix-python-traversal` (python). One
small fixture, one failing check, problem-shaped prompt.

## Fixture

Path: `o8v/tests/fixtures/agent-benchmark/fix-go/`
Size target: ~50–100 lines across 2 files. Small.
Contents:

```
fix-go/
├── go.mod               # module name only, no external deps
├── lib.go               # code under test (contains the bug)
└── lib_test.go          # failing test using stdlib testing
```

No external dependencies. `go.mod` pins only the Go version. No
`vendor/`. No network access needed at run time — stdlib only.

## Task

```rust
pub static FIX_GO: Task = Task {
    name: "fix-go",
    fixture: "agent-benchmark/fix-go",
    prompt: "Some tests in this Go project are failing. Find the bugs and fix them.",
    variables: &[],
};
```

Problem-shaped. Mirrors rust/python. No bug enumeration.

## Bug design

One seeded bug. Realistic Go candidates:

- Off-by-one in slice bounds (caught by a table-driven test).
- Wrong comparison (`>` vs `>=`) in a loop condition.
- Missing error check that causes a test to assert on wrong value.
- Format-verb mismatch (`%d` with a string) — caught by `go vet` even
  before `go test`.

Preference: a bug that fails `go test ./...`. A second variant could
fail `go vet` only — defer that.

Discovery required — no README, no AGENTS.md enumerating the bug.

## Success gate

Single deterministic command, same for baseline + 8v:

```
go test ./...
```

Exit 0 = pass. Reported on `tests_pass`.

Optional tightening: `go vet ./... && go test ./...`. But `go test`
invokes the compiler, which catches most vet-level issues already.
Start minimal — add `go vet` only if a scenario specifically targets
vet-only errors.

## Preflight

- `go version` resolves. Fail loudly if missing.
- Go toolchain is standard on dev machines and most CI runners, so
  this preflight rarely fires. It's still required — no silent skip.

## Scenario + experiment wiring

In `o8v/tests/scenarios.rs`:

```rust
pub static FIX_GO_BASELINE: Scenario =
    Scenario::claude_baseline("fix-go-baseline", &FIX_GO);
pub static FIX_GO_8V: Scenario =
    Scenario::claude_with_8v("fix-go-8v", &FIX_GO);

pub static EXPERIMENT_FIX_GO: ExperimentConfig = ExperimentConfig {
    name: "fix-go",
    task: &FIX_GO,
    control: &FIX_GO_BASELINE,
    treatments: &[&FIX_GO_8V],
    n: 6,
};
```

Test harness entry in `o8v/tests/agent_benchmark.rs`:

```rust
#[test]
fn fix_go_baseline() { ... }
#[test]
fn fix_go_8v() { ... }
#[test]
fn experiment_fix_go() { ... }
```

Same pattern as rust/python.

## Reporting

Per-language row alongside rust/python/ts in the report. Never folded
into a cross-language mean.

## Open questions

- **`go.mod` Go version.** Pin to the installed Go or leave floating?
  Pin to a conservative version (e.g. `go 1.21`) to minimise
  toolchain-driven variance.
- **Should we test Go modules workflow (external deps)?** Defer. First
  scenario is stdlib-only to isolate the signal.
- **Second scenario for `go vet` only?** Defer. Only if N=1 leaves
  signal ambiguous.

## Non-goals

- Generics-heavy fixtures — not representative of average Go code.
- Multi-module fixtures.
- gRPC / protobuf / large framework integration.
- Testing the Go modules ecosystem.

## Migration

- Net-new scenario. Does not replace anything.
- Depends on `polyglot-benchmark-redesign.md` being approved first.
- No fixture built, no code written, until this doc is approved.
