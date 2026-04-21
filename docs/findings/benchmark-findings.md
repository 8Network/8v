# Benchmark Findings — All Experiments

2026-04-16

## Summary

Four task shapes benchmarked across two agents (Claude Code, Codex CLI v0.121.0).
8v reduces Claude tokens 27-41% on focused tasks. 8v increases Claude tokens +57% on open-ended polyglot tasks. 8v increases Codex tokens +94%.
The divergence between focused and open-ended tasks is explained by discovery overhead, not tool quality.

---

## Claude Code Results

### fix-failing-test (N=6)

| Metric | Native | With 8v |
|--------|--------|---------|
| Tokens (mean) | 143,409 | 105,126 |
| Cost (mean) | $0.1439 | $0.1368 (-4.9%) |
| Output tokens | 1,096 | 705 |
| Cache read | 142,289 | 104,409 |
| Cache creation | 7,159 | 10,630 |
| Turns | 9.7 | 9.0 |
| Tests pass | 6/6 | 6/6 |
| Tokens CV% | 26.2% | 2.6% |
| Cost CV% | 18.3% | 11.8% |

### diagnose-issues (N=3)

| Metric | Native | With 8v |
|--------|--------|---------|
| Tokens (mean) | 144,669 | 96,645 |
| Cost (mean) | $0.1543 | $0.1439 (-6.7%) |
| Output tokens | 1,424 | 737 |
| Cache read | 143,234 | 95,896 |
| Cache creation | 7,460 | 12,328 |
| Turns | 11.0 | 9.3 |
| Tests pass | 3/3 | 3/3 |
| Tokens CV% | 7.1% | 12.1% |

### fix-python-traversal (N=6)

| Metric | Native | With 8v |
|--------|--------|---------|
| Tokens (mean) | 223,692 | 132,924 |
| Cost (mean) | $0.2271 | $0.1786 (-21.3%) |
| Output tokens | 2,407 | 1,266 |
| Cache read | 221,270 | 131,643 |
| Cache creation | 8,919 | 12,900 |
| Turns | 17.0 | 11.7 |
| Tests pass | 6/6 | 6/6 |
| Tokens CV% | 28.2% | 36.8% |
| Cost CV% | 19.5% | 19.3% |

### check-polyglot (N=6, Claude)

| Metric | Native | With 8v |
|--------|--------|---------|
| Tokens (mean) | 293,820 | 461,814 (+57%) |
| Cost (mean) | $0.42 | $0.49 (+18%) |
| Output tokens | 5,196 | 4,412 (-15%) |
| Cache read | 263,485 | 430,716 (+64%) |
| Turns | 43.5 | 47.3 (+9%) |
| Tool calls | 37.3 | 35.7 (-4.5%) |
| Tests/Check/Build | 6/6 | 6/6 |

**Behavioral analysis:**

- Outlier runs 6 and 8 (TodoWrite=8) drag the mean. Without them: +26% not +57%.
- 12-call discovery phase on every run: 8v ls → 8v ls --tree → 10x 8v read. Identical sequence every time. Builds 3,600B context before first edit.
- Baseline errors are cheap: 15 errors/run at ~50B each. 8v responses are ~300B each.
- Double ToolSearch (Claude Code deferred schema loading) wastes 2 turns per run.
- TodoWrite inflation: 4.67 calls/run vs baseline 0.67.

---

## Codex CLI Results

### fix-failing-test (N=3)

| Metric | Codex Native | Codex + 8v |
|--------|-------------|------------|
| Tokens (mean) | 248,469 | 482,650 (+94%) |
| Input tokens | 131,515 | 253,795 |
| Output tokens | 1,242 | 2,593 |
| Cache read | 115,712 | 226,261 |
| Tool calls | 6 | 11 |
| Turns | 1.0 | 1.0 |
| Tests pass | 3/3 | 3/3 |

---

## Cross-Agent Summary

| Agent | Task | Difficulty | Token delta | Cost delta |
|-------|------|-----------|-------------|------------|
| Claude | fix-test | Easy, focused | -27% | -5% |
| Claude | diagnose | Medium, focused | -33% | -7% |
| Claude | fix-python | Hard, focused | -41% | -21% |
| Claude | polyglot | Complex, open-ended | +57% | +18% |
| Codex | fix-test | Easy, focused | +94% | N/A |

**Key insight: 8v wins on focused tasks, loses on open-ended tasks.**
Focused tasks ("fix one thing") → 8v eliminates retry loops, saving more than overhead costs.
Open-ended tasks ("check everything") → fewer retries to eliminate, discovery overhead dominates.
The loss on polyglot is mechanical (12-call discovery phase, context compounding), not fundamental.

---

## Token Distribution Analysis

### Claude + 8v

- `cache_read` dominates (~99% of total tokens) — conversation history re-sent each turn
- Output tokens drop 35-47% with 8v (705 vs 1,096 / 737 vs 1,424 / 1,266 vs 2,407)
- Cache creation increases with 8v (+48% to +65%) — 8v responses are richer per call
- Fewer turns means less total `cache_read`, which is the dominant cost driver

### Codex + 8v

- Input tokens nearly double (131K → 254K) — MCP schema overhead in system prompt
- Single turn = all MCP results accumulate in context
- Output tokens double (1,242 → 2,593) — more tool calls to generate

---

## Behavioral Observations

### Claude baseline error storms (the primary cost driver 8v eliminates)

- fix-python: `python -m pytest` ERROR repeated 10-12 times in stuck loops
- diagnose: `cargo build` ERROR repeated 3-4 times before finding the fix
- fix-test: 4/6 baseline runs had stuck loops or error storms

### Claude + 8v deterministic patterns

- diagnose: `ls → read → write → check` (6 calls, zero variance)
- fix-python: `ls → test → read → write → test` (7 calls)
- fix-test: exactly 9 turns, exactly 7 tools, every run

### Codex baseline (6 calls, efficient)

```
rg --files .
rg -n "test_sum_range_inclusive" .
sed -n '1,220p' src/main.rs
cargo test  (fails)
[native apply_patch edit]
cargo test  (passes)
```

### Codex + 8v (10-12 calls, wasteful)

```
8v ls
8v ls --tree --loc         ← redundant (first call useless)
8v search pattern -C 3
8v read src/main.rs        ← symbols
8v read src/main.rs:1-60   ← actual code
[native apply_patch edit]  ← bypasses 8v write
8v test . --json
8v check .                 ← double verification
8v test . --json           ← triple verification
8v check .
8v read src/main.rs:1-40   ← re-reads after edit
```

---

## Why 8v Works on Claude but Not Codex

### 1. Retry elimination

Claude does multi-turn (8-14 turns). Without 8v: stuck loops and error storms.
With 8v: deterministic paths, zero variance.
Codex does single-turn — no retries to eliminate.

### 2. Write enforcement

Claude uses `8v write`. Codex bypasses with native `apply_patch`.
`--disable shell_tool` blocks shell commands but NOT `apply_patch`.
No feature flag to disable `apply_patch` in v0.121.0.

### 3. MCP overhead

Codex single-turn accumulates all MCP results in context.
Claude multi-turn resets context each turn.

### 4. MCP + sandbox impossible

Tested all combinations:
- `--full-auto` → MCP "user cancelled"
- `-c sandbox=read-only` → MCP "user cancelled"
- `approval_mode="full-auto"` on MCP server → cancelled
- `--enable exec_permission_approvals` → cancelled
- `--enable guardian_approval` → cancelled
- `--dangerously-bypass-approvals-and-sandbox` + `-c sandbox=read-only` → sandbox overridden

Only `--dangerously-bypass-approvals-and-sandbox` approves MCP calls, but it disables sandbox entirely. Can't have both.

---

## Bugs Found

### In Codex integration

1. **apply_patch bypass** — uses native editor, ignores `8v write`
2. **Redundant discovery** — `8v ls` then `8v ls --tree --loc` (first call useless)
3. **Double/triple verification** — `8v test → 8v check → 8v test → 8v check`
4. **MCP + sandbox impossible** in v0.121.0
5. **Baseline contamination** (FIXED) — `write_codex_config()` ran for all scenarios including baseline. Baseline got MCP server config it shouldn't have. Fixed: moved inside `if setup_8v`.

### In benchmark infrastructure

6. **Tool call detail not persisted** (FIXED) — `report.json` per-run data didn't include tool call sequences. Fixed: added `tool_calls_detail` to `RunRecord` and print block.

---

## Pending

- Global MCP registration + `--full-auto` test (credits exhausted). Old `8v codex` implementation (commit `523bdba24`) used `codex mcp add` (global). Our benchmark uses project-local `.codex/config.toml`. Global may get different approval treatment — untested.
- Batching opportunity: if 8v supported batch commands, 10-12 calls → 3-4 calls. Critical for Codex single-turn. Architecture-neutral for Claude multi-turn.
- Agent detection (`_8V_AGENT=1`) for CLI fallback — designed, not built.
