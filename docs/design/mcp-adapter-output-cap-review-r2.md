# Review R2: MCP Adapter Output Cap Design

**Reviewer:** Claude Sonnet 4.6 · **Date:** 2026-04-19  
**Design:** `docs/design/mcp-adapter-output-cap.md` · **Round:** 2 of N

---

## r1 Regression

| ID | Original finding (short) | Fixed? | Evidence |
|----|--------------------------|--------|----------|
| B1 | Pre-flight location lacks `caller` — doesn't compile | Fixed | Design moved to `handler.rs` before `dispatch_command_with_agent`. `if let Command::Read(args) = &parsed_command` checks the already-parsed enum — no `Caller` needed. Insertion point at lines 65–67 matches actual source exactly. |
| TG1 | Test 2 mechanism for oversized output unspecified | Fixed | §9 Test 2 now specifies: `O8V_MCP_OUTPUT_CAP=1000`, fixture directory large enough that `ls --tree` output exceeds 1000 chars. Mechanism is concrete. |
| TG2 | Env var edge cases not tested | Fixed | §9 Test 4 is parameterized over `{"0", "-1", "abc", ""}`, each must produce a distinct observable error before any command executes. |
| R1 | `use_stderr` branch bypasses post-render check | Fixed | §5 pseudocode wraps both arms: cap check runs against `out` before branching on `use_stderr`. Matches handler.rs lines 77–84 structure exactly. |
| R2 | Persist threshold lower bound unproven | Accepted | §10 R2 note documents the empirical basis and acknowledges the margin is not measured from below. Acceptable for a design. |
| R3 | 1.20× factor not validated for multi-file batch | Accepted | §10 R3 note documents that post-render catches overhead overruns; pre-flight may underestimate but cannot over-block. Acceptable. |
| N1 | "Parse at startup" ambiguous | Fixed | §3 now says "parsed on the first `handle_command` call and cached for the process lifetime." |
| N2 | Error template indentation not prescribed | Fixed | §6 now says "implementation must match this template verbatim (indentation included) for test stability." |

---

## New Blockers

None.

---

## New Test Gaps

**NTG1 — Test 1 "no `read_one` call" assertion is untestable as stated**  
§9 Test 1: "no file content read (verify via metadata-only code path, no `read_one` call)."  
`read_one` is a private function in `read.rs`. Integration tests in `o8v-cli` cannot assert it was not called — there is no observable side-effect to check (no log, no counter, no event). The test can verify the error message is returned and file content is absent from the output, but the "no `read_one` call" clause has no implementable assertion path.  
Resolution: replace with an observable proxy — e.g., assert response time is under N ms (metadata read is fast; full read is slow), or assert the error fires even when the fixture file is unreadable (permissions removed). Or simply drop the assertion and rely on the content-absent check.

---

## New Risks

**NR1 — `once_cell`/`OnceLock` thread-safety: design asserts single-threaded but does not state it**  
§3: "cached for process lifetime." §11: "All insertion points grounded by source read."  
The MCP server uses `rmcp` (async). If `handle_command` is `async` and the runtime is multi-threaded (Tokio default), the env-var parse-and-cache path races on first call. The design proposes a `OnceLock<u64>` or similar but does not name the primitive. If implemented with a plain `static mut` or a non-atomic cell, this is a data race under MIRI.  
Severity: low in practice (parse of a `u64` from env is unlikely to race meaningfully), but the design should name the primitive — `std::sync::OnceLock` is the correct answer and is `Send + Sync`.

---

## New Nits

**NN1 — `§9 Test 1` fixture size is inconsistent with cap override**  
Test 1 sets `O8V_MCP_OUTPUT_CAP=1000` but requires a fixture "≥ 55,000 bytes." A 1,000-byte fixture would suffice for the pre-flight condition (`1000 * 1.20 = 1200 > 1000`). The 55,000-byte requirement is leftover from the default-cap assumption. A smaller fixture is faster and equally effective; the large fixture may slow CI.

**NN2 — §4 source anchor code block has a syntax error**  
Line 26: `match super::parse::parse_mcp_command(...)?` is missing the opening brace. Not a design correctness issue, but the anchor will confuse implementers. Change to `match super::parse::parse_mcp_command(...)?  {`.

---

## Summary

| Category | Count |
|----------|-------|
| r1 regressions resolved | 8 / 8 |
| New blockers | 0 |
| New test gaps | 1 (NTG1) |
| New risks | 1 (NR1) |
| New nits | 2 (NN1, NN2) |

**Recommendation: proceed with one-line fixes.** No blockers. NTG1 requires rewriting one assertion clause in the test plan (5 minutes). NR1 requires naming the caching primitive in §3 (one sentence). Neither warrants blocking the implementation. Address NTG1 + NR1 inline before coding starts; NN1 + NN2 can be fixed during implementation.
