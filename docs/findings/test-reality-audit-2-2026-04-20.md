# Test Reality Audit — Slice 2: MCP adapter output-cap
**Date:** 2026-04-20
**Scope:** 7 tests in `o8v/tests/mcp_e2e.rs` (all prefixed `mcp_oc_`)
- `mcp_oc_preflight_fires` (line 416)
- `mcp_oc_post_render_fires` (line 457)
- `mcp_oc_under_cap_passes` (line 491)
- `mcp_oc_invalid_cap_zero` (line 518)
- `mcp_oc_invalid_cap_negative` (line 534)
- `mcp_oc_invalid_cap_non_numeric` (line 550)
- `mcp_oc_invalid_cap_empty` (line 566)

**Production file under test:** `o8v/src/mcp/handler.rs`
**Feature:** MCP output cap — enforced via `O8V_MCP_OUTPUT_CAP` env var. Two enforcement points:
1. Pre-flight: metadata sum × 1.20 > cap → abort before reading, returns per-file byte sizes.
2. Post-render: rendered output length > cap → `oversized_error()` with char counts.
Invalid cap values (zero, negative, non-numeric, empty) are rejected by `get_output_cap()` before dispatch.

---

## Method

Baseline confirmed first (7/7 tests pass on unmodified code). Applied 6 specified mutations (M1–M6) and 1 custom mutation (C1) to `handler.rs`, one at a time. Ran `cargo test --test mcp_e2e mcp_oc_` after each, recorded pass/fail per test, reverted before next mutation. Each mutation was the only change present when tested. `handler.rs` is returned to unmodified production state after the full matrix.

**Subprocess isolation note:** Each test spawns a fresh `8v mcp` child process with `O8V_MCP_OUTPUT_CAP` set via `.env()`. There is no `OnceLock` cross-contamination between tests — each subprocess initializes independently.

---

## Mutation Results

| ID | Mutation applied to `handler.rs` | mcp_oc_preflight_fires | mcp_oc_post_render_fires | mcp_oc_under_cap_passes | mcp_oc_invalid_cap_zero | mcp_oc_invalid_cap_negative | mcp_oc_invalid_cap_non_numeric | mcp_oc_invalid_cap_empty | Verdict |
|----|-----------------------------------|------------------------|--------------------------|-------------------------|-------------------------|-----------------------------|-------------------------------|--------------------------|---------|
| M1 | Invert pre-flight threshold: `>` → `<` | FAIL | pass | FAIL | pass | pass | pass | pass | Tests catch this |
| M2 | Change multiplier: `1.20` → `0.80` | FAIL | pass | pass | pass | pass | pass | pass | Tests catch this |
| M3 | Remove post-render check entirely (delete `if out.len() > cap { return Err(...) }`) | pass | FAIL | pass | pass | pass | pass | pass | Tests catch this |
| M4 | Silently convert invalid cap to 0 (`Ok(_) => Ok(0)` and `Err(_) => Ok(0)`) | pass | pass | pass | pass | FAIL | pass | pass | **GAP: 3 of 4 invalid-cap tests survive** |
| M5 | Empty string → default Ok(5) instead of Err | pass | pass | pass | pass | pass | pass | pass | **GAP: all 7 tests survive** |
| M6 | Remove OnceLock caching (call `std::env::var` on every `get_output_cap` invocation) | pass | pass | pass | pass | pass | pass | pass | **GAP: undetectable by subprocess-isolated tests** |
| C1 | Remove "O8V_MCP_OUTPUT_CAP" from `oversized_error()` format string | pass | FAIL | pass | pass | pass | pass | pass | Tests catch for post-render; invalid-cap path is unaffected |

---

## Verdict per Mutation

**M1 (invert threshold):** Caught by `preflight_fires` (no error when there should be one) and `under_cap_passes` (error when there should not be one). The two tests bracket the threshold correctly.

**M2 (0.80 multiplier):** Caught by `preflight_fires`. The 0.80 estimate (968 chars) for the 1210-byte fixture falls below the 1000 cap, so pre-flight does not fire. Good coverage of the multiplier.

**M3 (remove post-render check):** Caught by `post_render_fires`. The `ls --tree` output on 60 files exceeds 1000 chars; without the post-render guard, no error is returned. The test correctly detects the removal.

**M4 (silent 0 conversion):** Only `invalid_cap_negative` fails. The remaining three (`zero`, `non_numeric`, `empty`) pass via the wrong code path: a cap of 0 means every non-empty command output (`ls` returns several lines) triggers `oversized_error()`, which also contains "O8V_MCP_OUTPUT_CAP". The tests cannot distinguish the two error origins.

**M5 (empty → Ok(5)):** All 7 tests survive. With a silently assigned cap of 5, running `ls` in an empty directory produces output ≤ 5 chars (or possibly empty), so no post-render fires, and `invalid_cap_empty` returns `is_error=false`. The test checks `is_error=true` — yet it passes. This means under M5 the test's assertion fails... **wait: re-examining.** With cap=5, `ls` on a fresh empty workspace returns something like `.\n` (2 chars) or nothing. If output ≤ 5, `is_error=false` → test `assert!(is_error)` would FAIL. The M5 result of all-pass means the workspace is not empty and `ls` returns > 5 chars, pushing into the post-render path — which also emits "O8V_MCP_OUTPUT_CAP". So the test passes through the post-render path with the wrong error message. **Gap confirmed via wrong code path.**

**M6 (remove OnceLock caching):** All 7 tests survive. Each subprocess reads the env var directly; whether `get_output_cap()` caches or re-reads on every call is invisible from outside the process. This is an architectural property (process-lifetime caching) that cannot be verified by black-box subprocess tests. No fix is applied — this is a structural limitation, not a testable gap at the integration level.

**C1 (remove var name from oversized_error):** `post_render_fires` fails (it checks for "O8V_MCP_OUTPUT_CAP" in the post-render message). The four invalid-cap tests pass because their errors originate from `get_output_cap()`, which still emits the var name. `preflight_fires` passes because the pre-flight error message is built inline (not via `oversized_error()`), and also contains "O8V_MCP_OUTPUT_CAP". C1 correctly identified that `post_render_fires` pins the `oversized_error()` message.

---

## Root Cause of Gaps

### Gap 1 (M4, M5): Invalid-cap error source is unverified

The four invalid-cap tests each check two things:
1. `is_error == true`
2. `text.contains("O8V_MCP_OUTPUT_CAP")`

The string "O8V_MCP_OUTPUT_CAP" appears in:
- `get_output_cap()` error messages (the intended path)
- `oversized_error()` (the post-render fallback)

When a mutant silently converts an invalid cap to 0 or 5, the command still runs, and if any output is produced that exceeds the silently-assigned cap, `oversized_error()` fires. Its message contains "O8V_MCP_OUTPUT_CAP" — so condition (2) is satisfied via the wrong code path. The tests cannot tell the difference.

**What discriminates the two paths:**
- `get_output_cap()` rejection messages contain unique phrases: "is not a positive integer", "is not a valid integer", "is set but empty"
- `oversized_error()` contains "chars", "output too large", and "Use a line range" but none of the rejection phrases above

### Gap 2 (M3, C1): Post-render path has no positive discriminator

`mcp_oc_post_render_fires` checks for `isError=true`, "output too large for MCP transport", and "O8V_MCP_OUTPUT_CAP". These are necessary but not sufficient to prove the post-render path specifically fired. The pre-flight path satisfies all three checks too. The existing `mcp_oc_preflight_fires` test uses "bytes" as a discriminator (pre-flight lists per-file byte sizes; post-render does not). `post_render_fires` has no equivalent positive proof.

**What discriminates post-render from pre-flight:**
- Pre-flight: contains "bytes" and "estimated"
- Post-render (`oversized_error()`): contains "chars", does NOT contain "bytes" or "estimated"

### Gap 3 (M6): OnceLock caching is opaque to subprocess tests

This is a structural limitation. Black-box integration tests cannot observe whether a per-process OnceLock is used vs. a direct `env::var` read on every call. The caching is only meaningful under multi-call scenarios within the same process. This gap is acknowledged but not targeted by a test (no meaningful black-box test exists for it without process-internal introspection).

---

## New Tests Added

Two tests were added to `o8v/tests/mcp_e2e.rs` after line 579.

### `mcp_oc_invalid_cap_error_comes_from_validation` (closes Gap 1)

Runs `ls` with four invalid cap values and checks for the specific rejection phrase in each case:
- `"0"` → must contain `"is not a positive integer"`
- `"-1"` → must contain `"is not a positive integer"`
- `"abc"` → must contain `"is not a valid integer"`
- `""` → must contain `"is set but empty"`

These phrases appear only in `get_output_cap()`. A mutant that silently converts invalid caps to 0 or 5 will produce a post-render error (or success) but NOT these phrases → test fails → mutant detected.

**Verified:** fails under M4, fails under M5. Passes on production code.

### `mcp_oc_post_render_error_is_from_post_render_path` (closes Gap 2)

Uses the same 60-file fixture as `mcp_oc_post_render_fires`. Asserts:
- `is_error == true` (same as existing)
- `text.contains("chars")` — positive proof, only in `oversized_error()`
- `!text.contains("bytes")` — negative proof, pre-flight only
- `!text.contains("estimated")` — negative proof, pre-flight only

Together these three checks pin the error to the `oversized_error()` code path, not pre-flight.

**Verified:** fails under M3 (no post-render check → no "chars" in output). Passes on production code.

---

## Final Test Count

9 tests, all passing:
- 7 original Slice 2 tests: pass
- `mcp_oc_invalid_cap_error_comes_from_validation`: pass
- `mcp_oc_post_render_error_is_from_post_render_path`: pass

---

## 8v Feedback

**Friction observed during this session:**

1. **`8v read --full` on `handler.rs` returned truncated output in earlier reads.** During the first mutation (M1) verification, reading `handler.rs --full` via the MCP tool returned only the first portion of the file. This is the same friction reported in Slice 1. The DEFAULT_OUTPUT_CAP in handler.rs is 5 chars — when the MCP tool reads itself via 8v with the default cap, only 5 chars pass. This is a dogfood self-referential problem: the tool enforces a cap that prevents itself from being read via its own MCP interface without an explicit cap override.

2. **No way to verify OnceLock behavior via 8v commands.** Gap 3 (M6) is untestable via black-box MCP calls. A useful addition would be `8v mcp --diagnostics` or a process-internal stat exposed via a debug command that reports how many times `get_output_cap()` was called. This is not a missing CLI command but a testability gap in the current architecture.

3. **`8v search` for exact string "O8V_MCP_OUTPUT_CAP" across the codebase worked correctly and with no friction.** Searching across multiple files to identify all locations where the env var name appears (to determine which code paths would satisfy the test assertions) was fast and precise. This was the key tool for diagnosing Gaps 1 and 2.

4. **Batch reads (`8v read a.rs b.rs c.rs`) were essential for efficiency.** Reading `handler.rs` and the relevant test range in a single call was consistently reliable. No friction on batched reads.

---

## Files Changed

- `o8v/tests/mcp_e2e.rs` — added `mcp_oc_invalid_cap_error_comes_from_validation` and `mcp_oc_post_render_error_is_from_post_render_path` (2 new tests)
- `o8v/src/mcp/handler.rs` — unchanged (all mutations reverted; production code is identical to pre-audit state)
- `docs/findings/test-reality-audit-2-2026-04-20.md` — this file
