# Benchmark — Tool Profile as Third Dimension (Design)

**Date:** 2026-04-18
**Author:** Soheil Alizadeh
**Status:** Design — not yet implemented. Awaiting review + POC.
**Related:** `8v-competitor-intelligence.md`, `8v-positioning.md`, `benchmark-infrastructure.md`.

---

## 1. Why

The benchmark infrastructure is 8v's primary moat. Category competitors (Caveman, mcp2cli, OpenWolf, AXI, token-savior, code-review-graph, Tool Search) all publish token-savings claims, most with thin methodology. 8v's advantage is reproducibility.

For 8v to occupy the **auditor** position in the category, the benchmark must be able to run:
- **8v vs native** (current capability) — already done.
- **Competitor-X vs native** (proving their claim on our fixtures).
- **8v vs competitor-X** (head-to-head on identical tasks).

Today the harness cannot express the middle and third rows. This doc fixes that.

## 2. Current shape

`o8v-testkit/src/benchmark/` structures benchmarks as **agent × task**:

```
Run = (Agent, Task, Commit, Settings)
```

- `Agent` = Claude / Codex — encoded in `claude.rs` / `codex.rs`.
- `Task` = fix-test / fix-go / fix-typescript / diagnose / polyglot — encoded as scenario fixtures.
- Tool layer (native / 8v) is **implicit** in the agent config (`.mcp.json`, `CLAUDE.md`).

**Problem:** the tool layer is entangled with the agent. Adding "run Claude with Caveman" requires either:
- (a) a forked `claude.rs` per tool profile, or
- (b) environment swaps that the benchmark loop does not reason about.

Both options corrupt the data model. Neither scales past 3 tool profiles.

## 3. Proposed shape — `ToolProfile` as a third axis

```
Run = (Agent, ToolProfile, Task, Commit, Settings)
```

Where `ToolProfile` is an enum + trait pair:

```rust
pub enum ToolProfile {
    Native,
    EightV,
    Caveman,
    Mcp2cli,
    OpenWolf,
    TokenSavior,
    ToolSearch,   // Anthropic's built-in lazy tool loader
    // ... one variant per competitor we benchmark
}

pub trait ToolProfileHarness: Send + Sync {
    fn id(&self) -> &str;                            // "caveman", "mcp2cli", etc.
    fn version(&self) -> String;                     // pinned version for reproducibility
    fn install(&self, workspace: &Path) -> Result<InstalledTool>;
    fn agent_config(&self, agent: Agent, fixture: &Fixture) -> AgentConfig;
    fn cleanup(&self, installed: InstalledTool) -> Result<()>;
    fn approach(&self) -> Approach;                  // taxonomy tag for report grouping
}
```

**Key properties:**
- Each profile is a first-class object with install + config + teardown.
- Version is pinned per run (otherwise reproducibility dies).
- `agent_config` produces the exact MCP config, CLAUDE.md, env vars, hooks for the (Agent × Profile) pair.
- Cleanup is explicit — no leaked state across runs.

## 4. Implementation — two phases

### Phase 1 — Option A (minimal, first head-to-head)

Goal: publish a head-to-head table in 2–3 weeks.

- Add `ToolProfile` enum with variants: `Native`, `EightV`, `Caveman`, `Mcp2cli`, `OpenWolf`, `TokenSavior`.
- Extend `experiment.rs::ExperimentMatrix` to cartesian-product over profiles.
- Add `profile` field to `store.rs` run records (DB migration on `~/.8v/events.ndjson` schema).
- Inline config generation: each profile's `agent_config` is a function in `o8v-testkit/src/benchmark/profiles/<name>.rs`. Not yet a trait.
- Run fix-test, fix-go, diagnose across all profiles × Claude agent. N=6 per cell.
- Report renderer extended to show profile column.

**Estimated effort:** ~1 week of focused work. Bounded by competitor install quirks, not by 8v code.

### Phase 2 — Option C (harness trait, after Phase 1 proves value)

Goal: let third parties add profiles without touching 8v internals.

- Refactor `profiles/<name>.rs` files into implementations of the `ToolProfileHarness` trait.
- Expose registration: `ToolProfileRegistry::register(Box<dyn ToolProfileHarness>)`.
- Profiles can live outside the main crate (contributor PRs in `o8v-bench-profiles/`).
- Document the trait + contribution path in `CONTRIBUTING.md`.
- Reproducibility: every run records `profile.version()` for later citation.

**Estimated effort:** ~1 week after Phase 1 data is validated. Only worth doing if external contributors materialize.

## 5. Data model change

`store.rs` today records:

```
RunRecord { agent, task, commit, cost, turns, tool_calls, duration, ... }
```

Add:

```
RunRecord { agent, task, profile, profile_version, commit, cost, turns, ... }
                      ^^^^^^^ new ^^^^^^^^^^^^^^^^
```

Migration: new events get `profile`/`profile_version`; old events back-fill as `profile=native, profile_version="pre-2026-04"`.

## 6. Fixtures — what to run

Use existing fixtures to keep the table apples-to-apples:

- **fix-test** (canonical — most competitors should run here).
- **fix-go** (polyglot stress — most competitors are not polyglot; expected blowout).
- **fix-typescript** (same as above, different language).
- **diagnose** (non-edit task — tests navigation/reading, not just writing).

Do **not** add new fixtures for competitors. If they fail on our fixtures, that is the signal. Fixtures are the constant; tools are the variable.

## 7. What gets published

After Phase 1, publish a blog post with:

- **Head-to-head table**: cost, turns, calls, success rate per (profile × task).
- **Variance bands** (min, max, median, N=6).
- **Methodology**: clean-commit anchor, fixtures, seeds, agent version, profile versions.
- **Raw data**: `events.ndjson` downloadable.
- **Replication script**: one command runs the full matrix on any machine.

**Every competitor's claim appears in the table with two columns: "Claimed" and "Measured on 8v fixtures."** The delta is the story.

## 8. Risks + mitigations

| Risk | Mitigation |
|------|------------|
| Competitor installs are flaky → noise | Pin versions. Report install failures as their own row. |
| Competitor specializes in a task we don't fixture | Add their fixture as a separate row, noted. Do not remove our fixtures to suit them. |
| Competitor updates mid-measurement | Pin version. Rerun on new version with explicit `profile_version`. |
| We measure wrong — their real claim replicates | **Good.** Publish it. Measurement credibility beats marketing. |
| Benchmark run cost (Claude API spend) | Phase 1 matrix size: 6 profiles × 4 tasks × 6 reps × ~$0.30/run ≈ **$43 per full sweep.** Affordable. |

## 9. What this unlocks commercially

- **Public head-to-head table** — the single highest-ROI artifact 8v can produce in Q2 2026.
- **Auditor brand** — 8v as the only source of independently-verified numbers in the category.
- **Contributor funnel** — external developers can add profiles, run matrices, submit PRs with new data. Each PR is a marketing artifact.
- **Dashboard product seed** — a paid tier that runs the matrix continuously on customers' codebases, showing them which tool actually wins *their* workload.

## 10. Non-goals

- We are not building a universal benchmarking framework for all AI tools. Scope is token-efficiency tool layers.
- We are not running competitors on their own exotic fixtures. Our fixtures are the measuring stick.
- We are not caring about profile correctness beyond task success. If Caveman skips 30% of the task to save tokens, the report shows low task-success — that is the signal.

## 11. Next actions

1. **POC:** add `ToolProfile::Caveman` variant inline (Phase 1 style) and run one cell (Claude × Caveman × fix-test × N=3). Validate the data model change before scaling.
2. **Migration:** update `store.rs` schema + report renderer.
3. **Full matrix:** 6 profiles × 4 tasks × N=6.
4. **Blog post + raw data.**
5. **Decide Phase 2 after observing external contributor interest.**
