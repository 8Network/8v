# Review R1: MCP Adapter Output Cap Design

**Reviewer:** Claude Sonnet 4.6 · **Date:** 2026-04-19  
**Design:** `docs/design/mcp-adapter-output-cap.md` · **Round:** 1 of N

---

## Blockers

**B1 — `caller` not in scope at insertion point**  
Design §4: "Where: `read_to_report` (~line 190) … Condition: `caller == Caller::Mcp && args.full`"  
Source: `o8v/src/commands/read.rs` — actual signature is  
`fn read_to_report(args: &Args, ctx: &CommandContext) → Result<ReadReport, String>`  
No `caller` parameter exists. The condition cannot compile at that location.  
Resolution required before implementation: (a) thread `Caller` into `read_to_report` via `ctx` or a new param, (b) add a `Caller`-aware wrapper at the `execute()` dispatch layer, or (c) move pre-flight entirely into `handler.rs` before `dispatch_command_with_agent`.

---

## Test Gaps

**T1 — Post-render synthetic command unspecified**  
Design §9 Test 2: "synthetic command producing output > cap — verify `is_error: true`"  
No existing test infrastructure for producing a contrived oversized output. Mechanism is unstated.  
Must specify: fixture file of known size, or a mock/stub command in the test harness that emits N chars.

**T2 — Env var edge cases not covered**  
Design §3: "validate > 0" — no tests specified for `O8V_MCP_OUTPUT_CAP=0`, negative values,  
non-numeric strings, or empty string. Each should produce a distinct, observable error.

---

## Risks

**R1 — `use_stderr` branch bypasses post-render check**  
Design §5 insertion point: "after `dispatch_command_with_agent` returns, before returning `Ok(out)` or `Err(out)`"  
Source: `o8v/src/mcp/handler.rs:78-79` — a third path exists:  
`Ok((out, _exit, use_stderr)) => { if use_stderr { Err(out) } … }`  
If the cap check is inserted only before `Ok(out)`, an oversized `use_stderr` response escapes.  
The check must wrap both the `Ok(out)` and `use_stderr → Err(out)` arms.

**R2 — Persist threshold lower bound unproven**  
Design §3 grounds 55,000 cap in "safety margin under the ~57K persist threshold."  
Findings doc (`mcp-transport-cap-2026-04-19.md`) states "~57,000+" — no lower bound measured.  
Smallest confirmed PASS is 60,500 chars (bisection table). The persist threshold could be lower than 57,000.  
The 55,000 cap may still be safe, but the margin is not empirically grounded from below.

**R3 — 1.20× overhead factor not validated for multi-file batch**  
Design §4 pre-flight: "Multiply by 1.20." Findings doc measures 1.15–1.18× for single-file reads.  
Batch reads add per-file headers and separator lines; overhead could exceed 1.20× at scale.  
No test verifies the factor holds for N ≥ 2 files.

---

## Nits

**N1 — "Parse at startup" is ambiguous for env override**  
Design §3: "parse at startup, validate > 0." MCP handler has no explicit startup hook.  
Clarify: parse on first `handle_command` call and cache, or parse in server init.

**N2 — Error message template indentation**  
Design §6 error template has two-space indent on `output:`, `cap:`, `command:` lines.  
No style is prescribed; this is fine, but the implementation should match the design verbatim  
to keep tests stable.

---

## Summary

| Category | Count |
|----------|-------|
| Blockers | 1 (B1) |
| Test gaps | 2 (T1, T2) |
| Risks | 3 (R1, R2, R3) |
| Nits | 2 (N1, N2) |
| **Total** | **8** |

**Recommendation: Block.** B1 alone prevents implementation — the design's stated insertion point  
cannot access `Caller`. R1 is a correctness hole that would let oversized `use_stderr` responses  
through the cap. Both must be resolved before coding starts. T1 must specify the test mechanism.  
All other items are straightforward to address in a R2 pass.
