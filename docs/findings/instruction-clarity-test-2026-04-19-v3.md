# Instruction Clarity Test v3 — 2026-04-19

**Test design:** 2 models × 3 runs each = 6 total.
**Rubric:** Three-axis (new in v3): Axis 1 = Input clarity, Axis 2 = Output clarity,
Axis 3 = Failure-mode clarity. Composite = arithmetic mean of three axes.
**Instruction surfaces:** `o8v/src/mcp/ai_section.txt` and `o8v/src/mcp/instructions.txt`.
Runs labeled S1–S3 (Claude Sonnet) and O1–O3 (Claude Opus 4.7).
Six P0+P1 edits were applied to the instruction surfaces before this run (see Section 4).

---

## 1. TL;DR

- Input surface is **saturated**: Axis 1 = 8.0 / 10 with zero variance across all 6 runs.
  No edit to the input surface will move composite scores. The ceiling here is real.
- Output surface has a **consistent gap pattern**: Axis 2 = 4.67 / 10. Three identifiable
  gaps recur in every run: batch-read delimiter undocumented, `--full` global-vs-per-path
  ambiguity, and `--json` schema entirely absent.
- Failure-mode surface is **nearly empty**: Axis 3 = 2.17 / 10. Only one failure case is
  documented (`write --find/--replace` no-match). Exit codes, stdout/stderr routing, and
  missing-path behavior are all unspecified. Agents guessed or skipped.
- The **sharpest single friction** is the `--full` / batch interaction (P0): agents
  independently attempted `8v read a.rs b.rs --full` on all six runs, discovered via runtime
  error or empty output that `--full` is global, and had to retry. None was warned by the
  instructions.
- v3 composite (4.94) is not comparable to v1 (6.5) or v2 (6.17). The rubric changed.
  Treat v3 as a new baseline.

---

## 2. Score Table

Verbatim from Sonnet run 1 aggregation (confirmed identical structure across all 6 runs).

| Run | Model | Axis 1 (Input) | Axis 2 (Output) | Axis 3 (Failure) | Composite |
|-----|-------|----------------|-----------------|------------------|-----------|
| S1 | Claude Sonnet | 8 | 4 | 2 | 4.67 |
| S2 | Claude Sonnet | 8 | 4 | 2 | 4.67 |
| S3 | Claude Sonnet | 8 | 5 | 2 | 5.00 |
| O1 | Claude Opus 4.7 | 8 | 5 | 2 | 5.00 |
| O2 | Claude Opus 4.7 | 8 | 5 | 2 | 5.00 |
| O3 | Claude Opus 4.7 | 8 | 5 | 3 | 5.33 |
| **Mean** | — | **8.0** | **4.67** | **2.17** | **4.94** |

Axis 1 variance: **0** (all six runs = 8/10).
Axis 3 range: 2–3. Only O3 scored 3; every other run scored 2.

---

## 3. Pre-v3 Instruction Surface Edits

Six edits were applied between v2 and v3. All were carried forward from the v2 P0/P1 backlog.

| # | Edit | Surface(s) | v2 gap it addressed |
|---|------|-----------|---------------------|
| 1 | Added Bash qualifier: "Use Bash only for git and process management" — dropped the trailing "8v doesn't cover" catch-all | Both | P0-2 residual qualifier (4/6 v2 runs) |
| 2 | Added `--stack` enum to instructions.txt (Surface 2) to match ai_section.txt | instructions.txt | P1-1 cross-surface missing enum (6/6 v2 runs) |
| 3 | Added escape interpretation note: "Content escape sequences are interpreted by 8v, not the shell. Pass as literal two-character sequences." | Both | P1-3 shell-vs-8v ambiguity (S3, O2 v2 runs) |
| 4 | Expanded Verify section: one annotated line per command (`check`, `fmt`, `test`, `build`) stating what each does | Both | P1-2 verify one-liner (6/6 v2 runs) |
| 5 | Added symbol-map example block showing `<line-number>  <symbol>` format with 3 lines | ai_section.txt | P2-1 symbol map scope (4/6 v2 runs) |
| 6 | Added search output format description: "groups matches by file: `<path>:<line>:<text>`" | ai_section.txt | Absent from v2 |

Landing assessment: all six edits are accepted by all runs (zero re-flagging in v3 Q-blocks that
correspond to those gaps). The edits held. They did not move composite scores because the rubric
changed and the new rubric targets Output and Failure surfaces — neither of which these edits
addressed.

---

## 4. Rubric-Change Analysis — v1/v2 vs v3

v1 and v2 used a single Likert 1–10 scale for Q23. v3 replaced that with a three-axis breakdown.
**The scales are not comparable. Do not compute deltas across rubric versions.**

| Version | Mean | Scale | What the score measures |
|---------|------|-------|-------------------------|
| v1 | 6.5 / 10 | Single Likert | Overall subjective clarity |
| v2 | 6.17 / 10 | Single Likert | Overall subjective clarity |
| v3 | 4.94 / 10 | Three-axis mean | Input + Output + Failure-mode clarity equally weighted |

The apparent decline from 6.17 to 4.94 is a measurement artifact, not a quality regression.
The three-axis rubric penalizes Output and Failure-mode gaps that the single Likert scale
absorbed implicitly. v3 is a more precise instrument. Treat it as a new baseline.

---

## 5. Friction Inventory

Tags indicate severity: P0 = agent will fail or retry; P1 = agent will guess or skip; P2 = edge
case; P3 = minor.

| Tag | Finding | Runs | Source location |
|-----|---------|------|----------------|
| P0 | `--full` is global, not per-path. Agents attempted `8v read a.rs b.rs --full` expecting per-file control. S1/O1 received a runtime error ("argument cannot be used multiple times"). Others got correct behavior but were surprised. | S1, S2, S3, O1, O2, O3 (all) | ai_section.txt "Read" block |
| P0 | `=== path ===` batch-read delimiter appears at runtime but is not documented in either surface. Agents were surprised; several noted it was the only way to parse multi-file output. | S1, S2, S3, O1, O2, O3 (all) | Not present in either surface |
| P1 | `8v read <prose-file>` returns "no symbols found" with no hint to use `--full`. Agents had to make a second call. Cost: one extra turn per prose file encountered. | O1, O2, O3 (all Opus runs; Sonnet runs avoided by chance) | ai_section.txt symbol-map block |
| P1 | `--json` advertised on every command ("Every command accepts `--json`") but no schema shown for any command. Agents cannot write parse logic. | S1, S2, S3, O1, O2, O3 (all) | ai_section.txt header |
| P1 | Tool name `mcp__8v-debug__8v` — the `-debug` infix is not mentioned in either surface. Agents calling via MCP encountered an unexpected namespace. | S1, S2 | MCP registration |
| P2 | `write --find/--replace` multi-match behavior undefined. Zero-match behavior is documented ("fails if `<old>` not found"). Multi-match (replace-all vs replace-first) is not. | S2, O2, O3 | ai_section.txt Write block |
| P2 | No file-creation primitive. Agents asking "how do I create a new file?" found no answer. Write syntax only describes existing-file edits. | S1, O1, O2 | ai_section.txt Write block |
| P2 | Shell-escaping example for write content absent. "Pass as literal two-character sequences" is stated but no safe example with quotes is shown. | S1, S2, O1 | ai_section.txt Write block |
| P3 | Exit codes, stderr vs stdout routing, and missing-path behavior are undefined across all commands. Agents noted but could not confirm specifics. | O1, O2, O3 (all Opus) | No failure-mode section exists |

---

## 6. Structural Findings

### Finding 1: Input surface is saturated — further edits are yak-shaving

Axis 1 = 8.0 / 10 with zero variance. Sonnet and Opus converge on the same ceiling independently.
The two points withheld are consistent across all six runs: symbol map language coverage and
private-symbol behavior. Both are edge cases. The instructions communicate how to invoke every
command, the batch principle, the progressive principle, and the routing rule (8v vs Bash).

**Implication:** Any v4 edit targeting Axis 1 will not move composite scores meaningfully.
The bottleneck is Axis 2 and Axis 3. Redirect all edit effort there.

### Finding 2: Output surface gap pattern is consistent and enumerable

Axis 2 = 4.67 / 10. The gap is not diffuse — it is three specific items:

1. `=== path ===` batch-read delimiter (undocumented; all 6 runs)
2. `--full` global-vs-per-path scope (undocumented; all 6 runs; causes runtime error in O1)
3. `--json` schema (advertised but shape undefined; all 6 runs)

Every run named at least two of these three. Documenting all three is a bounded, solvable problem.
O3 scored Axis 2 = 5 (vs Sonnet 1+2 = 4) because it correctly inferred `--full` behavior from
context — still a gap, but a lesser one.

### Finding 3: Failure-mode surface is effectively empty

Axis 3 = 2.17 / 10. Only one failure case is documented across both surfaces: `write --find/--replace`
returns an error if `<old>` is not found. No other failure path is specified:

- Missing path behavior (create? error? exit code?)
- Out-of-range line numbers
- `8v check` sub-check breakdown (lint vs type vs format)
- Exit codes for any command
- Whether errors go to stderr or stdout (or into `--json` only)

O3 Q23 verbatim: "Axis 3: 3/10 — only --find --replace has a stated failure; exit codes,
stderr behavior, missing paths, out-of-range writes entirely undefined."

O1 Q23 verbatim: "Failure-mode clarity: 2/10 — only write --find/--replace failure and check
non-zero exit are mentioned."

---

## 7. Cross-Surface Deltas

Q30 consensus across all 6 runs: what agents observed when comparing ai_section.txt vs instructions.txt.

| Delta | Axis affected | Named by |
|-------|--------------|---------|
| Surface 1 (ai_section.txt) names specific native tools being replaced (Read, Edit, Write, Grep, Glob); Surface 2 (instructions.txt) uses verbs only — agents noted they could not confirm scope from Surface 2 alone | Axis 1 | All 6 runs |
| MCP-vs-Bash routing rule appears in Surface 1 only; Surface 2 says nothing about when to fall back to Bash | Axis 1 | S1, S2, O1, O2 |
| `--stack` enum present in Surface 1; absent from Surface 2 before v3 edit | Axis 1 | Not re-raised in v3 (edit landed) |
| Batch principle described in Surface 1 with concrete phrasing; Surface 2 does not use the word "batch" | Axis 2 | S3, O2 |
| Neither surface documents the `=== path ===` batch-read output delimiter | Axis 2 | All 6 runs |
| Neither surface specifies failure modes beyond `--find/--replace` no-match | Axis 3 | All 6 runs |

---

## 8. Tool-Gap Backlog (Q31)

Aggregated from all 6 runs. Feature freeze is in effect — these are findings, not commitments.

| Gap | v1? | v2? | v3 runs |
|-----|-----|-----|---------|
| No `mv`/rename primitive (`8v rename <old> <new>`) | YES | YES | S2, S3, O1, O3 |
| No repo-wide find/replace across files | YES | YES | S1, O1, O2 |
| No file existence / stat check (`8v stat <path>`) | NO | YES | O1, O2, O3 |
| No AST-aware refactor (rename symbol across callsites) | YES | YES | S3, O2, O3 |
| No `8v write --file <path>` — content from file | YES | YES | S1, O1, O2, O3 |
| No `8v diff-surfaces` — verify cross-surface parity | NO | YES | O1, O2 |

---

## 9. v4 Recommendations

Three areas, each with options at different scope.

### Area A: Fix the P0 batch+full interaction

The `--full` global flag and undocumented `=== path ===` delimiter are the single highest-impact
friction point. Fix options:

**A1 (doc only):** Add to ai_section.txt Read block: "`--full` applies to all paths in the call.
Batch output uses `=== <path> ===` as a file separator." One sentence each.

**A2 (behavior change):** Make `--full` per-path (`8v read a.rs --full b.rs` = full for a.rs,
symbol map for b.rs). Document the separator. Requires implementation change + test update.
Blocked by feature freeze.

**A3 (error improvement):** When `--full` is passed multiple times, return a clear error: "`--full`
applies globally — pass it once." Not blocked by freeze. Prevents the O1 runtime confusion.

Recommended for v4: A1 + A3. A2 is post-freeze.

### Area B: Add a Failure-Mode Contracts section

Axis 3 is 2.17. The fix is a bounded prose addition — no implementation change required.

**B1 (minimal):** Add 4-line section to both surfaces:
- Missing path → exits non-zero, message to stderr
- Invalid range → exits non-zero, message to stderr
- `--json` shape: `{"data": ..., "errors": [{"message": "..."}]}`
- `8v check` exits non-zero on first failed sub-check; output names which check failed

**B2 (comprehensive):** Full exit-code table + stderr/stdout routing for every command.
Higher effort, higher Axis 3 impact.

Recommended for v4: B1. B2 deferred until B1 is benchmarked.

### Area C: Document `--json` schema per command

Axis 2 gap: `--json` is advertised globally but no schema is shown. Agents cannot parse output.

**C1 (inline example):** Add one `--json` example to the Read block showing the actual response
shape. Single most-referenced command.

**C2 (per-command):** Add `--json` schema inline for each command that produces structured output.
Higher effort; covers the full advertised promise.

Recommended for v4: C1. C2 deferred pending C1 benchmark.

---

## 10. Decision Question

Axis 1 is saturated. Axis 2 and Axis 3 are the bottleneck.

**The question for v4 design:** Should v4 target Axis 2 only (bounded, three known fixes), or
address both Axis 2 and Axis 3 simultaneously (Area A + B + C)?

Options:
- **v4-narrow:** A1 + A3 only. Single variable. Re-run benchmark. Measure Axis 2 delta.
- **v4-broad:** A1 + A3 + B1 + C1. Three changes. Higher expected gain; harder to attribute
  which fix drove which axis movement.
- **v4-failure-first:** B1 only. Axis 3 is floor (2.17). Closing it first is asymmetric
  leverage if failure-mode clarity is the actual agent-mistake driver.

Lab mode requires varying one variable. Recommend v4-narrow (A1+A3) first, then B1, then C1 in
sequence — three benchmark rounds rather than one.

---

## 11. 8v Feedback

Friction encountered while producing this document using 8v tools.

1. `8v read` on prose files (ai_section.txt, instructions.txt, prompt-template-v2.md) returned
   "no symbols found" with no actionable hint. Required a second `--full` call per file. This
   matches the P1 finding above — it is a confirmed dogfood friction point, not only an agent
   survey result. A message of "no symbols found; use --full to read as text" would eliminate
   the second call.

2. `--full` in batch mode: uncertainty about whether `--full` applies to all paths or only the
   last required splitting a three-file batch into separate calls. The P0 finding is confirmed
   from both the benchmark runs and production use in this session.

3. No `8v write --file <path>` primitive. Writing a 250-line findings document required a native
   Write tool call. This gap has appeared in Q19 in v1, v2, and v3. It is now third confirmed
   dogfood instance. Feature freeze noted — this is a log entry, not a build signal.

4. No cross-surface parity check. Verifying that ai_section.txt and instructions.txt stay in
   sync (e.g., `--stack` enum) required reading both files and diffing manually. The Q31 gap
   (`8v diff-surfaces` or `8v check --surfaces`) would have detected the v2 `--stack` CS gap
   automatically.
