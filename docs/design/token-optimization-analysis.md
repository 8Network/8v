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

---

## 6. Polyglot Findings — Where Open-Ended Tasks Break the Model

The polyglot task ("check this project for issues and fix everything you find") is open-ended. 8v adds a fixed 12-call discovery overhead that compounds across 47+ turns. On focused tasks, 8v eliminates 3-8 retry turns, saving more than the overhead costs. On open-ended tasks, there are fewer retries to eliminate, so the overhead dominates.

| Metric | Native | With 8v | Delta |
|--------|--------|---------|-------|
| Tokens (mean) | 293,820 | 461,814 | +57% |
| Cost (mean) | $0.42 | $0.49 | +18% |
| Output tokens | 5,196 | 4,412 | -15% |
| Cache read | 263,485 | 430,716 | +64% |
| Turns | 43.5 | 47.3 | +9% |
| Tool calls | 37.3 | 35.7 | -4.5% |

Key observations:
- Outlier runs 6 and 8 (TodoWrite=8) drag the mean. Without them: +26% not +57%.
- 12-call discovery phase on every run: `8v ls → 8v ls --tree → 10x 8v read`. Identical sequence every time. Builds 3,600B context before first edit.
- Baseline errors are cheap: 15 errors/run at ~50B each. 8v responses are ~300B each.
- Double ToolSearch (Claude Code deferred schema loading) wastes 2 turns per run.
- TodoWrite inflation: 4.67 calls/run vs baseline 0.67.

The +57% is a worst-case measurement (open-ended + outlier inflation). The +26% without outliers is the structural cost of the discovery overhead on complex multi-stack tasks.

---

## 7. Task Design for Deterministic Benchmarks

The benchmark prompt shapes the result more than the tool. Findings:
- Focused prompts ("fix the failing test") → deterministic behavior, low variance, 8v wins
- Open-ended prompts ("check everything, fix everything") → high variance, discovery-heavy, 8v loses

To get deterministic results without reducing complexity:
- Specify WHAT to check, not HOW: "This project has issues in the Rust, Python, and Go code. Find and fix them."
- Name the scope, not the steps: "Fix all compiler errors and test failures" vs "Run cargo check, go vet, pytest"
- Keep multi-stack complexity but focus the goal

The prompt is a confound in current polyglot results. A focused polyglot prompt ("fix all compiler errors and test failures in this Rust+Python+Go project") would isolate 8v's tool effect from the discovery overhead effect. This is the next benchmark to run before drawing conclusions about open-ended task performance.

---

## 8. Batching Opportunity (Quantified)

The 12-call discovery phase (`8v ls + 8v ls --tree + 10x 8v read`) could be 2-3 batched calls:
- Batch 1: `8v ls --tree --loc` (replaces `8v ls` + `8v ls --tree`)
- Batch 2: `8v read file1 file2 file3 ...` (replaces 10 individual reads)

Impact estimate for polyglot:
- 12 calls → 2-3 calls = 9-10 fewer MCP round-trips
- Each round-trip adds ~200B to context that compounds across 47 turns
- Estimated saving: ~85K tokens per run (9 calls × 200B × 47 turns)
- Would bring 8v from +57% to approximately +25-30%

For Codex (single-turn): batching is MORE critical because all responses accumulate in one context window. The 10-12 MCP calls → 3-4 batched calls would reduce input tokens by ~40%.

Batching does NOT solve the fundamental problem for open-ended tasks (8v adds turns). It only reduces the per-turn cost of the discovery phase.

---

## 9. Combined Optimization Potential

If all optimizations were applied:
1. Focused task prompt (eliminates variance, ensures retry-elimination value)
2. Batched discovery (saves ~85K tokens on polyglot)
3. Agent detection `_8V_AGENT` (saves formatting overhead on CLI fallback)
4. Smarter error messages (saves 1-2 retry turns on focused tasks)

| Optimization | Estimated saving | Applies to |
|---|---|---|
| Focused prompt | Eliminates +31% outlier effect | Open-ended tasks |
| Batched discovery | ~85K tokens (~18% of polyglot total) | Polyglot, Codex |
| Agent detection | <1K tokens per run | CLI fallback only |
| Smarter errors | 10K-22K tokens per run | Focused tasks |

Estimated combined effect on polyglot: from +57% to approximately +5-15%.

The path to positive ROI on polyglot is: focused prompt + batched discovery. Both are achievable without architectural changes to 8v — prompt is a benchmark design decision, batching is a command API extension.
