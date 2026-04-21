# Test Reality Audit — Slice 1: `read --full` repeat-accept
**Date:** 2026-04-20
**Scope:** 2 tests in `o8v/tests/e2e_cli.rs`
- `read_double_full_flag_accepted` (lines 250–276)
- `read_triple_full_flag_accepted_matches_single` (lines 279–323)

**Production file under test:** `o8v/src/commands/read.rs`
**Feature:** `#[arg(long, overrides_with = "full")]` — makes repeated `--full` flags accepted silently

---

## Method

Applied 4 mutations to production code one at a time, ran the 2 target tests, recorded pass/fail, reverted each before the next. Baseline confirmed first (both tests pass on unmodified code). Each mutation was the only change in the file when tested.

---

## Mutation Results

| ID | Mutation | Expected | Actual | Verdict |
|----|----------|----------|--------|---------|
| M1 | Remove `overrides_with = "full"` entirely (`#[arg(long)]` only) | FAIL — clap rejects repeated flag (exit 2) | FAIL (exit 2, "cannot be used multiple times") | Tests catch this |
| M2 | Change to `overrides_with = "nonexistent_arg"` | FAIL — panic on invalid arg name reference | FAIL (exit 101, clap debug assertion panic) | Tests catch this (indirectly — via process exit, not by detecting semantics) |
| M3 | Add `action = clap::ArgAction::SetTrue` without `overrides_with` | FAIL — SetTrue same as default, rejects repeated flag | FAIL (exit 2, identical to M1) | Tests catch this |
| M4 | Add `.take(1)` to Full branch (return only first line) | SURVIVE — clap still accepts flags, `contains("hello")` still passes, single==triple still true | **SURVIVE** (both tests pass despite bug) | **Gap confirmed** |

---

## Verdict

**M1, M2, M3:** Tests catch all mutations that break the clap `overrides_with` mechanism. This is the feature the tests claim to verify — that repeated `--full` flags are accepted without error.

**M4: Gap confirmed.** The tests do NOT verify that `--full` returns correct file content. A mutation that silently truncates output to one line is completely undetectable by both tests:

1. `read_double_full_flag_accepted` checks `stdout.contains("hello")` — the first line of the two-line fixture `"hello\nworld\n"`. With `.take(1)`, "hello" is still present. "world" is silently lost.

2. `read_triple_full_flag_accepted_matches_single` checks `single.stdout == triple.stdout`. Both code paths hit the same `.take(1)` mutation, so both produce identically wrong output. The equality holds; no divergence is detected.

**The gap is structural, not incidental.** Test 2 was designed to catch flag-count variation causing behavioral divergence, not to verify correctness of a single invocation. If the single-flag code path is wrong, the triple-flag comparison is equally wrong. Equality between two wrong results is not correctness.

---

## Root Cause of Gap

The two tests verify a **single correctness property**: repeated `--full` flags are accepted (exit 0) and produce consistent output. They do not verify the **content property**: that `--full` returns all lines of the file.

This is a narrow but real gap. The fixture is `"hello\nworld\n"` (2 lines). Any mutation that returns a strict non-empty prefix of the file while leaving "hello" intact survives both tests.

---

## New Test Added

`read_full_returns_all_lines` was added to `o8v/tests/e2e_cli.rs`. It:
- Creates `fixture.txt` with `"hello\nworld\n"` (same fixture as the existing tests)
- Runs `8v read --full <path>`
- Asserts exit 0
- Asserts `stdout.contains("hello")` (first line present)
- Asserts `stdout.contains("world")` (second line present — catches M4 truncation)

This test fails under M4 (`.take(1)` applied) and passes on production code. It is the minimal addition that closes the content-completeness gap without duplicating the acceptance behavior already covered.

---

## 8v Feedback

**Friction observed during this session:**

1. **`8v read --full` on the target file returns only one line of content.** When reading `o8v/src/commands/read.rs --full` to inspect the M4-mutated file, only the first line was returned. This is surprising behavior — `--full` implies complete file content. If the MCP tool is applying a line cap internally, that cap is invisible to the caller and breaks the progressive disclosure contract ("--full is the last resort, returns everything"). This friction directly delayed confirming the M4 revert because the output was ambiguous.

2. **No output when `8v read path:N-M` range is adjacent to a mutation site.** Had to fall back to `Read` (native tool) to confirm the exact state of lines 147–161. The 8v read range command would have been the right tool, but prior context showed it sometimes returns nothing for small ranges on certain file types.

3. **Batching `read` with `--full` is useful.** No friction here — running `8v read a.rs b.rs --full` to inspect both the test file and production file simultaneously worked as documented.

---

## Files Changed

- `o8v/tests/e2e_cli.rs` — added `read_full_returns_all_lines` test (1 new test)
- `o8v/src/commands/read.rs` — unchanged (all mutations reverted; production code is identical to pre-audit state)
- `docs/findings/test-reality-audit-1-2026-04-20.md` — this file
