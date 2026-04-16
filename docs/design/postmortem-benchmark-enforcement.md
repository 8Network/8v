# Postmortem — Benchmark Enforcement Failure

**Date:** 2026-04-16
**Duration of impact:** 3 days (all benchmark runs from `af536b5` onward)
**Severity:** All benchmark results invalid

## Timeline

1. **Commit `6c5a821`** — Benchmark infrastructure created. Three environments:
   - `BASELINE_ENV`: native tools, no 8v
   - `WITH_8V_ENV`: 8v + native tools available (`blocked_tools: &[]`)
   - `EIGHTVEE_ONLY_ENV`: 8v only, native tools blocked (`["Read", "Edit", "Write", "Bash", "Grep", "Glob"]`)
   - Experiments had 3 conditions: baseline, with-8v (competition), 8v-only (enforced)

2. **Commit `af536b5`** — "Bump fix-test and polyglot experiments to N=6"
   - `EIGHTVEE_ONLY_ENV` deleted
   - All `*_8V_ONLY` scenarios deleted
   - Experiments reduced from 3 conditions to 2
   - From this point: the 8v treatment = `WITH_8V_ENV` = `blocked_tools: &[]` = nothing enforced

3. **3 days of work on wrong data:**
   - fix-test (N=6): reported -27% tokens — INVALID
   - diagnose (N=3): reported -33% tokens — INVALID
   - fix-python (N=6): reported -41% tokens — INVALID
   - polyglot (N=6, twice): reported +57% tokens — INVALID
   - Codex (N=3): separate issue (Codex uses apply_patch) — partially valid
   - Behavioral analysis of tool sequences — data is real, conclusions suspect
   - Token optimization analysis — built on invalid numbers
   - Benchmark findings document — all numbers wrong

4. **Commit `7d06bc9`** — Fix: `blocked_tools: &["Read", "Edit", "Write", "Glob", "Grep", "NotebookEdit"]`

## What went wrong

The `blocked_tools` field was the single most important configuration in the benchmark. It determines whether the experiment measures anything. It was set to empty for all runs.

The field was never audited. 43 code quality issues were found and fixed (types, naming, silent fallbacks), but the one field that controls experimental validity was never checked.

The comment on `WITH_8V_ENV` said: "Measures whether the agent chooses 8v when both options exist." This was read and accepted as intentional design, not questioned as a configuration error.

## What is valid

All code changes are structurally correct and independent of benchmark results:
- `_8V_AGENT` detection + `--human` flag + 18 tests
- OutputFormat centralization + `audience_with_default`
- MCP respects explicit `--json`/`--plain` flags
- Dead code deletion (Format, Render, check_renderer)
- `--no-color` fix for build/test/search
- Multi-path read
- Improved MCP instructions
- 43 benchmark reliability fixes (enums, Option returns, naming)
- Benchmark infrastructure (pipeline, drivers, report generation)

## What must be re-done

1. Re-run ALL benchmarks with enforcement
2. Rewrite benchmark-findings.md with real numbers
3. Rewrite token-optimization-analysis.md with real data
4. Re-evaluate whether 8v saves tokens at all when enforced

## Open question

The original `EIGHTVEE_ONLY_ENV` blocked Bash. The current fix does NOT block Bash. If Bash is available, the agent can bypass 8v via `cat`, `grep`, `sed`. This needs a decision:
- Block Bash: agent must use 8v for everything including running commands
- Keep Bash: agent uses 8v for file ops, Bash for build/test/run commands

## Lesson

Audit experimental config BEFORE running experiments. The first question is always: "is the treatment condition actually enforced?" Not code quality, not naming — enforcement first.
