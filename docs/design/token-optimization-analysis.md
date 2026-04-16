# Token Optimization Analysis

2026-04-16

Source data: codex-benchmark-findings.md (N=6 Claude fix-python, N=6 fix-test, N=3 diagnose, N=3 Codex fix-test)

---

## 1. Where Tokens Go

### Claude (multi-turn)

| Category | fix-test baseline | diagnose baseline | fix-python baseline |
|----------|-------------------|-------------------|---------------------|
| cache_read | 142,289 (99.2%) | 143,234 (99.0%) | 221,270 (98.9%) |
| cache_creation | 7,159 (5.0%) | 7,460 (5.2%) | 8,919 (4.0%) |
| output | 1,096 (0.8%) | 1,424 (1.0%) | 2,407 (1.1%) |
| **total** | **143,409** | **144,669** | **223,692** |

cache_read is the bill. Every turn re-sends the full conversation history. Adding turns multiplies cache_read linearly.

### Codex (single-turn)

| Category | Native | With 8v |
|----------|--------|---------|
| input | 131,515 (52.9%) | 253,795 (52.6%) |
| cache_read | 115,712 (46.6%) | 226,261 (46.9%) |
| output | 1,242 (0.5%) | 2,593 (0.5%) |
| **total** | **248,469** | **482,650** |

For Codex: input + cache_read split evenly. MCP schema in system prompt doubled input immediately.

---

## 2. What Causes Waste

### Stuck loops (Claude baseline, primary driver)

- fix-python: `python -m pytest` ERROR repeated 10-12 times. 17.0 turns mean. Each extra turn adds ~13K cache_read tokens.
- diagnose: `cargo build` ERROR repeated 3-4 times. 11.0 turns vs 9.3 with 8v.
- fix-test: 4/6 baseline runs had stuck loops. CV 26.2% vs 2.6% with 8v — variance proves loops are random, not structural.

Calculation: fix-python surplus turns = 17.0 - 11.7 = 5.3 turns × ~13K tokens/turn ≈ 69K tokens wasted per run in stuck loops. Observed delta: 221,270 - 131,643 = 89,627 cache_read tokens. Loops account for ~77% of the cache_read delta.

### Redundant discovery (Codex + 8v)

Codex calls `8v ls` then `8v ls --tree --loc` — first call is discarded. Two MCP calls where one suffices. In single-turn context accumulation, both results stay in context for the full turn.

### Double/triple verification (Codex + 8v)

Sequence observed: `8v test → 8v check → 8v test → 8v check`. Four verification calls for one edit. Codex does 10-12 tool calls vs baseline 6. Each extra MCP call appends its result to the single-turn context permanently.

### Re-reads after edit (Codex + 8v)

`8v read src/main.rs:1-40` appears after the edit to confirm. Adds tokens with no diagnostic value since `8v test` already confirmed correctness.

### Human-formatted output in agent context (Claude CLI fallback)

When 8v CLI is used (not MCP), output includes ANSI codes, box-drawing, color markers. These render as raw escape sequences in agent context — token waste with zero information gain. Measured in the `_8V_AGENT` design but not yet in the benchmark.

---

## 3. What 8v Already Fixes

### Retry elimination (Claude, proven)

8v produces deterministic tool sequences. diagnose: `ls → read → write → check` every run, 6 calls. fix-python: `ls → test → read → write → test` every run, 7 calls. CV drops from 26.2% to 2.6% on fix-test, proving variance elimination is structural.

Token impact: -27% to -41% depending on task difficulty. Scales with difficulty because more difficult tasks produce more baseline retries.

### Output compression (Claude, proven)

Output tokens drop 35-47%: 1,096 → 705 (fix-test), 1,424 → 737 (diagnose), 2,407 → 1,266 (fix-python). Structured 8v responses require less agent prose to interpret — fewer tokens spent reasoning about ambiguous tool output.

### Error surface reduction

8v check/test surfaces the exact failing check in one call. Baseline agents call `cargo build` then `cargo clippy` then `cargo test` separately. 8v collapses this to one call. Each eliminated redundant call removes one turn of cache_read accumulation.

---

## 4. Optimization Opportunities

### A. Agent detection (`_8V_AGENT`) — designed, not built

When 8v runs via CLI (not MCP), output includes human formatting: ANSI codes, separators, padding. An agent reading this in context pays tokens for decoration.

Estimate: a typical `8v ls` human output is ~150 tokens. Plain text equivalent is ~80 tokens. 10 calls × 70 tokens/call = 700 tokens per run. Across 11.7 turns average (fix-python with 8v): marginal. But for Codex single-turn with 10-12 calls accumulating in context, 10 calls × 70 tokens = 700 tokens in a 482K run = <0.2%. Low priority for token cost; higher value for accuracy (agents misparse ANSI).

### B. Batching (multiple commands in one MCP call) — Codex critical

Codex wastes calls on `8v ls` + `8v ls --tree --loc` (2 calls, 1 useful). If 8v supported batch: `{"commands": ["ls", "ls --tree --loc"]}` → 1 round-trip, 1 MCP result in context.

Estimate: Codex 10-12 calls → 5-6 batched calls. Each eliminated MCP result that accumulates in Codex single-turn context is ~2-5K tokens. Eliminating 5 redundant calls × 3K avg = 15K tokens. On 482K total = 3% reduction. Larger impact on verification loops: eliminating 2 of 4 verification calls × 5K each = 10K tokens.

Total batching opportunity on Codex: ~25K tokens = ~5% reduction. Brings Codex overhead from +94% to roughly +85%. Not a fix — Codex's structural disadvantage (single-turn MCP accumulation) remains.

### C. Smarter error messages — Claude, measurable

When `8v check` fails, the agent reads the error, reasons about it, forms a plan, then acts. If the error message includes the fix location (file:line) and failure category, the reasoning step collapses.

Baseline Claude on diagnose: 3-4 `cargo build` repetitions before finding the error location. If `8v check` error output is `src/lib.rs:47: unused import — remove or use it`, the agent acts on turn 1.

Estimate: diagnose baseline 11.0 turns → 9.3 with current 8v. If smarter errors eliminate 1 more retry turn: 8.3 turns. Cache_read delta: 1 turn × ~10K tokens = 10K tokens. On 96,645 mean = ~10% additional reduction. Diagnose would go from -33% to -40%.

Fix-python potential is larger: 11.7 turns with 8v. If smarter Python test errors (exact line + assertion value) eliminate 1-2 more stuck turns: 9.7-10.7 turns. Delta: 1-2 turns × ~11K tokens = 11-22K tokens. On 132,924 = 8-17% additional reduction.

### D. Progressive disclosure — already implemented, verify usage

`8v read <path>` gives symbols; `8v read <path>:<start>-<end>` gives lines. This prevents agents from reading 500-line files when they need 20 lines.

Codex behavioral trace shows this is working: `8v read src/main.rs` (symbols) then `8v read src/main.rs:1-60` (specific range). But the re-read after edit (`8v read src/main.rs:1-40`) is noise — 40 lines read after test already confirmed the fix. No token value.

Opportunity: if `8v test --json` output included the changed lines in its diff summary, agents would not re-read. Estimate: eliminates 1 read call per fix = ~2K tokens per run on Codex.

### E. MCP schema size — Codex structural issue

Codex input jumped 131K → 254K, a 123K increase. The MCP schema for 8v is 129 tokens per the memory file. But the full schema accumulates in Codex system prompt. 123K additional input tokens suggests the entire conversation setup (system prompt + schema repetition per turn structure) doubles.

This is a Codex architecture problem, not an 8v problem. 8v cannot reduce MCP schema size without removing commands. The only lever: reduce redundant tool calls so fewer MCP results accumulate.

---

## 5. Priority Stack

| Opportunity | Effort | Token impact | Agent |
|-------------|--------|--------------|-------|
| Smarter error messages (file:line + category) | Medium | -10% to -17% additional | Claude |
| Batching | High | -5% | Codex |
| Agent detection (`_8V_AGENT`) | Low (designed) | <1% tokens, accuracy gain | Both |
| Eliminate re-read after edit | Low | <1% | Codex |
| Progressive disclosure | Done | Already captured | Both |

**Smarter error messages is the highest-leverage optimization remaining.** The benchmark data shows diagnose and fix-python still have 1-2 avoidable retry turns even with 8v. Error quality is the bottleneck, not tool coverage.

Batching matters for Codex but does not close the structural gap. Codex single-turn with MCP is architecturally disadvantaged regardless of batching.
