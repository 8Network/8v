# MCP Transport Cap — Empirical Measurement

**Date:** 2026-04-19
**Method:** Binary bisection via `mcp__8v-debug__8v read <path> --full` calls.

## Summary

The MCP tool-result transport layer enforces a hard cap between **60,500 and 63,336 characters** of output. 8v itself enforces no output size gate. The cap is enforced by Claude Code's MCP client.

## Two Observed Thresholds

| Threshold | Chars | Behavior |
|-----------|-------|----------|
| Display cap | ~57,000+ | Claude Code saves result to disk instead of showing inline. MCP transport succeeds. |
| Hard cap | 60,501–63,336 | MCP transport rejects the result. Hard error returned. |

## Bisection Results

All tests used `mcp__8v-debug__8v read <file> [<file2>] --full`.

| Files | Raw bytes (approx) | Output chars | Result |
|-------|--------------------|--------------|--------|
| `e2e_stats_contract.rs` alone | ~48,497 | ~57,200 | PASS (persisted to disk) |
| `e2e_stats_contract.rs` + `regression_run_id_uniqueness.rs` | ~51,098 | 60,500 | PASS (persisted to disk) |
| `e2e_stats_contract.rs` + `session_id.rs` | ~55,022 | 63,336 | FAIL |
| `e2e_stats_contract.rs` + `counterexamples_hook_redaction.rs` | ~63,490 | 72,202 | FAIL |

**Bracket:** largest PASS = 60,500 chars / smallest FAIL = 63,336 chars / gap = 2,836 chars.

## Exact Error Message (Verbatim)

```
result (63,336 characters across 1,606 lines) exceeds maximum allowed tokens
```

Pattern: `result (N characters across M lines) exceeds maximum allowed tokens`

## 8v Source Audit

File: `o8v/src/mcp/parse.rs`, line 9:

```rust
pub(super) const MAX_COMMAND_LEN: usize = 65_536; // 64 KB
```

This caps **incoming command string length only** (enforced at lines 43–48 with error "error: command exceeds maximum length"). Zero output size enforcement in 8v's MCP adapter.

## Cap Enforcer

**Claude Code's MCP client** — not 8v, not the Anthropic API. Evidence:
- 8v source has no output gate.
- Error message format ("result (N characters ... ) exceeds maximum allowed tokens") matches Claude Code's MCP client error vocabulary.
- The display-vs-fail two-threshold model is consistent with Claude Code client behavior (it first persists large results, then hard-fails above the true cap).

## Overhead Factor

`8v read --full` output is ~1.15–1.18x raw file bytes due to line-number prefixes, file headers, and separator lines.

## 8v Friction Observed

- No 8v-side feedback when approaching the cap. Agent learns the limit only from a hard failure.
- A pre-flight check or a warning at ~55K chars would prevent wasted calls.
- Batch `read --full` with multiple large files is the primary trigger. Single-file reads under ~52K raw bytes stay under the cap.
