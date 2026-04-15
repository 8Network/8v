# Structured Benchmark Report — Design Note

**Status:** draft, pre-review. Do NOT implement until reviewed.
**Author:** Claude, at Soheil's direction (lab mode, 2026-04-14).
**Scope:** benchmark harness output. No CLI changes. No feature work.

---

## The problem

Current output is an ASCII table printed at the end of `experiment_*` tests. For each run we also print a short block with tokens/cost/tool-names. Everything else lives in NDJSON that humans parse with ad-hoc `/tmp/*.py` scripts.

That's fine for lab iteration. It is **not** fine for:
- A publishable benchmark table that readers trust without re-running our Python snippets
- Catching pathologies (stuck loops, retries, is_error storms) without a human stepping through NDJSON
- Comparing conditions on secondary axes (stddev, cold/warm split, mechanism) because they are not printed
- Long-term comparison across commits (we have the NDJSON but no diffed view)

Soheil's directive (2026-04-14): "robust reliable structure, extraction, separation, better benchmark report, structured benchmark report."

## Non-goals

- New statistics. We keep to means, stddev, CV%, min/max, count — no bootstrapping yet.
- Long-term store / dashboards. Out of scope.
- Replacing NDJSON. The structured report is *derived* from NDJSON; NDJSON stays authoritative.

## Design

Every `experiment_*` test emits three artifacts, written under `.8v/benchmarks/<timestamp>-<experiment-name>/`:

| Artifact          | Format        | Consumer                                    |
|-------------------|---------------|---------------------------------------------|
| `report.md`       | Markdown      | Readers. Checked in as the publishable doc. |
| `report.json`     | Structured    | Machines (regression diff, future tools).   |
| `raw.ndjson`      | NDJSON        | Same as today. Per-run observations.        |

Existing NDJSON writing does not change. We add a post-processing step that reads the NDJSON we just wrote, computes the tables, and emits `report.md` + `report.json`.

### `report.json` schema

```jsonc
{
  "schema_version": 1,
  "experiment": "check-polyglot",
  "commit": "abc123",
  "version_8v": "0.1.0",
  "started_ms": 1744648000000,
  "finished_ms": 1744648274000,
  "task": {
    "name": "check-polyglot",
    "shape": "report",                  // enum: report | fix | diagnose — see P35
    "fixture": "agent-benchmark/polyglot-violated",
    "prompt_sha": "deadbeef"            // stable hash of prompt text
  },
  "conditions": [
    {
      "name": "baseline",
      "description": "Native",
      "n": 3,
      "tokens": { "mean": 157184, "stddev": 13463, "cv": 0.086, "min": 141000, "max": 172000 },
      "cost_usd": { "mean": 0.2745, "stddev": 0.0251, "cv": 0.091, "min": 0.248, "max": 0.302 },
      "tokens_by_category": {
        "input":          { "mean": 10,     "stddev": 0 },
        "output":         { "mean": 2712,   "stddev": 315 },
        "cache_read":     { "mean": 154462, "stddev": 13200 },
        "cache_creation": { "mean": 20637,  "stddev": 2100 }
      },
      "cache_split": {
        "cold_runs": [ { "run": 0, "cost_usd": 0.301, "cache_read": 140000 } ],
        "warm_runs": [ { "run": 1, "cost_usd": 0.271 }, { "run": 2, "cost_usd": 0.253 } ]
      },
      "turns":      { "mean": 24.3, "stddev": 2.1 },
      "tool_calls": { "mean": 20.7, "stddev": 2.5 },
      "tools_histogram": { "Bash": 8.7, "Read": 11.0, "Glob": 1.0 },
      "verification": {
        "tests_pass": { "passed": 3, "total": 3 },
        "build_pass": { "passed": 3, "total": 3 },
        "check_pass": { "passed": 0, "total": 3, "note": "inapplicable for report tasks" }
      },
      "landmines": {
        "stuck_loop_runs":         0,    // max_identical >= 3
        "near_stuck_runs":         0,    // max_similar  >= 3 AND not sequential-linter
        "append_recovery_runs":    0,    // consecutive --append
        "is_error_only_storms":    0     // ≥5 consecutive isError on *our* tool (not Bash)
      }
    }
    // ... next condition ...
  ],
  "deltas_vs_control": [
    {
      "condition": "with_8v",
      "cost_delta_pct":   -0.418,
      "tokens_delta_pct": -0.539,
      "calls_delta_pct":  -0.468,
      "turns_delta_pct":  -0.395,
      "note": "N<5 — not yet publishable"
    }
  ],
  "confidence": {
    "n_per_condition": 3,
    "publishable": false,
    "reason": "need N>=6 per condition"
  }
}
```

### `report.md` layout

```
# Benchmark — <experiment name>

**Commit:** abc123  |  **8v version:** 0.1.0  |  **Runs:** N=3 per condition
**Task shape:** report  |  **Fixture:** agent-benchmark/polyglot-violated
**Published:** 2026-04-14 23:27

## Headline

| Condition     | Cost (mean)  | Cost Δ vs control | Tokens (mean) | Turns | Verification |
|---------------|-------------:|------------------:|--------------:|------:|:-------------|
| Native (bash) | $0.2745      | —                 | 157,184       | 24.3  | tests 3/3 ✔  |
| With 8v       | $0.1597      | **−41.8%**        | 72,464        | 14.7  | tests 3/3 ✔  |

> ⚠ N=3 per condition. Not yet publishable (need N≥6). Re-run to confirm.

## Token breakdown (means)

| Category       | Native  | With 8v | Δ       |
|----------------|--------:|--------:|--------:|
| input_tokens   | 10      | 12      | +20%    |
| output_tokens  | 2,712   | 1,455   | −46.3%  |
| cache_read     | 154,462 | 70,997  | −54.0%  |
| cache_creation | 20,637  | 13,968  | −32.3%  |

> Cache-read is ~10% of input-token price. Cost delta (−41.8%) is less dramatic than token delta (−53.9%) because the smaller arm has proportionally more output.

## Variance

| Metric         | Native    | With 8v   |
|----------------|----------:|----------:|
| Tokens CV%     | 8.6%      | 18.2%     |
| Cost CV%       | 9.1%      | 14.0%     |

## Cold vs steady-state (first-run-is-cold)

| Condition | Cold run cost | Steady-state mean |
|-----------|--------------:|------------------:|
| Native    | $0.301        | $0.262            |
| With 8v   | $0.177        | $0.151            |

## Mechanism — tools histogram (per-run means)

| Tool        | Native  | With 8v |
|-------------|--------:|--------:|
| Bash        | 8.7     | 0       |
| Read        | 11.0    | 0       |
| Glob        | 1.0     | 0       |
| ToolSearch  | 0       | 1.0     |
| mcp__8v__8v | 0       | 10.0    |

## Landmines

*No landmines detected.*
(If any, lists: condition, run index, tool-sequence fingerprint, first pathology.)

## Notes

- `check_pass` is 0/3 on both arms. Task shape is `report`; fixture verification runs `cargo clippy -- -D warnings` against the still-violated code. Verification is inapplicable here — deferred.
- Tool histogram shows zero Bash / Read / Glob in the 8v arm. The AGENTS.md cleanup (2026-04-14) removed the `8v run` instruction; agents no longer shell out.

## Per-run raw data

| Run | Condition | Tokens  | Cost    | Turns | Tools | Tests | Build | Check | Landmine |
|-----|-----------|--------:|--------:|------:|------:|:-----:|:-----:|:-----:|:---------|
| 0   | Native    | 141,000 | $0.2480 | 22    | 19    | ✔     | ✔     | ✘     | —        |
| 1   | Native    | 158,792 | $0.2735 | 24    | 20    | ✔     | ✔     | ✘     | —        |
| 2   | Native    | 171,759 | $0.3021 | 27    | 23    | ✔     | ✔     | ✘     | —        |
| 0   | With 8v   | 57,345  | $0.1767 | 12    | 10    | ✔     | ✔     | ✘     | —        |
| 1   | With 8v   | 78,686  | $0.1473 | 14    | 10    | ✔     | ✔     | ✘     | —        |
| 2   | With 8v   | 81,360  | $0.1549 | 18    | 13    | ✔     | ✔     | ✘     | —        |
```

## Implementation plan

**Increment 1 — JSON writer.** Pure function: `NDJSON runs + ExperimentConfig → ReportJson`. All numbers computed here. Unit tests: synthetic NDJSON → expected JSON. No IO in the pure function.

**Increment 2 — Markdown writer.** Pure function: `ReportJson → MarkdownString`. Templating, no math. Unit tests: canonical JSON → stable golden markdown.

**Increment 3 — Landmine detectors.** Ported from `/tmp/landmines.py` and Entry 9's methodology. Pure. Unit tests for each signature (stuck-loop, repeat-append, is_error storm).

**Increment 4 — Integration with `run_experiment`.** After the existing NDJSON write, call the writer pipeline. Write all three artifacts to `.8v/benchmarks/<stamp>-<name>/`. Print path. Keep the existing ASCII table printed to stdout (don't regress the dev loop).

**Increment 5 — Acceptance tests.** Round-trip: run a tiny synthetic experiment with fake stream-json, verify report.md and report.json are byte-stable (modulo timestamps).

Nothing is shipped until acceptance tests pass AND Soheil reviews the first real `report.md` end-to-end.

## Open questions (review needed)

1. **Cold/warm split heuristic.** "First run of a condition is cold" is a proxy. Is there a cleaner signal? (Cache-creation > threshold × cache-read = cold?)
2. **Landmine thresholds.** Entry 9 used `max_identical >= 3`. Should these be condition-dependent? Per-task-shape?
3. **Hash stability of `prompt_sha`.** Include env vars? Just the user prompt string? System prompt is controlled by Claude Code, not us.
4. **Diff view.** Do we want `8v bench diff <commit-a> <commit-b>` later, or is that a separate tool?
5. **Where do artifacts live in git?** `.8v/benchmarks/` is `.gitignore`'d today. Do we commit the `report.md` of publishable runs, or keep them ephemeral and render them into a docs site?

## Not in this change

- New task shapes, new fixtures.
- Per-task-shape verification (see P35). Separate design.
- Cache-pricing model changes. We report cache_read separately, reader computes cost.

---

When Soheil approves this, we write Increment 1 with unit tests and stop there for a review gate before writing anything else.

---

## Review findings (2026-04-15)

Adversarial agent review surfaced schema and sequencing gaps. Address before Increment 1 freezes the schema:

1. **Schema must include pricing model.** Without `pricing: {model, input_per_mtok, output_per_mtok, cache_read_per_mtok, cache_creation_per_mtok}`, `cost_usd` cannot be recomputed by a reader. The doc says "reader computes cost" but omits the inputs.
2. **Schema must include model id, Claude Code version, harness version.** `version_8v` alone does not pin behavior. Add `model_id`, `claude_code_version`, `harness_version`.
3. **Per-run records must live in `report.json`, not just markdown.** Today's design references runs by index inside `cache_split` and `landmines`. Without the per-run array (tokens, categories, turns, tool sequence, duration, exit reason), readers cannot re-bucket cold/warm or recompute stddev without re-parsing NDJSON. Promote the markdown's "Per-run raw data" table into JSON.
4. **Increment ordering bug.** Increment 1 freezes a schema with `landmines` fields, but landmine detectors don't land until Increment 3. Either move landmine detection into Increment 1, or commit to nullable/versioned landmine fields from day 1 and document the schema-bump policy.
5. **N≥6 is the wrong gate.** At CV=18%, 95% CI half-width is ±14.4% at N=6 — fine for a −41.8% headline, useless for an 8% delta. Replace the hard-coded N=6 with `N_required(observed_CV, target_delta_resolution)`. Hard-coding 6 will mint false-confident "publishable" stamps on small-delta tasks.
6. **One renderer, not two.** Increment 4 says "keep ASCII table." The dev-loop ASCII print should also derive from `ReportJson` so there is a single source of truth.
