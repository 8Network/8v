# Test Patterns and Mistakes — 2026-04-18

## Why This Exists

Two bugs shipped in the same week despite tests existing:

1. **output_bytes=0**: `o8v/src/dispatch.rs` passes `0` as `output_bytes` to every `CommandCompleted` event. All pipeline tests hardcode `output_bytes: 512`. The bug was invisible.
2. **Retry-cluster window**: The consecutive-gap algorithm allows a cluster spanning 4+ minutes to be counted as a 30s window. Every test used events within tight (sub-2s) windows. The algorithm's failure mode was never exercised.

Both bugs have the same root shape: **tests exercised the code we wrote, not the behavior we promised**.

---

## The Five Recurring Patterns

### Pattern 1 — Hardcoded Happy-Path Fixtures

**What it looks like:** Every test helper sets a sentinel value. Tests check that the value round-trips, but never check that a *different* value also round-trips.

**Concrete evidence:**
- `o8v/tests/counterexamples_stats_v2.rs:48` — `make_completed` hardcodes `output_bytes: 512`. All stats tests are built on this helper.
- `o8v/tests/counterexamples_stats_v2.rs:100` — `ndjson_pair` hardcodes `"output_bytes": 512_u64`.
- `o8v/tests/counterexamples_basic_ops.rs:88-89` — comment: "Why existing tests don't catch this: every helper hardcodes `output_bytes=512`. A regression that always returns 512 (or never reads the field) would pass."

**Why it hides bugs:** A bug that always returns the sentinel (or ignores the field entirely) passes every test. The field being read is never verified — only the presence of the test artifact.

**The fix:** Parameterize helpers. Pass the value under test as an argument. Explicitly test `0`, boundary values, and values that differ from any hardcoded constant.

---

### Pattern 2 — Tests at the Wrong Layer

**What it looks like:** A bug lives in Layer A (the dispatcher). Tests exercise Layer B (the pipeline). Layer A is never touched by any test.

**Concrete evidence:**
- `o8v/src/dispatch.rs:57` — passes literal `0` as `output_bytes`. This is the dispatcher layer.
- All pipeline tests in `o8v/tests/counterexamples_stats_v2.rs` inject events directly into the pipeline, bypassing the dispatcher entirely.
- `o8v/tests/counterexamples_basic_ops.rs:6-7` — gap inventory notes this as Gap 1: "output_bytes=0 round-trip from CLI dispatch to DrillReport".

**Why it hides bugs:** Layer B tests can have 100% coverage and prove nothing about Layer A. Every abstraction boundary is a potential hiding spot for exactly this class of bug.

**The fix:** For each bug, identify *which layer it lives in*. Write the test at that layer, not at a convenient layer above it. For CLI bugs: write a test that calls the binary or the dispatcher function directly.

---

### Pattern 3 — Same-Shape Inputs (No Boundary Crossing)

**What it looks like:** All test inputs share the same structural properties. The test suite is wide but not deep. Edge cases that cross a threshold are never exercised.

**Concrete evidence:**
- Retry-cluster tests in `o8v/tests/counterexamples_basic_ops.rs:283-340` use 3 events within a 1,400ms window. The 30,000ms window boundary is never tested.
- The consecutive-gap bug: 10 events × 29s gaps = 4.4min span counted as a 30s cluster. No test has events with gaps near the boundary (e.g., 28s, 30s, 31s, 29s×N).
- `o8v/tests/counterexamples_hook_redaction.rs:53-80` — the redaction tests DO cross boundaries (19-char key NOT redacted, 20-char MUST be redacted). This is the correct pattern.

**Why it hides bugs:** Algorithms have thresholds. If all test inputs are well inside the threshold, off-by-one errors and window-boundary bugs are invisible.

**The fix:** For every numeric threshold in the algorithm, test: below, at, above. For window-based algorithms: test inputs that span exactly the window, inputs that span 2x the window, and inputs that span 0.9x the window.

---

### Pattern 4 — Enforcement Not Verified

**What it looks like:** The test claims to enforce a constraint ("agent must use 8v", "tool is blocked"). The constraint is never confirmed to be active. The test passes regardless.

**Concrete evidence:**
- `postmortem_benchmark_enforcement.md` — `blocked_tools: &[]` for all benchmark runs. Agent was never forced to use 8v. Three days of results invalid.
- Root cause (exact words from postmortem): "I audited types, naming, silent fallbacks — surface-level code quality — but missed the one field that determines whether the experiment measures anything at all."
- Pattern holds in non-benchmark code: if a test "checks" a security property by asserting on output without verifying the mechanism is active, the same class of failure applies.

**Why it hides bugs:** Passing tests prove the code ran, not that the constraint was active. A test that passes with `blocked_tools: &[]` AND with `blocked_tools: &["Read", "Edit"]` is not a test of enforcement — it's a test of output format.

**The fix:** First audit of any test with a constraint: "Is the constraint actually enforced?" Not types, not naming. Enforcement first. For benchmark configs: verify `blocked_tools` is non-empty before reading a single result.

---

### Pattern 5 — Coverage Gaps by Omission

**What it looks like:** Entire subcommands, code paths, or modules have no tests. Not because they were tested and found clean — because no one wrote the test.

**Concrete evidence:**
- `e2e_coverage_audit_apr17.md` — search: zero E2E. hooks: zero E2E. upgrade: zero E2E. mcp: zero E2E. Only 3/12 subcommands are fully E2E'd.
- `test_gap_discipline.md` — TS-runner silent-empty bug: "no test exercised jest/vitest output paths." The test gap was the real failure.
- `feedback_tests_must_catch_bugs.md` — B-MCP-3: existing parse.rs tests only exercised "flag is present" case. The "no flag, multi-positional" corner was untested.

**Why it hides bugs:** The code compiles and existing tests pass. There is no red signal. The gap is invisible until a user hits it.

**The fix:** Maintain an explicit gap list (like `e2e_coverage_audit_apr17.md`). Before shipping any change to a subcommand, check whether that subcommand has E2E coverage. If it doesn't, that's the first item on the task list.

---

## The Five Rules

**Rule 1 — Write the failing test first.**
Before writing the fix, write the test. Run it on the pre-fix code. If it does not fail, the test is wrong. A test that passes before the fix is a documentation artifact, not a test.

**Rule 2 — Test at the layer where the bug lives.**
Identify the exact function, struct, or module where the bug exists. Write the test that calls that function directly. Do not test through abstractions — every layer of indirection is a layer where bugs can hide.

**Rule 3 — Cross every threshold.**
For every numeric boundary in the code under test, write one test below, one at, one above. For window algorithms: sparse inputs that span the window, dense inputs that don't, and inputs that cross the boundary by one element.

**Rule 4 — Verify enforcement before reading results.**
When a test constrains behavior (blocks tools, enforces a quota, requires a property), verify the constraint is active before running the test. The first line of a benchmark audit is: "is `blocked_tools` non-empty?" Not line counts, not output format.

**Rule 5 — Name the gap before closing it.**
Every subcommand, every algorithm, every security property needs a named entry in the gap list. If a test doesn't exist, that must be an explicit decision — not an oversight. The gap list (`e2e_coverage_audit_apr17.md`) is not optional metadata. It is the source of truth for what is covered.

---

## Checklist — Grading a New Test

Use this before merging any test. A test fails this checklist if it cannot answer YES to every applicable question.

```
[ ] 1. LAYER: Does the test call the exact layer where the bug/behavior lives?
        (Not a layer above it. Not through a convenience wrapper that bypasses the mechanism.)

[ ] 2. FAILING FIRST: Was the test run on pre-fix code? Did it fail?
        (If adding after the fix: can you revert the fix and confirm it fails?)

[ ] 3. PARAMETERIZED: Does the test use a value that differs from any hardcoded constant in helpers?
        (output_bytes: 512 in every helper = not parameterized. Test 0, boundary values, distinct values.)

[ ] 4. BOUNDARY: If the code has a threshold (30s, 20 chars, 6 stacks), does the test cross it?
        (Test below, at, above. Not just inside the safe zone.)

[ ] 5. ENFORCEMENT: If the test claims to block/constrain something, is the constraint verified active?
        (blocked_tools non-empty. Flag actually set. Config actually loaded.)

[ ] 6. GAP UPDATED: Is the gap list updated to reflect what this test now covers?
        (If adding E2E for search: mark search as covered in e2e_coverage_audit.)
```

A test that fails items 1 or 2 is not a test — it is dead code that provides false confidence.

---

## How to Apply Going Forward

**Before any fix:**
1. Identify the layer. Write one-line description: "Bug lives in `dispatch.rs:57`, not in the pipeline."
2. Write the test at that layer. Run on pre-fix code. Confirm it fails.
3. Fix the code. Confirm the test now passes.
4. Run the adversarial checklist above. Add boundary tests for the nearest 3 variants.

**Before any benchmark run:**
1. Read the config. Find `blocked_tools`. Confirm it is non-empty.
2. If empty: stop. Fix the config. Do not run.

**Before any code review:**
1. For each concern, ask: "Is there a test that would catch a regression here?" If no: that's the primary gap to fix, not the concern itself.
