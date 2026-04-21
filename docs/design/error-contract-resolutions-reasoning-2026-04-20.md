# Reasoning Trace: CE-2 and CE-3 Resolutions

**Date**: 2026-04-20
**Applies to**: `docs/design/error-contract.md` §7
**Scope**: Level 1 only — what was decided and why. No implementation detail.

---

## CE-2: `search` partial failure — harvest or fail-fast?

### What was considered

Three options:
- **(a) Harvest with visible warnings** — continue past unreadable files, emit `error: permission denied: <path>` to stderr per file.
- **(b) Fail-fast** — abort on first unreadable file, emit the error, return nothing.
- **(c) `--allow-partial` flag** — user-controlled. Ruled out immediately: §4 of the design explicitly prohibits new flags for this slice. Not a judgment call.

The real choice was (a) vs. (b).

### Why (a) was picked

Evidence: `command-qa-search-2026-04-20.md`, Issue 2 and Issue 3.

Issue 2 documents that exit 1 currently means three distinct things (no match, read error, invalid argument) and that STDERR is always empty — so the problem is invisible failures, not the wrong traversal strategy. Issue 3 documents that chmod-000 files are already counted in the `files_skipped` footer in the current binary — meaning the traversal already continues; the bug is that the skip is silent.

If the traversal already continues (empirically), fixing the bug means making the skip visible (stderr warning), not changing the traversal strategy to fail-fast. Fail-fast would require a behavior reversal for a case that already works correctly at the traversal level.

Second evidence point: `error-contract-measurement-2026-04-20.md`, Section 4: "search silently skips unreadable files — this is partial-failure without signaling — the worst variant." The word "signaling" is the lever. The worst variant is not harvest per se — it is harvest without a signal. Fix the signal.

Ripgrep reference behavior confirms: continues past unreadable files, emits per-file errors to stderr, exits 0 if matches found regardless of I/O warnings. The Unix convention for search tools is harvest-and-warn, not fail-fast.

### What could still be wrong

The exit-code contract leaves a residual ambiguity: `exit 1 + non-empty stderr` means "partial I/O failure" but gives no count of how much work succeeded. An agent cannot determine "I got 80% of results" vs. "I got 0% of results" from exit code alone. The agent must parse stderr to count how many files were skipped. This is acceptable at Level 1 — the instruction surfaces must document this explicitly. If measurement later shows agents misread this signal, the fix is in the instruction text, not the exit-code contract.

A second residual: the `exit 1 + stderr empty = genuine no match` rule depends on stderr being reliably empty when there are no I/O errors. If any future code path emits to stderr on a clean no-match, this discriminant breaks. Level 2 must enforce stderr silence on clean paths.

---

## CE-3: Two `--json` schemas — conflict or feature?

### What was considered

Three options:
- **(a) Pass-through** — emit subprocess JSON/text as-is, no 8v envelope.
- **(b) Always-wrap** — all errors, including subprocess output, go through `{"error":...,"tool_output":{...}}`.
- **(c) Two-level** — 8v-side errors use `{"error":...,"code":...}`; subprocess output uses `{"exit_code":...,"tool":...,"output":...,"duration_ms":...}`. Top-level key disambiguates.

### Why (c) was picked

§2.4 of the design doc already described option (c) as "the explicit exception" — the two-level split was the implicit design choice before CE-3 was written. CE-3 exposed that the design had not named the schemas or specified the agent's disambiguation path. The resolution is not a new decision; it is formalizing the existing implicit design.

Evidence: `error-contract-measurement-2026-04-20.md` confirms that `check --json` on error emits plain-text to stderr and nothing to stdout — neither schema is honored today. The gap is the pre-run failure case (tool not installed, project not found). Option (a) pass-through leaves this case as plain-text stderr — still broken. Option (b) adds a wrapping layer that requires the agent to double-parse — cost with no benefit.

The top-level key discriminant (`"error"` vs. `"exit_code"`) requires zero new logic from the agent: a JSON parser reading the response object already has the key set. The agent branches on which key is present. No prefix scanning, no exit-code cross-referencing.

### What could still be wrong

The critical open question is the pre-run failure path for verify commands. Today `8v check /nonexistent --json` emits plain-text stderr. The Level 1 design says this path must emit `{"error":...,"code":...}` to stdout instead. If Level 2 does not implement this path, the agent faces: `exit 1 + stdout empty + stderr has plain text` — which matches neither documented schema. An agent trained on the documented contract would not know how to handle it.

This is not a flaw in the Level 1 design — it is a Level 2 implementation requirement. The risk is that Level 2 ships the subprocess-capture path but misses the pre-run failure path. The mitigation is a failing test: write `8v check /nonexistent --json`, assert stdout is valid JSON with `"error"` key, assert stderr is empty. If this test is not in the acceptance criteria for Level 2, it will be missed.

---

## Cross-cutting observation

Both CE-2 and CE-3 turned out to be cases where the design doc had already made the right implicit choice — the CEs exposed that the choices were not named or justified, not that they were wrong. The resolution work was mostly formalization, not revision. This is the expected output of a counterexample review pass on a well-formed Level 1 design.

The one genuine design decision was the exit-code discriminant for CE-2 (`exit 1 + stderr empty` vs. `exit 1 + stderr non-empty`). This was not in the original design and had to be added. It is a new clause in the contract, not a bug fix.
