# Instruction Clarity Benchmark — v4 Findings
Date: 2026-04-20
Models tested: Claude Opus 4.7 (N=3), Claude Sonnet 4.6 (N=3)
Surfaces under test: `o8v/src/mcp/ai_section.txt` (Surface 1) and `o8v/src/mcp/instructions.txt` (Surface 2)
Reference: [v3 findings](instruction-clarity-test-2026-04-19-v3.md)

---

## 1. Summary Table

| Run | Model | Axis 1 Input | Axis 2 Output | Axis 3 Failure | Composite |
|-----|-------|-------------|--------------|----------------|-----------|
| opus-run1 | Claude Opus 4.7 | 8 | 6 | 3 | 5.67 |
| opus-run2 | Claude Opus 4.7 | 8 | 6 | 3 | 5.67 |
| opus-run3 | Claude Opus 4.7 | 8 | 6 | 3 | 5.67 |
| sonnet-run1 | Claude Sonnet 4.6 | 7 | 5 | 2 | 4.67 |
| sonnet-run2 | Claude Sonnet 4.6 | 8 | 6 | 2 | 5.33 |
| sonnet-run3 | Claude Sonnet 4.6 | 8 | 6 | 2 | 5.33 |

**v4 mean composite**: (5.67 + 5.67 + 5.67 + 4.67 + 5.33 + 5.33) / 6 = **5.39**

### Δ vs. prior versions

| Version | Axis 1 (Input) | Axis 2 (Output) | Axis 3 (Failure) | Composite |
|---------|---------------|----------------|-----------------|-----------|
| v2 | — | — | — | 6.17 (single-axis; not comparable) |
| v3 | 8.00 | 4.67 | 2.17 | 4.94 |
| v4 (Opus) | 8.00 | 6.00 | 3.00 | 5.67 |
| v4 (Sonnet) | 7.67 | 5.67 | 2.00 | 5.11 |
| v4 (overall) | 7.83 | 5.83 | 2.33 | 5.39 |

v2 used a single-axis Likert scale and is structurally incomparable. All v3/v4 figures use the same three-axis rubric.

---

## 2. Axis Breakdown — What Changed Between v3 and v4

### Axis 1 — Input clarity (7.83 v4 vs. 8.00 v3)

No regression. All six runs confirmed that command syntax, flag lists, and `write` variants are well-documented. The slight drop from 8.00 to 7.83 is attributable to sonnet-run1 scoring 7 rather than 8, citing the `write` escape syntax lacking a concrete shell-safe example (Q11, Q20: "the `\n` as literal two-character sequence vs. shell behavior is confusing; I'd need a concrete before/after example").

Slices that affected Axis 1: None in v4. Input was already saturated at v3.

### Axis 2 — Output clarity (5.83 v4 vs. 4.67 v3)

**+1.17 improvement.** This is the sharpest axis improvement across the version transition.

Slices responsible:
- **Slice 4**: Added batch delimiter contract (`=== <label> ===`) and symbol-map example output lines to Surface 1. All six runs demonstrated they could correctly predict batch output format (Q10, Q25: "I know this from the explicit batch output contract in both surfaces" — sonnet-run3). Opus runs scored full credit on this question; Sonnet runs also scored correctly.
- **Slice 4** (MCP hint): Surface 2 now includes the batch output contract section. Prior to v3 this was absent from Surface 2, causing the asymmetry that depressed Output scores.

Remaining gap: verify commands (`8v test .`, `8v build .`, `8v fmt .`, `8v check .` output format) are entirely undocumented on success. All six runs independently flagged this. sonnet-run3 Q13: "check, fmt, test, build: Description only — [no output format shown]." This caps Output at 6 rather than higher.

### Axis 3 — Failure-mode clarity (2.33 v4 vs. 2.17 v3)

**+0.16 improvement — effectively unchanged.**

The only documented failure case across both surfaces remains: "`--find/--replace` fails if `<old>` not found." No exit-code table, no error-output channel, no `--json` error envelope, no file-not-found behavior for any command.

Opus runs scored 3/10 on this axis; Sonnet runs scored 2/10. The marginal Opus advantage reflects that Opus runs gave partial credit to the single documented failure case (`--find` not found) and the one `check .` line ("Non-zero exit on any issue"). Sonnet runs applied stricter partial-credit accounting.

No slices shipped between v3 and v4 targeted the failure surface. The +0.16 delta is within noise.

---

## 3. Consensus Friction — All 6 Runs

The following frictions were raised independently in all 6 runs. Each citation identifies the specific question and run.

### [P0] Error/exit-code contract missing

**Problem**: No documentation of: exit codes per command, which channel carries errors (stdout vs. stderr), JSON error envelope shape on failure, file-not-found behavior for any command.

**Measurement**: Axis 3 score = 2–3/10 across all runs. Q7, Q18, Q26, Q27, Q28, Q29 were blocked by this gap in every run.

**Root cause**: Error contract was never written. Both surfaces describe success paths only.

**Citations**:
- opus-run1, Q22: "Add a contract table at the bottom: for each command, exit code on success/failure, error channel, JSON error shape."
- opus-run2, Q22: "Add Errors & Exit Codes section with success/failure exit, stderr vs stdout, JSON error schema shape, one sample failure." 8v feedback: "Five of my thirteen gap callouts (Q7, Q18, Q26, Q27, Q28, Q29) are all the same root cause: no documented error model."
- opus-run3, Q22: "Add concrete error/exit-code contract table (one row per command)." 8v feedback: "Five of the 33 questions (Q18, Q26, Q27, Q28, Q29) are blocked by the same gap."
- sonnet-run1, Q24: "Add a single 'Error contract' paragraph: 'On any error, 8v exits non-zero, prints a human-readable message to stderr, and (with `--json`) returns `{"error": "<message>"}` to stdout.'"
- sonnet-run2, Q22: "Add an error-behavior subsection under each command showing: (1) the exit code on success and failure, (2) what appears on stdout vs. stderr on failure, (3) the `--json` shape for errors."
- sonnet-run3, Q22: "Add a Failure behavior section that documents: (1) exit codes for each command, (2) where errors appear (stdout vs. stderr), and (3) the JSON error envelope shape."

All six runs named the same root cause. All six recommended the same fix.

### [P0] `write --find/--replace` multiple-occurrence behavior undefined

**Problem**: The instructions state that `--find/--replace` "fails if `<old>` not found" but do not define behavior when `<old>` appears more than once. Agents cannot determine whether the operation replaces the first match, all matches, or errors on ambiguity.

**Measurement**: Q27 in all six runs; also flagged as a top-3 likely mistake in Q14 across multiple runs.

**Root cause**: The specification covers the zero-match case but omits the N>1 case.

**Citations**:
- opus-run1, Q27: "Multiple occurrences: not stated — my guess is it replaces all occurrences, but this is pure inference."
- opus-run2, Q27: Flagged `--find/--replace` scope as a "booby trap" (Q14 and Q27).
- opus-run3, Q14: "The single most dangerous undefined behavior: `write --find/--replace` on a file with multiple occurrences."
- sonnet-run1, Q27: "More than one occurrence: not stated."
- sonnet-run2, Q14: Characterized multiple-match as a "booby trap."
- sonnet-run3, Q27: "More than one occurrence: Instructions don't say — my guess: replaces all occurrences or fails with an ambiguity error. Gap: not documented."

### [P1] `ls` bare form not described

**Problem**: The instructions show `8v ls --tree --loc` as the canonical invocation and say "Start here." What bare `8v ls` returns (without flags) is never stated.

**Measurement**: Flagged in Q19 (missing items) across multiple runs.

**Citations**:
- opus-run1, Q19: "Bare `8v ls` — no description of what it returns without flags."
- opus-run2, Q19: Same gap listed.
- sonnet-run2, Q19: "`ls` default scope (bare `8v ls`) not described."
- sonnet-run3, Q19: "What does bare `8v ls` return? Only `--tree --loc` is shown."

Opus-run3 and sonnet-run1 did not separately call this out by name, though both scored the ls section identically.

### [P1] `(regex)` token opaque — dialect, behavior on failure, and literal mode unspecified

**Problem**: The search command signature shows `8v search <pattern> (regex)` but does not specify which regex dialect (PCRE, ERE, literal), whether the pattern is always a regex or literal by default, whether regex failures surface as errors or empty results, or how to search for regex metacharacters literally.

**Measurement**: Flagged in Q6 (ambiguous phrases), Q8 (undefined terms), Q14 (likely mistakes), and Q19 (missing items) in multiple runs.

**Citations**:
- opus-run1, Q6 and Q8: "regex dialect not stated."
- opus-run2, Q19: "Regex dialect unstated."
- opus-run3, Q14: "Regex dialect unstated; escaping metacharacters not documented."
- sonnet-run1, Q14: "regex support implied by `(regex)` in search signature, but no explicit regex example."
- sonnet-run2, Q6: Noted `(regex)` parenthetical is ambiguous.
- sonnet-run3, Q6: "`(regex)` — ambiguous about quoting and literal vs. regex mode."

### [P1] Surface 1 ↔ Surface 2 wording drift

**Problem**: The two surfaces contain the same content but differ in phrasing and structure in several ways. All runs identified these differences in Q30.

**Citations (per run)**:
- opus-run1, Q30: "Surface 2 says 'shell tools' vs. Surface 1 'Bash'; Surface 2 omits 'One call beats N sequential calls.'"
- opus-run2, Q30: "Surface 2 says 'shell tools'; Surface 1 says 'Bash'. Both are clear but differ."
- opus-run3, Q30: "95% duplicate but failure-mode contracts missing from both."
- sonnet-run1, Q30: Surface differences in bullet formatting, batch rationale sentence presence in S1 only.
- sonnet-run2, Q30: "S2 adds '## Write — prefer targeted edits', intro orientation sentence, discovery purpose clause. S1 has 'One call beats N sequential calls' reinforcement."
- sonnet-run3, Q30: (1) formatting bullet vs. bare line; (2) S1 has "One call beats N sequential calls" not in S2; (3) S2 has orientation tagline not in S1; (4) S2 write header more prescriptive.

The drift is presentational, not factual. No run identified a factual contradiction between surfaces. However, multiple runs noted that the missing reinforcement sentence in Surface 2 ("One call beats N sequential calls") weakens the batch rationale for agents seeing only the MCP description.

---

## 4. What Moved and Why

### Slices shipped between v3 and v4

The following slices were applied to the instruction surfaces. The v3 document listed them as recommendations; this section records which ones landed.

**Slice 4**: Batch delimiter contract added to both surfaces. Symbol-map example output lines added to Surface 1. MCP description (Surface 2) received the batch output contract section. **Effect**: Output axis +1.17. All six runs correctly predicted batch delimiter behavior (Q10). Prior to Slice 4, the batch output format was absent from Surface 2; this was the primary driver of the v3 Output score of 4.67.

**Slice 4b**: Surface 2 orientation tagline added ("8v — code reliability tool for AI agents. Designed to minimize round-trips."). Surface 2 write section header changed to "## Write — prefer targeted edits". **Effect**: sonnet-run2 and sonnet-run3 noted the tagline positively in Q30. sonnet-run1 did not reference it. No axis score impact identified.

**Slice 4c**: "One call beats N sequential calls" reinforcement sentence added to Surface 1. **Effect**: All six runs reproduced the principle in Q3 correctly. Absent from Surface 2, which multiple runs flagged in Q30.

### What did not move

**Failure surface**: No slice addressed the error/exit-code contract between v3 and v4. Axis 3 improved by +0.16 (within noise). The v3 recommendation B1 ("add failure-mode contracts") was not implemented.

**Input surface**: Already saturated. No regression, no improvement expected.

---

## 5. Next Slice Recommendation — Document the Error/Exit-Code Contract

**Recommendation**: Implement slice B1 from the v3 roadmap. Add a `## Failure behavior` section to both surfaces.

**Minimum content** (based on Q22/Q24 consensus across all 6 runs):

```
## Failure behavior
All commands exit 0 on success, non-zero on failure.
Error messages go to stderr.
With --json, errors return {"error": "<message>"} on stdout alongside the non-zero exit.
If a path does not exist, `read` and `write` both exit non-zero.
`write --find/--replace` replaces all occurrences if `<old>` matches more than once.
```

**Expected effect**: Axis 3 from 2.33 → 6–7, raising composite from 5.39 → ~6.5.

**Priority**: P0. Six independent evaluators identified this as the single highest-impact edit. The current score of 2.33 on a 10-point scale means agents cannot reason about any failure path except the one partially documented (`--find` not found).

**Secondary item**: Pin `write --find/--replace` multi-occurrence behavior. Either "replaces all" or "errors on ambiguity" — whichever is true. The spec is currently silent and all six evaluators noted they had to guess.

---

## 6. Honest Caveats

1. **N=3 per model.** Three runs per model is below the variance threshold for statistical confidence. The Opus composite of 5.67 and Sonnet composite of 5.11 are consistent within model but the inter-model difference is 0.56 — within the range that could shift with additional runs.

2. **Single evaluator prompt.** All 33 questions were issued in one prompt per run with no follow-up probing. The scores reflect first-pass understanding, not adversarial comprehension testing. A deeper test — for example, asking the model to act on the instructions and observing what breaks — would surface additional gaps not visible in Q&A scoring.

3. **Scoring derived from conversation only.** The rubric asked models to self-score. Self-scoring conflates self-confidence with clarity. A model that infers well despite documentation gaps may score higher than one that identifies gaps more precisely. The Sonnet/Opus difference on Axis 3 (2 vs. 3) may partly reflect this dynamic: Opus gave partial credit to the two documented failure behaviors; Sonnet applied a stricter reading.

4. **Sample size.** Both surfaces together total approximately 650 words. This is a small documentation surface. At this size, N=3 per model is coarser than it would be for a larger corpus.

5. **v2 incomparability.** v2 used a single-axis Likert scale. The headline of 6.17 cannot be compared to v3/v4 three-axis composites. Any statement like "v4 is lower than v2" would be structurally invalid.

---

## 7. 8v Feedback

Frictions reported by the models during the six evaluation sessions. These are distinct from the instruction-clarity findings — they reflect tool-use friction during the session itself.

**[F1] MCP deferred schema is a friction barrier** (sonnet-run2, item 1)
The `8v` MCP tool is not pre-loaded in the evaluation environment. Deferred schema loading requires an explicit ToolSearch call before any `8v` command can be issued. As a result, the model defaulted to native Read/Write tools rather than calling the MCP tool. "Agents default to native tools" when the schema is not immediately available.

**[F2] Batch read instruction not followed under native-tool execution** (sonnet-run2, item 2; sonnet-run3, item 2)
When the model executed via native tools rather than the 8v MCP, it issued sequential per-file reads rather than batching. The batch principle is documented but not enforced by the execution environment. The instruction exists; the tooling gap prevents compliance.

**[F3] No `--mkdir-p` equivalent for write** (sonnet-run2, item 3)
`8v write` has no documented option to auto-create parent directories if they don't exist. The model noted this as a missing capability.

**[F4] No file-existence check command** (sonnet-run3, item 1)
The model fell back to Bash `ls` to verify a directory existed because 8v has no documented existence-check command. "8v has no file-existence check command."

**[F5] No friction on write** (sonnet-run3, item 3)
Single write call to the target path worked cleanly with no retry. Positive signal: the write command syntax is clear enough to execute correctly on first attempt.

Opus runs reported no 8v tooling friction (all three runs; no 8v feedback section filed). This is consistent with the evaluation design: Opus runs used the MCP tool description as the document under test but executed the session through native tools.
