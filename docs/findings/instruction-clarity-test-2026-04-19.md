# Instruction Clarity Test v2 — 2026-04-19

**Test design:** 2 models × 3 runs each = 6 total. Same 24-question protocol as v1 (2026-04-18).
Instruction surfaces: `o8v/CLAUDE.md` (injected block, 39 lines) and `o8v/src/mcp/instructions.txt`
(32 lines). Runs labeled S1–S3 (Sonnet) and O1–O3 (Opus). v2 applied nine P0+P1 edits from v1.

---

## 1. Summary

Mean clarity score: **6.17 / 10** (range 6–7). v1 mean was 6.5/10 — **regression of −0.33**.
No run exceeded 7. The nine P0+P1 edits landed partially: `--delete` fix and 1-indexed annotation
are accepted by all six runs; the "most Bash" replacement opened a new ambiguity ("8v doesn't
cover" = open-ended residual). The dominant new finding is a missing output/error contract:
all six runs independently asked for exit codes, `--json` schema, and error destination. This gap
did not appear as a primary Q24 edit in v1; it is the top P0 candidate for v3.

---

## 2. Clarity Scores

| Run | Model  | Q23 | Q24 One-Minute Edit (verbatim) |
|-----|--------|-----|-------------------------------|
| S1  | Sonnet | 6   | "Add Symbol map output format block." |
| S2  | Sonnet | 7   | "Add Failure modes section with explicit error format/exit codes." |
| S3  | Sonnet | 6   | "Add Output formats section with one concrete example per command." |
| O1  | Opus   | 6   | "Add 6-line Contract block: paths missing → exit 2, invalid range → exit 3, --json returns {data, errors[]}, batch read returns one record per input in argument order, write content verbatim, search recursive by default." |
| O2  | Opus   | 6   | "Add one concrete end-to-end example: ls → batched read → write → test → check with actual output." |
| O3  | Opus   | 6   | "Replace Verify one-liner with 4 lines (one per command) stating what each does and what it returns, plus 4-5 line symbol map sample." |

v1 scores: S1=6, S2=6, S3=7, O1=6, O2=7, O3=7. v1 mean 6.5. v2 mean 6.17.

---

## 3. P0+P1 Edit Landing Status

Nine edits were applied between v1 and v2. Status assessed per run.

| Edit | Description | v1 gap | v2 result | Landed? |
|------|-------------|--------|-----------|---------|
| P0-1 | `--delete` two-line split (CLAUDE.md:28) | Flagged 1/6 | Q12h conf=5 ALL 6; not re-flagged in Q9 | YES |
| P0-2 | 1-indexed, end-inclusive annotation (CLAUDE.md:21) | Flagged 6/6 | Not raised as primary gap in any run | YES |
| P1-1 | Symbol map example `36  pub struct Args` (CLAUDE.md:20) | Flagged 6/6 | Acknowledged; agents ask language coverage, private symbols | PARTIAL |
| P1-2 | "most Bash" → explicit exclusion list (CLAUDE.md:5) | Flagged 6/6 | "8v doesn't cover" clause creates new open-ended residual — S1, O1, O2, O3 flagged | PARTIAL |
| P1-3 | Escape table `\n`/`\t`/`\\` (CLAUDE.md:32) | Flagged 6/6 | Accepted; S3+O2 flag shell-vs-8v interpretation ambiguity | PARTIAL |
| P1-4 | Regex annotation on search (CLAUDE.md:16) | Flagged 5/6 | Not re-raised as primary gap | YES |
| P1-5 | `--stack` enum added (CLAUDE.md:15) | Flagged 5/6 | Surface 1 fixed; Surface 2 (instructions.txt:11) still missing enum | PARTIAL |
| CS-3 | "Last resort" annotation on `--full` | Added to both surfaces | Understood correctly by all runs | YES |
| CS-4 | "Use after the symbol map" for range | Added to both surfaces | Understood correctly by all runs | YES |

**Fully landed: 4/9.** Partially landed: 5/9. None regressed to "made worse" except P1-2 side effect.

---

## 4. New Gaps Opened by v2 Edits

### 4a. "8v doesn't cover" — open-ended residual (4/6 runs)

**Source:** CLAUDE.md line 5 — "Use Bash only for git, process management, and env operations 8v
doesn't cover."

The explicit list (git, process management, env operations) is good. The trailing qualifier "8v
doesn't cover" creates a second open-ended bucket: any operation not covered by the named
categories also qualifies for Bash. Agents cannot enumerate this set.

S1 Q6 verbatim: "The named categories are helpful, but 'doesn't cover' is a trailing catch-all
that expands the Bash set to anything I believe 8v lacks. I cannot confirm completeness."

O2 Q6: flagged cross-doc drift — project-level `CLAUDE.md` said "most Bash" while injected block
says "and Bash for file operations." Both surfaces now resolved but the residual qualifier survives.

**Fix:** Drop "8v doesn't cover." Replace with: "Use Bash only for git and process management."
The env-operations category should be folded into process management or named specifically.

### 4b. `--stack` enum absent from Surface 2 (6/6 runs)

**Source:** CLAUDE.md line 15 lists 16 valid values. instructions.txt line 11 shows `--stack <name>` with no enum.

All six runs flagged this in Q9 (cross-surface inconsistency). S2 and O1 specifically noted that
an agent reading only the MCP description cannot derive valid stack names.

O1 Q9 verbatim: "Surface 2 shows `--stack <name>` but gives no list. An agent working only via
MCP would have to guess rust, go, python — and would miss terraform, helm, kustomize."

---

## 5. Consensus Gaps — Present in Both v1 and v2 (6/6 Runs)

These gaps were in v1 and survive v2 unchanged.

### 5a. Output/error contract — undefined (6/6) **[NEW priority — absent from v1 P0 list]**

No run in v1 had this as its Q24 edit. All six v2 runs named it.

The gap: no instruction surface specifies exit codes, where errors go (stderr vs stdout vs --json),
or the `--json` schema structure. Agents cannot write reliable error-handling code.

O1 Q22 verbatim: "Add Output & Errors section: stdout shape, --json schema keys, exit-code
meaning, behavior on missing path/invalid range."

O1 Q24 verbatim: "Add 6-line Contract block: paths missing → exit 2, invalid range → exit 3,
--json returns {data, errors[]}, batch read returns one record per input in argument order,
write content verbatim, search recursive by default."

S2 Q24 verbatim: "Add Failure modes section with explicit error format/exit codes."

### 5b. `check`/`fmt`/`test`/`build` — scope undefined (6/6)

**Source:** CLAUDE.md line 34 — "`8v check .`  `8v fmt .`  `8v test .`  `8v build .`"

All four commands appear as a one-liner with no description of what each does or what it returns.
Q12l confidence scores: S1=2, S2=2, S3=2, O1=3, O2=3, O3=2.

O3 Q13: classified these four commands as "example-only, no description" — the most precise
taxonomy of this gap across all runs.

O3 Q24: "Replace Verify one-liner with 4 lines (one per command) stating what each does and
what it returns."

### 5c. `--find/--replace` — literal vs regex undefined (6/6)

**Source:** CLAUDE.md line 30 — "`8v write <path> --find \"<old>\" --replace \"<new>\"` — fails if `<old>` not found."

O2 Q22: explicitly flagged the cross-doc drift — project CLAUDE.md (oss/8v/CLAUDE.md) has no
indication while injected block is silent too. "Is `<old>` a regex? A literal? Can I use `.` to
mean any character?"

### 5d. Symbol map — language coverage, private symbols (4/6)

The `36  pub struct Args` example (new in v2) clarified Rust. Remaining questions:
what file types produce a symbol map? What about private symbols (`fn private_helper`)? What
about macros, trait impls, enums?

S1 Q7: "The Rust example is clear. I do not know what `8v read` returns for a Go file or a
Python class."

S3 Q12e: confidence 4 (not 5) — "bare path + range mix not explicitly shown; I assume it works."

---

## 6. Low-Confidence Commands (Q12)

Scenarios where ≥ 4 of 6 runs gave confidence ≤ 3.

| Scenario | Description | S1,S2,S3,O1,O2,O3 | Source |
|----------|-------------|-------------------|--------|
| 12g | `8v write` when target file does not exist | 1,1,2,1,1,2 | CLAUDE.md:26–31 |
| 12m | cross-file rename via write | 1,2,1,1,1,2 | CLAUDE.md:26–31 |
| 12b | multi-line range write with escaped content | 3,3,3,2,2,3 | CLAUDE.md:27,32 |
| 12l | confirm `8v check .` runs lint+types+fmt | 2,2,2,3,3,2 | CLAUDE.md:34 |
| 12f | `--json` output schema for `8v read` | 2,2,3,3,3,3 | CLAUDE.md:11 |

v1 floor scenarios were 12g and 12m — unchanged in v2. 12b confidence improved slightly (escape
table partially addressed). 12l and 12f were in v1 but scored identically.

---

## 7. Model Differences

### Sonnet (S1–S3) vs v1

v1 Sonnet mean Q23: 6.33. v2 Sonnet mean Q23: 6.33 — unchanged.
S2 raised from 6→7 (new error contract framing resonated); S3 dropped from 7→6 (new escape
ambiguity introduced by the `\n` table).

S3 introduced the only new escape ambiguity in v2: does the shell interpret `\n` before 8v, or
does 8v interpret it? This is a side-effect of the escape table being added without a clarifying
sentence about who does the interpretation.

### Opus (O1–O3) vs v1

v1 Opus mean Q23: 6.67. v2 Opus mean Q23: 6.0 — **regression of −0.67**.
All three Opus runs dropped from 7→6. Opus is more sensitive to the output contract gap than
Sonnet — O1, O2, O3 all led with contract/error-format as Q22/Q24, not symbol map.

O3 Q14 (three failure modes agents would hit): quoting bugs in write content; assuming check
covers fmt+types; falling back to Bash because error output is unhelpful. This is the clearest
causal chain any run produced linking instruction gap → agent mistake.

---

## 8. Teaching-Hesitation Hotspots (Q20)

| Hotspot | v1 named by | v2 named by | Status |
|---------|-------------|-------------|--------|
| "most Bash" boundary | S1,S2,S3,O1,O2,O3 | S1,O1,O2,O3 (residual) | Improved but not closed |
| Symbol map output | S1,S2,S3,O1,O2,O3 | S1,S2,S3,O1,O3 | Improved, not closed |
| Multi-line write escaping | S1,O1,O2,O3 | S3,O2 | Improved |
| `--delete` contradiction | O3 only | None | CLOSED |
| `check`/`fmt`/`test`/`build` scope | O1,O2,O3 | S1,S2,S3,O1,O2,O3 | Worsened (now universal) |
| Output/error contract | Not in v1 top-5 | S1,S2,S3,O1,O2,O3 | NEW — universal |

O2 Q20 verbatim: "I would hesitate most on the output contract. If someone asked me 'what does
8v return on error?', I cannot answer from instructions."

S3 Q20 verbatim: "I can teach the write syntax now. I cannot teach the escape sequence
interpretation — is `\n` a shell escape or an 8v escape?"

---

## 9. Wished Features (Q19)

Aggregated from all six runs, de-duplicated with v1 baseline.

| Feature | v1? | v2 runs |
|---------|-----|---------|
| `8v write --file <path>` — content from file | YES | S1,O1,O2,O3 |
| `8v rename <old> <new>` — atomic rename | YES | S2,S3,O1,O3 |
| `8v write --dry-run` — preview before apply | YES | O1,O3 |
| Error output contract (`--json` schema, exit codes) | Partial (v1 Q19 #7) | ALL 6 |
| `8v check` breakdown — which sub-check failed | NO | O1,O2,O3 |
| `8v read --symbols-only` filtering by type | NO | S2,O2 |
| `8v test --filter <name>` — run single test | NO | S3,O3 |

The output contract wish (error schema, exit codes) escalated from partial mention in v1 to
universal in v2 — the most significant shift in the Q19 distribution.

---

## 10. v1→v2 Regression Analysis

| Metric | v1 | v2 | Delta |
|--------|----|----|-------|
| Mean Q23 | 6.50 | 6.17 | −0.33 |
| Sonnet mean | 6.33 | 6.33 | 0.00 |
| Opus mean | 6.67 | 6.00 | −0.67 |
| Edits fully landed | — | 4/9 | — |
| New gaps opened | 0 | 2 (residual Bash, --stack CS gap) | +2 |
| Universal Q24 theme | symbol map (4/6) | output contract (6/6) | shifted |
| 12g confidence floor | 1,1,2,1,1,2 | 1,1,2,1,1,2 | unchanged |
| 12h confidence (`--delete`) | 1,2,1,2,2,2 | 5,5,5,5,5,5 | FIXED |
| Teaching-hesitation count | 6 hotspots | 6 hotspots (2 new, 1 closed) | net 0 |

**Root cause of regression:** The "most Bash" → explicit list edit introduced a residual qualifier
that agents treat as a new open-ended set. Simultaneously, all six runs surfaced the output
contract gap as the dominant problem — a gap not addressed by any v2 edit and more salient to
Opus than Sonnet.

**Expected floor was 8/10.** Actual floor is 6/10. The hypothesis that fixing `--delete` and
1-indexed annotation would raise scores was falsified: those fixes landed cleanly but did not
move scores because agents were not blocked by those gaps — they were blocked by the output
contract.

---

## 11. Prioritized Edit List for v3

### P0 — Output contract is missing; agents cannot handle errors correctly

| # | Edit | Source |
|---|------|--------|
| P0-1 | Add Contract section: exit codes (missing path → 2, invalid range → 3), --json schema `{data, errors[]}`, write verbatim (no auto-newline), search recursive by default. | Both surfaces |
| P0-2 | Drop "8v doesn't cover" trailing qualifier. Replace with: "Use Bash only for git and process management." | CLAUDE.md:5, instructions.txt:1 |

### P1 — Instruction is incomplete; agent must guess

| # | Edit | Source |
|---|------|--------|
| P1-1 | Add `--stack` enum to instructions.txt (Surface 2). Copy from CLAUDE.md:15. | instructions.txt:11 |
| P1-2 | Expand Verify one-liner to 4 annotated lines: what each command does, what it checks, what it returns. | CLAUDE.md:34 / instructions.txt:29 |
| P1-3 | Add escape interpretation note: "Content escape sequences are interpreted by 8v, not the shell. Pass them as literal backslash-n." | CLAUDE.md:32 / instructions.txt:27 |
| P1-4 | Add `--find/--replace` literal declaration: "matching is literal string, not regex." | CLAUDE.md:30 / instructions.txt:25 |

### P2 — Correct but incomplete on edge cases

| # | Edit | Source |
|---|------|--------|
| P2-1 | Clarify symbol map scope: "Returns symbols for code files; key/value pairs for config files; empty for plain text." | CLAUDE.md:19–20 |
| P2-2 | State `8v write` behavior on nonexistent file (creates or errors). | CLAUDE.md:26–31 |
| P2-3 | State `[path]` default for search: "defaults to workspace root when omitted." | CLAUDE.md:16 / instructions.txt:12 |

---

## 8v Feedback

Friction encountered while producing this document using 8v tools:

1. **Batch read of six .output files (task output paths)** — Absolute paths outside any project
   root. The tool accepted them without issue. No friction on discovery or read.

2. **No `8v write --file`** — The findings doc is ~500 lines. Writing it required a native Write
   tool call. The Q19 wish (`--file <path>` to pass content from a file) is the same gap that
   appeared in both v1 and v2 Q19. It is now a confirmed dogfood friction point, not just an
   agent survey result.

3. **`8v read` on output files produced a symbol map attempt** — `.output` files contain prose,
   not code. The symbol map returned nothing useful; `--full` was needed. For non-code file types,
   a graceful "no symbols detected; use --full to read as text" message would reduce the read
   round-trip from 2 calls to 1.

4. **No cross-surface diff command** — Verifying that CLAUDE.md and instructions.txt stay in sync
   requires reading both and diffing manually. A `8v diff-surfaces` or `8v check --surfaces`
   command would have caught the `--stack` enum CS gap automatically rather than requiring agent
   survey to surface it.
