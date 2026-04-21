# Stabilization Loop

The practice we run during Phase 0. Named so it can be cited, critiqued, and improved. Every slice in this phase follows these seven steps in order. If a step is skipped, the slice is not done.

## The Loop

1. **Measure.** Observe what 8v actually does before touching code. Run the command, capture stdout/stderr/exit code, read the code path. No assumptions — if it is not observed it is not a finding. Dogfood sessions and benchmark runs are the primary measurement surface.

2. **Design.** One concern per design doc. Narrow enough to ship in a single slice. Document what changes, why, and what it does not change. Level 1 (what/why) and Level 2 (how, with software principles) — see `feedback_design_two_levels`.

3. **Counterexample review.** Adversarial round against the design. Reviewer tries to break it. No code until rounds come back empty — see `feedback_counterexample_review`. Review is the gate, not a formality.

4. **Narrow slice.** One behavioral change per design, one design per slice. Cross-cutting designs fan out into many slices but each slice changes one thing.

5. **Failing-first test.** Write the test, run it on pre-fix code, confirm it fails. Only then implement the fix — see `feedback_tests_must_catch_bugs`. Tests that pass before the fix prove nothing.

5.5. **Mutation audit.** After the test passes, mutate the production code in ≥3 targeted ways and re-run the new tests. At least one mutation must break each claimed assertion. Slice-1/2/3 audits (2026-04-20) found theater in every slice we shipped — this step is not optional. Gaps close with targeted tests before the slice is done.

6. **Ship.** Implementation matches the design verbatim. No deviations — see `design_deviation_rule`. `8v check` must pass on itself before the slice is done.

7. **Benchmark / cross-check.** Re-run the benchmark that exposed the original gap. Did the slice move the axis? If yes, record the delta. If no, the fix missed the target — return to step 1.

## Why the loop works

Each step closes a specific failure mode we have observed in this repo:

| Step | Failure it prevents |
| --- | --- |
| Measure | Fixing imagined bugs instead of real ones |
| Design | Code-first churn, architectural drift |
| Counterexample review | Shipping designs with unanalyzed edge cases |
| Narrow slice | Multi-concern PRs that stall in review |
| Failing-first test | Happy-path theater that survives regressions |
| Mutation audit | Tests that pass via the wrong code path |
| Ship | Implementation drift from approved design |
| Benchmark | Celebrating work that did not move the user-visible surface |

## Dogfood feeds the loop

Two feedback channels run continuously in parallel with the loop:

- **Agent feedback sections.** Every agent that uses 8v must end its report with an explicit friction list — see `feedback_agents_dogfood_8v`. Agents are our heaviest users.
- **`8v log` / `8v stats`.** The quantitative counterpart. After every agent report, cross-check whether log/stats would have surfaced the same friction. If not, log/stats has a gap.

The loop's step 1 pulls from both channels.

## Worked examples (this session, 2026-04-19 → 2026-04-20)

| Slice | Measure | Design | Review | Test | Ship | Benchmark delta |
| --- | --- | --- | --- | --- | --- | --- |
| 1. `read --full` accepts repeats | Batch-read agent hit `--full --full` error | `read-multi-full-accept.md` | 1 round, empty | 2 E2E tests | `overrides_with = "full"` on one line | Output axis +1.16 (v3→v4) |
| 2. MCP output cap | 120,950-char spill truncated by Claude Code | `mcp-adapter-output-cap.md` | 1 round, r1 blocker (location) → fixed | 7 E2E tests | Pre-flight + post-render + env var | Prevents Claude-Code-side silent truncation |
| 3. Empty symbol map hint | Agents re-read with `--full` anyway | `read-empty-symbol-map-hint.md` | 1 round, 1 test gap → fixed | 4 tests | One-line format!() | Reduces wasted round-trip |
| 4. Instruction-surface updates | v3 agents confused on batch delimiter, MCP-vs-Bash choice | (text-only, no code) | Mirrored across 8 files | n/a | `=== <label> ===` contract + MCP hint added | Output axis +1.16 (shared with slice 1) |

## What the loop is not

- **Not a gate system.** The loop is one team's current practice, not a policy. It exists because it keeps slices honest, not to bureaucratize.
- **Not a substitute for judgment.** If step 3 (counterexample review) finds a flaw too large to patch, throw the design out — do not route around it.
- **Not for large features.** This is a stabilization loop. Feature work uses the full two-level design process plus POC — see `feedback_design_process`.

## Anti-patterns we have seen here

- **Skipping measurement.** Happened on the "hooks.rs rendering as symbol map" bug — turned out to be a stale-binary artifact. Durable learning: `/mcp` reconnect before any MCP measurement.
- **Broad slices.** The first slice-1 design was "`read --full` scope AND delimiter" — 17 review findings, 2 blockers. Narrowed to single concern, review passed immediately.
- **Happy-path tests.** Contains-check for `"no symbols found"` would have passed both the old and new message. Caught at step 3 review, not step 5.
- **Fixing without benchmarking.** Without step 7 we would have shipped slices 1-4 and never known that the Failure axis stayed stuck at 2-3/10. That is now the next measurement surface.

## Current frontier (2026-04-20)

- Failure axis locked at 2-3/10 across six v4 runs. Top friction: error/exit-code contract undocumented.
- Error-contract measurement just landed: `--json` emits JSON errors on only one command; `write` prefixes `error:` twice; `search` silently swallows permission-denied; six distinct stderr prefix patterns.
- Next loop iteration: design the error/exit-code contract slice from the measurement.
