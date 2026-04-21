# B1 CE Round 1 — Adversarial Review
Date: 2026-04-20
Reviewer: Claude (Sonnet 4.6)
Inputs: failure-behavior-mcp-draft.md, failure-behavior-ai-section-draft.md, error-contract.md, error-routing-decomposition.md, session-2026-04-20-index.md

---

## Summary

Both drafts identify their own problems (5 CEs each) but do not apply the fixes to the draft text itself.
Every CE in these files is a finding in this review. The drafts ship lies to agents unless blockers are resolved.
Three blockers span both drafts and must be resolved before either ships.

---

## Per-draft findings — MCP draft (failure-behavior-mcp-draft.md)

**B1-MCP-1 [BLOCKER] Pre-B2 NOTE positioned after target**
The NOTE "Pre-B2 binary — do not parse stderr as JSON yet" appears after the post-B2 target description.
An agent reading top-to-bottom learns the target behavior first, may act on it before seeing the caveat.
CE-1 in the draft explicitly identifies this and says "move NOTE to top" — the fix is not applied.
An agent on the pre-B2 binary will call `--json` expecting a structured envelope and get plain-text stderr.

**B1-MCP-2 [BLOCKER] `init` stdout exception missing from draft text**
Draft states: "Stdout is always clean on failure."
Current binary violates this for `init` (BUG-4 / INCONSISTENCY-8): `init: failed` goes to stdout, reason to stderr.
CE-4 in the draft identifies this but the exception does not appear in the shipped text.
Draft ships a false invariant for the current binary.

**B1-MCP-3 [BLOCKER] Two-level JSON schema field names unresolved**
Draft teaches: discriminate by top-level key `"error"` vs `"exit_code"`.
Field names for the subprocess-capture envelope:
  - error-contract §2.4: `{"exit_code","stdout","stderr","duration"}`
  - error-routing-decomposition.md B2b: `{"exit_code","tool","tool_output"}`
These two authoritative docs disagree. The draft does not state which shape the binary will implement.
An agent parsing `"stdout"` will get null when the binary emits `"tool_output"`.

**B1-MCP-4 [MODERATE] Batch `read` partial-success exit behavior ambiguous**
CE-5 identifies: if some entries succeed and some fail, exit code is unspecified.
Draft text says batch `read` errors appear inline — does not address whether exit is 0 or 1.
Agent cannot distinguish "all entries failed" from "some entries failed" via exit code alone.

**B1-MCP-5 [LOW] No concrete example for two-level schema discriminant**
CE-3 identified the teachability problem and suggests an example — fix not applied.
A reader can understand the rule but cannot verify understanding without an example.

---

## Per-draft findings — AI section draft (failure-behavior-ai-section-draft.md)

**B1-AI-1 [BLOCKER] Pre-B2 NOTE positioned after target (same as B1-MCP-1)**
NOTE appears in a blockquote after the JSON code example block.
Agents reading the tutorial voice absorb the target contract, then hit the caveat.
Same risk: agent acts on post-B2 contract against pre-B2 binary.

**B1-AI-2 [BLOCKER] `"output"` field does not exist in any authoritative doc**
Draft says: "inspect `output` for the tool's own diagnostics"
error-contract §2.4 uses: `"stdout"`, `"stderr"`, `"duration"`
error-routing-decomposition B2b uses: `"tool_output"`
Neither doc uses `"output"`. The AI section draft teaches a field name that is wrong against both schemas.

**B1-AI-3 [BLOCKER] `init` stdout exception missing from draft text (same as B1-MCP-2)**
"Stdout is always clean on failure" — same false invariant for the current binary.

**B1-AI-4 [MODERATE] No example for search no-match retry loop**
CE-1 in the draft identifies that agents retry on `exit 1` without checking stderr.
The draft text gives the rule but no example reinforcing when to stop.
Retry-loop risk remains; CE-1 fix not applied.

**B1-AI-5 [LOW] Partial I/O failure doesn't clarify already-returned results are valid**
CE-5 notes this. Current text: "some files were unreadable" — agent may discard all output.
Fix not applied.

---

## Cross-draft tension

**CT-1 [CRITICAL] Three-way subprocess-capture field name conflict**
Three docs, three different shapes for the `exit_code` envelope:
  1. error-contract §2.4:       `{"exit_code","stdout","stderr","duration"}`
  2. error-routing-decomposition B2b: `{"exit_code","tool","tool_output"}`
  3. ai-section draft:           `{"exit_code","output",...}`

This is not a draft ambiguity — it is a design inconsistency between authoritative docs.
Neither B1 draft can be correct until the canonical shape is decided and written into error-contract.
This is a pre-condition for both drafts: resolve CT-1 before either ships.

**CT-2 [MODERATE] Pre-B2 timeline — both drafts can ship before B2 IF**
Both drafts describe post-B2 targets with pre-B2 caveats. This is acceptable provided:
  (a) NOTE moves above the target description (fixes B1-MCP-1 and B1-AI-1)
  (b) `init` stdout exception added to main text (fixes B1-MCP-2 and B1-AI-3)
Without both fixes, shipping before B2 teaches agents things that are false today.

---

## Verdicts

| Draft | Verdict | Blockers |
|---|---|---|
| failure-behavior-mcp-draft.md | REVISE | B1-MCP-1, B1-MCP-2, B1-MCP-3 (3 blockers) |
| failure-behavior-ai-section-draft.md | REVISE | B1-AI-1, B1-AI-2, B1-AI-3 (3 blockers) |

Neither draft is REJECT — the structure and coverage are correct. All blockers are fixable.

---

## What unblocks each

**Unblock MCP draft:**
1. Resolve CT-1: pick one canonical field name set for `exit_code` envelope; update error-contract §2.4.
2. Move pre-B2 NOTE to top of the `--json` section (above the envelope shape).
3. Add `init` exception to the stderr discipline paragraph.

**Unblock AI section draft:**
1. Same CT-1 resolution; replace `"output"` with the canonical field name.
2. Move pre-B2 NOTE blockquote above the JSON code example.
3. Add `init` exception to the "Stdout is always clean" bullet.

**Shared gate:**
CT-1 must be resolved first. It blocks both drafts. It requires a decision on the B2b JSON shape,
not just a doc edit.

---

## git status --short

```
 M AGENTS.md
 M CLAUDE.md
 M o8v-core/src/render/read_report.rs
 M o8v-testkit/Cargo.toml
 M o8v-testkit/src/benchmark/claude.rs
 M o8v-testkit/src/benchmark/experiment.rs
 M o8v-testkit/src/benchmark/mod.rs
 M o8v-testkit/src/benchmark/pipeline.rs
 M o8v-testkit/src/benchmark/report.rs
 M o8v-testkit/src/benchmark/store.rs
 M o8v-testkit/src/benchmark/types.rs
 M o8v/src/aggregator/sliding_window.rs
 M o8v/src/commands/read.rs
 M o8v/src/init/ai_section.txt
 M o8v/src/mcp/handler.rs
 M o8v/src/mcp/instructions.txt
 M o8v/tests/agent_benchmark.rs
 M o8v/tests/counterexamples_hook_redaction.rs
 M o8v/tests/e2e_cli.rs
 M o8v/tests/e2e_stats.rs
 M o8v/tests/e2e_stats_contract.rs
 M o8v/tests/e2e_stats_session.rs
 M o8v/tests/mcp_e2e.rs
 M o8v/tests/regression_orphan_session_filter.rs
?? docs/design/failure-behavior-adversarial-r1-2026-04-20.md
... (other untracked docs/design/ and docs/findings/ files)
```
