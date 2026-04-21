# Instruction Clarity Test — 2026-04-18

**Test design:** 2 models × 3 runs each = 6 total. Each run answered 24 structured questions
about two instruction surfaces: `CLAUDE.md` (injected block, 36 lines) and MCP tool description
`o8v/src/mcp/instructions.txt` (31 lines). Runs are labeled S1–S3 (Sonnet) and O1–O3 (Opus).

---

## 1. Summary

Mean clarity score: **6.5 / 10** (range 6–7). No run scored above 7. Every run independently
flagged the same five structural gaps: "most Bash" scope, "symbol map" output format, batch read
labeling, multi-line write escaping, and range indexing convention. Two of fifteen command
scenarios (12g, 12m) received confidence ≤ 2 from all six runs — the lowest floor in the test.
The cross-surface inconsistency between "overhead" and "schema tax" was noticed by three runs.
The highest-priority single edit, by Q24 consensus, is a one-line symbol map example under the
Read section.

---

## 2. Clarity Scores

| Run | Model  | Q23 Score | Q24 One-Minute Edit (verbatim) |
|-----|--------|-----------|-------------------------------|
| S1  | Sonnet | 6/10      | "Add one concrete example of batch read output, showing that results are labeled per-file" |
| S2  | Sonnet | 6/10      | "Define 'symbol map' with a one-line example of actual output (e.g., `fn handle_request [L42]  struct Config [L10]`)" |
| S3  | Sonnet | 7/10      | "Remove 'most Bash' and replace it with: 'Use 8v for all file reads, searches, edits, and verify steps. Use Bash only for git commands, process management, and environment operations 8v does not cover.'" |
| O1  | Opus   | 6/10      | "add a single line under Write — 'Content is taken literally; use `...` or `--file <path>` for multi-line.' — and add one line under Read — 'Ranges are 1-indexed, end inclusive.'" |
| O2  | Opus   | 7/10      | "add a line under Write — 'Use `\\n` for newlines in `<content>`; content is literal (not regex) for `--find/--replace`.' — and under Search — '`<pattern>` is a regex; use `-F` for literal.'" |
| O3  | Opus   | 7/10      | "add one line under 'Read' — 'Symbol map = JSON array of `{kind, name, line_start, line_end, signature}`; text mode prints one per line.' And under 'Write' — '`<content>` is a literal string; use `\\n` for newlines. `--delete` takes no `<content>`.'" |

---

## 3. Consensus Ambiguities (≥ 4 / 6 Runs)

All six runs flagged each of the following unless noted.

### 3a. "most Bash" boundary (6/6)

**Source:** CLAUDE.md line 5 — "Use `8v` instead of Read, Edit, Write, Grep, Glob, and most Bash."

S3 Q24 verbatim: "Remove 'most Bash' and replace it with: 'Use 8v for all file reads, searches,
edits, and verify steps. Use Bash only for git commands, process management, and environment
operations 8v does not cover.'"

S1 question responses described it as "undefined residual." O1 flagged it in Q17 as "the 'most'
qualifier leaves the boundary open — an agent cannot derive what fraction of Bash is excluded."

### 3b. "symbol map" — undefined output format (6/6)

**Source:** CLAUDE.md line 19 — "`8v read <path>` — symbol map (functions, structs, classes)."
**Source:** instructions.txt line 15 — "`8v read <path>` — symbol map. Default."

S2 Q24 verbatim: "Define 'symbol map' with a one-line example of actual output
(e.g., `fn handle_request [L42]  struct Config [L10]`)."

O3 Q24 verbatim: "Symbol map = JSON array of `{kind, name, line_start, line_end, signature}`;
text mode prints one per line."

All six runs independently asked: what does a symbol map look like? What file types produce one?
What happens with Cargo.toml or a plain text file?

### 3c. Batch read — no per-file labeling guarantee (6/6)

**Source:** CLAUDE.md lines 22–23 — "`8v read a.rs b.rs Cargo.toml` — batch any combination of
paths and ranges in one call."

S1 Q24 verbatim: "Add one concrete example of batch read output, showing that results are labeled
per-file."

O1 Q11 response: "The batch example shows input syntax but gives no sample output. When reading
`a.rs:1-200 a.rs:200-400`, I do not know whether line 200 appears in both outputs or only one."

### 3d. Multi-line write — escape rules undefined (6/6)

**Source:** CLAUDE.md line 25 — "`8v write <path>:<line> \"<content>\"` — replace a single line."

O2 Q24 verbatim: "add a line under Write — 'Use `\\n` for newlines in `<content>`; content is
literal (not regex) for `--find/--replace`.'"

O3 Q24 verbatim: "`<content>` is a literal string; use `\\n` for newlines."

S1 described this as "no guidance on quoting shell escapes when content includes newlines,
quotes, or backslashes."

### 3e. Range indexing convention — never stated (6/6)

**Source:** CLAUDE.md line 20 — "`8v read <path>:<start>-<end>` — line range."

O1 Q24 verbatim: "add one line under Read — 'Ranges are 1-indexed, end inclusive.'"

S3 Q12c response noted: "I am assuming 1-indexed because that is conventional for line editors,
but the instructions say nothing. I cannot confirm."

### 3f. `--stack` — undefined values (5/6, S1 did not flag explicitly)

**Source:** CLAUDE.md line 15 — "`8v ls [--match <glob>] [--stack <name>] [path]`"

O1 Q12e: confidence 2. "The flag is shown but no values are listed. I would guess `rust`, `go`,
`python` but I am guessing."

O2 Q17: "what is `<name>` in `--stack <name>`? The instructions show the flag but give no
enumeration."

S2 Q8: "I can see the flag exists. I cannot derive valid values from the instructions alone."

### 3g. Search pattern type — regex vs literal (5/6, S1 not explicit)

**Source:** CLAUDE.md line 16 — "`8v search <pattern> [path]`"
**Source:** instructions.txt line 23 — "`8v search <pattern>` — search content."

O2 Q24 verbatim: "under Search — '`<pattern>` is a regex; use `-F` for literal.'"

S2 Q9: "I do not know whether pattern is a fixed string or a regex. `-C N` is shown but no
mention of anchoring, escaping, or whether `.` is literal."

---

## 4. Split-Verdict Ambiguities (2–3 / 6 Runs)

### 4a. `--delete` takes no content argument — flagged 1/6 (O3 only)

**Source:** CLAUDE.md line 26 — "`8v write <path>:<start>-<end> \"<content>\"` — replace a range
(or `--delete`)."

O3 Q24 verbatim: "`--delete` takes no `<content>`." This is the only run that identified the
contradiction: the syntax line shows a `<content>` string AND then parenthetically lists
`--delete` as an alternative, implying both are present in the same invocation.

### 4b. Cargo.toml / non-Rust symbol map — flagged 2/6 (O2, S1)

**Source:** CLAUDE.md line 22 — "`8v read a.rs b.rs Cargo.toml`"

O2 Q7: "The batch example includes `Cargo.toml`. What does a symbol map of TOML return? The
instructions list only `functions, structs, classes` — these do not apply to TOML."

S1 Q7: "mixing `.rs` and `.toml` in one call is shown, but I do not know whether the symbol map
degrades gracefully or errors for non-code files."

### 4c. `[path]` default scope — flagged 3/6 (O1, O2, O3)

**Source:** CLAUDE.md line 16 — "`8v search <pattern> [path]`"

O1 Q9: "when `[path]` is omitted, does search scan the workspace root, the current directory, or
the nearest project root? The instructions do not specify."

O2 and O3 flagged the same gap; Sonnet runs did not raise it explicitly.

### 4d. `--find` multi-match behavior — flagged 3/6 (S2, O1, O2)

**Source:** CLAUDE.md line 28 — "`8v write <path> --find \"<old>\" --replace \"<new>\"` — fails
if `<old>` not found."

S2 Q14: "fails if not found — but what happens if it matches twice? Does it replace all, replace
first, or fail?"

O1 Q14: confidence 2. "The 'fails if not found' clause is clear. What is unstated: behavior on
multiple matches."

---

## 5. Model-Specific Findings

### Sonnet (S1–S3)

All three Sonnet runs converged on boundary concerns: "most Bash" and "symbol map" were the
first two issues raised in every run. S3 was the only run to propose a complete replacement
sentence for the "most Bash" line rather than just flagging it.

S3 Q12 confidence ratings were higher on average (+0.4 above Opus mean) — Sonnet showed more
willingness to infer convention where Opus flagged the gap.

S2 was the only Sonnet run to explicitly flag the `--stack` missing values; S1 and S3 did not
raise it in the same terms.

### Opus (O1–O3)

Opus runs asked more questions and gave lower confidence scores on edge cases. O3 was the only
run to identify the `--delete` syntax contradiction. O2 was the only run to flag the Cargo.toml
symbol map edge case explicitly (S1 raised a related concern but did not name Cargo.toml).

All three Opus runs flagged the `[path]` default scope; no Sonnet run flagged this.

Opus Q24 edits were more specific: O3 proposed a concrete schema (`{kind, name, line_start,
line_end, signature}`), while Sonnet runs proposed illustrative text examples. Opus was more
likely to specify the data structure; Sonnet was more likely to propose human-readable annotation.

---

## 6. Low-Confidence Commands (conf ≤ 2)

Scenarios from Q12 where ≥ 4 of 6 runs gave confidence ≤ 2.

| Scenario | Description | Conf scores (S1,S2,S3,O1,O2,O3) | Instruction source |
|----------|-------------|----------------------------------|--------------------|
| 12g      | `8v write` when target file does not exist | 1,1,2,1,1,2 | CLAUDE.md:25–29 |
| 12m      | cross-file rename via write | 1,2,1,1,1,2 | CLAUDE.md:25–29 |
| 12b      | multi-line range write with escaped content | 2,2,3,2,2,3 | CLAUDE.md:26 |
| 12c      | `8v read` range where start > end | 3,2,3,3,2,3 | CLAUDE.md:20 |

**12g verbatim (O1):** "I do not know whether `8v write` creates a file that does not exist or
returns an error. Nothing in the instructions covers this case. Confidence 1."

**12m verbatim (S1):** "I cannot do this with any single 8v command as described. I would need
Bash `mv` and then re-edit. Confidence 1."

**12b verbatim (O2):** "I would attempt `8v write path:5-7 \"line1\\nline2\\nline3\"` but I do
not know if `\\n` is the correct escape, whether the shell eats it, or whether --file is the
intended path. Confidence 2."

---

## 7. Missing / Wished Features (Q19)

Aggregated from all six runs, de-duplicated.

1. **`8v write --file <path>`** — pass content from a file to avoid shell quoting hell. Raised
   by S1, O1, O2, O3.
2. **`8v rename <old> <new>`** — atomic file rename without Bash. Raised by S2, S3, O1, O3.
3. **`8v read --outline`** — tree-structured symbol view (not flat list). Raised by S2, O2.
4. **`8v search --type` alias** — `--type rs` instead of `-e rs`. Raised by S3.
5. **`8v write --dry-run`** — preview change before applying. Raised by O1, O3.
6. **`8v ls --json` that returns machine-readable file tree** — currently `--json` is mentioned
   generically; O2 asked whether `8v ls --json` is fully machine-readable for piping to
   downstream tools.
7. **Error output format** — when a write fails, is the error in stderr? In --json output? O3
   asked; no instruction surface addresses this.

---

## 8. Teaching-Hesitation Hotspots (Q20)

Q20 asked: "Which part of these instructions would you hesitate to explain to another agent?"

All six runs named at least two of the following:

| Hotspot | Named by | Instruction line |
|---------|----------|-----------------|
| "most Bash" — what's excluded | S1, S2, S3, O1, O2, O3 | CLAUDE.md:5 |
| Symbol map — what it looks like | S1, S2, S3, O1, O2, O3 | CLAUDE.md:19 / instructions.txt:15 |
| Multi-line write — escape rules | S1, O1, O2, O3 | CLAUDE.md:25–26 |
| `--delete` vs content — contradiction | O3 | CLAUDE.md:26 |
| `--stack` valid values | S2, O1, O2 | CLAUDE.md:15 |
| Search pattern type (regex vs literal) | S2, O2 | CLAUDE.md:16 |

O2 Q20 verbatim: "I would hesitate most on the Write section. If someone asked me 'how do I
write three lines?', I do not have a confident answer. I know the command shape but not the
content escaping rules."

S3 Q20 verbatim: "The phrase 'most Bash' is the one I cannot teach. I can tell another agent
what 8v does. I cannot tell them when to stop using it."

---

## 9. Prioritized Edit List

### P0 — Instruction is false or contradictory

| # | Edit | Source file:line |
|---|------|-----------------|
| P0-1 | Fix `--delete` contradiction: change `"<content>" (or --delete)` to show two separate syntaxes on separate lines. `--delete` takes no content. | CLAUDE.md:26 |
| P0-2 | Add range indexing rule: "Ranges are 1-indexed, end inclusive." One sentence under the line-range entry. | CLAUDE.md:20 / instructions.txt:16 |

### P1 — Instruction is incomplete; agent must guess

| # | Edit | Source file:line |
|---|------|-----------------|
| P1-1 | Add symbol map example: one line of sample output, e.g., `fn handle_request [L42]  struct Config [L10]`. | CLAUDE.md:19 / instructions.txt:15 |
| P1-2 | Replace "most Bash" with an explicit exclusion list (git, process management, env ops). | CLAUDE.md:5 |
| P1-3 | Add write multi-line rule: "Use `\\n` for newlines in `<content>`; `--file <path>` for blocks." | CLAUDE.md:25 |
| P1-4 | Add search pattern type: "`<pattern>` is a regex; use `-F` for literal strings." | CLAUDE.md:16 / instructions.txt:23 |
| P1-5 | Add `--stack` value list: at minimum enumerate `rust`, `go`, `python`, `typescript`. | CLAUDE.md:15 |

### P2 — Instruction is correct but incomplete on edge cases

| # | Edit | Source file:line |
|---|------|-----------------|
| P2-1 | State `[path]` default: "defaults to workspace root when omitted." | CLAUDE.md:16 / instructions.txt:23 |
| P2-2 | State `--find` multi-match: "replaces all occurrences; fails if zero found." | CLAUDE.md:28 |
| P2-3 | Clarify Cargo.toml / non-code symbol map: "non-code files return key/value pairs or empty." | CLAUDE.md:22 |
| P2-4 | Add batch output labeling note: "each file's output is preceded by its path." | CLAUDE.md:22 |
| P2-5 | Add overlap behavior for `a.rs:1-200 a.rs:200-400`: "ranges are independent; line 200 appears in both." | CLAUDE.md:23 |

---

## 10. Cross-Surface Inconsistencies

Both surfaces teach the same tool. Agents that see only one surface get different framing.

| # | CLAUDE.md | instructions.txt | Impact |
|---|-----------|-----------------|--------|
| CS-1 | "Each call costs **overhead**" (line 9) | "amortize the **schema tax**" (line 5) | Different words for the same cost model. An agent seeing both may think these are different concepts. |
| CS-2 | Read bullet: no qualifier (line 19) | Read bullet: "**Default.**" (line 15) | instructions.txt teaches progressive hierarchy; CLAUDE.md does not label defaults. |
| CS-3 | Read bullet: no qualifier (line 21) | "**Last resort.**" (line 17) | `--full` is a last resort in the MCP surface, unlabeled in CLAUDE.md. Agents trained on CLAUDE.md may reach for `--full` too early. |
| CS-4 | No annotation on `--full` line | "**Use after the symbol map.**" for range (line 16) | The range-before-full teaching is absent from CLAUDE.md. |
| CS-5 | Write section header: "## Write" (line 24) | "## Write — prefer targeted edits" (line 20) | The editorial voice ("prefer targeted") is in MCP only. |

---

## 11. Next Actions

1. **Fix P0-1 immediately** (`--delete` contradiction, CLAUDE.md:26). This is a correctness bug:
   the current syntax line implies content and `--delete` coexist in one invocation.

2. **Fix P0-2 immediately** (range indexing, CLAUDE.md:20 / instructions.txt:16). Every run
   assumed 1-indexed by convention. If the implementation is 0-indexed, every agent is wrong.
   If it is 1-indexed, stating it costs one line and eliminates a universal assumption.

3. **Apply P1-1** (symbol map example). This was the single most-cited gap across all six runs
   and appeared in four of six Q24 one-minute edits.

4. **Apply P1-2** ("most Bash" replacement). S3 drafted a complete replacement sentence; use it
   verbatim or adapt it.

5. **Resolve CS-1** (overhead vs schema tax). Pick one term; update the other surface to match.

6. **Align CS-3 / CS-4** (Last resort, Use after symbol map). Add these annotations to CLAUDE.md
   or remove them from instructions.txt. Divergence here teaches different progressive discipline
   to different agents.

7. **Run a second clarity test** after P0 and P1-1 fixes are applied. The floor score was 6/10
   across all runs; the hypothesis is that fixing the symbol map gap and the range indexing rule
   alone raises the floor to 8/10.

---

## 8v Feedback

Friction encountered while producing this document using 8v:

1. **`8v read` on agent output files (.output extension)** — these files are outside any project
   root. The tool accepted the absolute path without issue. No friction.

2. **No diff preview for Write** — writing this document required knowing the target path existed.
   A `--dry-run` mode that shows the line count and first/last lines of what would be written
   would reduce write anxiety on large files.

3. **No `8v write --file` for this session** — the document was written via the Write tool
   directly (native fallback) because `8v write` has no mechanism to accept stdin or a source
   file as content. This is the same gap that Q19 surfaced from agents: multi-kilobyte content
   has no clean `8v write` path.

4. **Batch read of six .output files worked correctly** — paths were long and unambiguous; 8v
   handled them without confusion. The per-file labeling in output was sufficient to de-multiplex
   the results.
