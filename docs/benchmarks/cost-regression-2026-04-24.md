# Cost Regression — 2026-04-24

Internal record. Do not cite publicly. Public claim is token efficiency / rate-limit headroom only.

---

## Timeline

| Date | Benchmark | Cost Δ vs baseline | Notes |
|------|-----------|-------------------|-------|
| Apr 17 | fix-go N=6 | −22% | Errors-first slice, best result |
| Apr 21 | fix-go N=6 | −18% | After codebase org pass |
| Apr 23/24 | fix-go N=6 | +38–52% | Regression detected — see root cause below |
| Apr 23/24 post-fix | fix-go N=6 | +23.8% | After Turn-1 directive partial fix |

---

## Root Cause

Claude CLI's **Tool Search / deferred-schema mechanism** is the culprit.

When an agent first invokes an MCP tool that has a deferred schema, Claude CLI fetches the full schema mid-conversation as a `tool_result` message. This:

1. Creates a new prompt-cache breakpoint at an arbitrary mid-session point.
2. Cascades into roughly **−36K lost `cache_read` tokens** across the session — those tokens must be re-created instead of read from cache.
3. The mid-session schema fetch itself costs ~**1,040 `cache_creation` tokens** on top.

Together these dominate any savings from fewer tool calls or turns.

---

## Partial Fix (Turn-1 Directive)

`8v init` writes a Turn-1 directive into `CLAUDE.md` instructing the agent to run `8v ls --tree` as its first action. This pins the ToolSearch deferred-schema break to Turn 1, where nothing downstream can be cache-invalidated — the cache breakpoint is at the start of the session regardless.

Measured effect on fix-go: cost Δ moved from **+38.2% → +23.8%**.

The fix is partial, not complete.

---

## Remaining Gap (~+24%)

The residual +24% is **inherent MCP overhead**: Claude CLI writes `settings.json` and `mcp.json` paths into the initial prompt context. This inflates `cache_creation` tokens on every session. There is no 8v-side fix for this — it is a Claude CLI architectural property.

---

## Things We Considered and Rejected

- **`ENABLE_TOOL_SEARCH=false` env var**: Founder ruled out. Not a supported config; fragile against Claude CLI version changes. Ruled out 2026-04-14.
- **Reverting `instructions.txt` to pre-Apr21 size**: Tested. No measurable effect on cost regression.
- **Trimming `ai_section.txt`**: Tested. Recovered only the predicted ~900 tokens (consistent with expectation). Cost remained positive. The regression is not in the instruction size.

---

## Future Work

(a) **File feature request with Anthropic** for a first-class `eagerSchemas` / `preload` MCP config key. Claude CLI telemetry strings suggest they are already measuring deferred-schema latency — this is a known friction point on their side.

(b) **Benchmark provenance** (done in #50 today): every future bench run is version-correlated so we can track cost Δ across releases without ambiguity.

(c) **`BENCH_TRANSCRIPT_DIR`**: Every future bench run should set this env var to preserve raw stream-json. Without raw transcripts, post-hoc cache analysis requires re-running the bench.

---

## What Is Defensible for Release

The public claim is **token efficiency and rate-limit headroom**, not cost:

- Total tokens −13.4% (Claude Sonnet, fix-go, N=6, 2026-04-24)
- Tool calls −30%
- Turns −27%
- Output tokens −50% (the tightest OTPM bucket — biggest headroom win)
- 6/6 tasks pass both sides

Cost claim: do not make it. MCP overhead means 8v users pay more per session in dollars until Anthropic resolves the deferred-schema break. Token and call reductions are real and translate directly to rate-limit headroom, not to invoice savings.
