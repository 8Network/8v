# Instruction Clarity v4 — Opus Run 3 — 2026-04-20
Model: Claude Opus 4.7

## A. Understanding

### Q1: summary
8v is a code-reliability tool for AI agents that replaces native Read/Edit/Write/Grep/Glob/Bash for file operations (use shell only for git, process, env). Use it whenever you read, edit, search, or inspect files. Two principles: **Progressive** (minimum-useful default output, flags escalate) and **Batch** (pass many inputs in one call to amortize overhead).

### Q2: minimum invocations
- `ls`: `8v ls` — Instructions don't state minimum default output; the recommended first call is `8v ls --tree --loc` (hierarchy with line counts).
- `read`: `8v read <path>` — returns a symbol map (line-number + symbol).
- `search`: `8v search <pattern>` — groups matches by file as `<path>:<line>:<text>`.
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line (no default success output stated).
- `check`: `8v check .` — lint + type-check + format-check; non-zero exit on issue.
- `fmt`: `8v fmt .` — auto-formats in place, idempotent.
- `test`: `8v test .` — runs project tests.
- `build`: `8v build .` — compiles.

### Q3: principles
1. **Progressive** — commands default to the smallest useful payload and escalate with flags. Non-obvious example: `read` returns *symbols only*, not lines, so you pay symbols-token-cost and then spend a targeted range read instead of `--full`.
2. **Batch** — pass multiple inputs per call. Non-obvious example: you can mix distinct files and multiple ranges of the same file in one `read` call (`a.rs b.rs:1-50 b.rs:200-300`) — the batch isn't just "list of files".

### Q4: when 8v vs native
Use 8v for anything that reads, edits, searches, or inspects files. Use Bash for git, process management, and environment operations. If an `8v` MCP tool is available, call it directly rather than shelling out.

### Q5: flag discovery
Run `8v <cmd> --help` for the full flag list. Every command also accepts `--json`.

## B. Ambiguity

### Q6: ambiguous phrases
- "minimum useful answer" — reading 1: smallest syntactic output; reading 2: smallest semantically-sufficient output for the common task. Not defined.
- "Last resort" (for `--full`) — reading 1: technically allowed; reading 2: considered an error/anti-pattern worth avoiding.
- "Use Bash only for git, process management, and environment operations" — reading 1: *exclusively* those three; reading 2: those and other operations 8v doesn't cover.
- "`fails if <old> not found`" — reading 1: fails only on zero matches; reading 2: also fails on >1 matches (ambiguity). Not resolved.
- "All verify commands … run on the whole project by default. Pass a path to scope to a subtree." — reading 1: `.` is redundant; reading 2: `.` is required. Examples show `.` always.
- "Line numbers point at the definition; use them with `:start-end`" — reading 1: the symbol-map line number equals `start`; reading 2: the agent is supposed to figure out the end. End is never defined.

### Q7: implied but unstated
- Error format (human vs JSON) on failure.
- Exit codes for every command except `check` (success=0 implied, nonzero on issue).
- Where errors print (stdout vs stderr).
- Behavior on path that doesn't exist.
- Whether writes create parent directories.
- Whether `write` auto-appends a newline.
- Whether batch `read` interleaves or sequences.
- Unicode handling in content / regex engine used for `search`.
- Whether `--json` replaces or supplements text on failure.
- Whether `search` respects `.gitignore` (not stated in ai_section.txt; main CLAUDE.md says so).
- Concurrency / atomicity of writes.
- Default `--limit` value for `search`.

### Q8: undefined terms
"symbol map" (only shown by example), "stack" (values enumerated, concept undefined), "compact mode" (never mentioned in the two surfaces under test), "subtree", "the whole project" (what defines project boundary?), "overhead" (cost model implied), "minimum useful answer".

### Q9: contradictions
No direct contradictions. Surface 1 (ai_section.txt) has a few extra sentences Surface 2 lacks (e.g., `search` default-output explanation is present in Surface 1; Surface 2 version also has it). The two surfaces are near-duplicates — largest difference is prose framing ("code reliability tool for AI agents. Designed to minimize round-trips" sentence in Surface 2; Surface 1 omits that one-liner).

### Q10: batch output shape
From the text: "each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges. Single-file reads emit no header." So: concatenated, one symbol map (or range body) per file, separated by `=== <label> ===` markers, in positional order. Interleaving isn't mentioned — the contract implies serial sections.

### Q11: write quoting and newlines
"Content arguments are parsed by 8v (not the shell): `\n` becomes a newline, `\t` a tab, `\\` a literal backslash. Pass them as literal two-character sequences — do not rely on shell interpolation." Multi-line content is written by embedding `\n`. Whether a trailing newline is appended automatically is **not stated**. Surrounding quotes are standard shell quoting to keep the string as one argument; they are not special to 8v.

## C. Concrete commands

### Q12: scenarios
a. Read 5 files — `8v read a.rs b.rs c.rs d.rs e.rs` — 5 — batch rule explicit.
b. Replace lines 10–20 of foo.rs with 3-line content — `8v write foo.rs:10-20 "line1\nline2\nline3"` — 4 — syntax explicit, trailing-newline behavior unspecified.
c. Find `handle_*` functions — `8v search "fn handle_\w+|def handle_\w+|function handle_\w+"` — 3 — regex supported ("(regex)"), but language-agnostic pattern is my guess; Instructions don't say — my guess: regex engine is stdlib.
d. Append to notes.md — `8v write notes.md --append "new line"` — 5 — explicit.
e. Symbol map + read range — `8v read bar.rs bar.rs:100-150` — 4 — batch supports mixing a path and a range; confidence not 5 because ordering/dedup isn't stated.
f. Tests with JSON — `8v test . --json` — 5 — every command accepts `--json`.
g. Check file exists — Instructions don't say — my guess: fall back to Bash `test -e` or run `8v read` and inspect exit code. 2.
h. Delete lines 50–60 — `8v write foo.rs:50-60 --delete` — 5 — explicit.
i. Insert before line 30 — `8v write foo.rs:30 --insert "new line"` — 5 — explicit.
j. Case-insensitive Rust TODO — `8v search "TODO" -i -e rs` — 5 — flags explicit.
k. Files by name `*_test*.md` — `8v ls --match "*_test*.md"` — 3 — `--match` is a glob filter on `ls`; it may or may not include `.md` outside known stacks. Instructions don't say — my guess: it matches file names.
l. lint+format+type — `8v check .` — 5 — explicit.
m. Multi-file refactor `old_name`→`new_name` — Instructions don't say — my guess: loop `8v write <file> --find "old_name" --replace "new_name"` per file from a `search --files` list. 2 — no native multi-file find/replace documented.
n. Symbols of 10 files — `8v read f1 f2 f3 f4 f5 f6 f7 f8 f9 f10` — 5 — batch.
o. Two ranges of big.rs — `8v read big.rs:1-200 big.rs:500-600` — 5 — explicit example.

### Q13: taught by
- `ls`: example (tree+loc, filtered flags).
- `read`: example (rich, with output samples).
- `search`: example (flags listed, default format shown).
- `write`: example (every subform has a line).
- `check`: description-only.
- `fmt`: description-only.
- `test`: description-only.
- `build`: description-only.

## D. Behavioral prediction

### Q14: 3 likely mistakes
1. Forgetting to double-escape `\n` inside shell single vs double quotes, producing literal `\n` or broken lines, because the "parsed by 8v, not shell" rule is subtle.
2. Guessing at error/exit-code semantics for `write --find/--replace` when `<old>` appears multiple times (ambiguous) and corrupting a file.
3. Using `--full` too eagerly on large files because "symbol map first" discipline isn't enforced and "Last resort" is weak wording.

### Q15: fall back to native
- Checking file existence (not documented).
- Binary-file inspection.
- File creation from scratch — Instructions don't say — my guess: maybe `write --append` to a nonexistent path, but behavior is unspecified.
- Renames/moves/deletes of whole files.
- Directory creation.
- Git operations.
- `find -exec` style bulk operations beyond `--match`.

### Q16: most/least
- Most: `read` (symbol map + ranges) and `search` — natural early-phase tools.
- Least: `build` — most projects exercise `test`/`check` instead, and `check` already covers lint+type+format.

### Q17: first command
`8v ls --tree --loc` — the instructions say "Start here" and it gives hierarchy + line counts, which drives the next `read` targets.

### Q18: trust write errors?
Confidence 2. The instructions state one failure case explicitly (`--find` with no match). Everything else — out-of-range line numbers, bad paths, permission errors, parent dir missing — is silent. The error text format is not specified, so I would not trust the error to tell me the next step.

## E. Missing / wished

### Q19: missing
- Exit-code table per command.
- Error JSON schema.
- Path-not-found behavior.
- `write` newline semantics.
- `search` regex dialect and `.gitignore` behavior.
- A `mv`/`rm`/`mkdir` story (or explicit "use Bash for this").
- Glossary of "symbol map", "stack", "project".
- A multi-file refactor example.

### Q20: teach hesitations
I'd hesitate on: (1) exact `write` newline/quoting mechanics across shells; (2) how batch output is consumed when one file errors; (3) what "project" means when scoping `8v check <path>`; (4) when to choose `--full` over multi-range.

### Q21: vs native
Better: fewer round-trips (symbol map + batch), JSON on every command, a single verify command (`check`), first-class stack awareness.
Worse: no file-existence test, no file create/delete/rename verbs, regex engine undocumented, error semantics opaque, no streaming for long outputs.

### Q22: one biggest edit
Add a concrete error/exit-code contract table (one row per command): success output, failure output, exit code, JSON error shape, and the three most likely failure modes. That single addition would close most Q7/Q18/Q26–Q29 gaps at once.

## F. Overall

### Q23: clarity axes
- Axis 1 — Input clarity: 8 — syntax for every command is shown with examples, flags are enumerated, batch and range syntax are explicit.
- Axis 2 — Output clarity: 6 — `read` batch contract is stated and `search` default format is stated, but `write`/`check`/`fmt`/`test`/`build` success output is entirely unspecified.
- Axis 3 — Failure-mode clarity: 3 — only one failure case is documented (`write --find` no match). Exit codes, stderr vs stdout, JSON error shape, path-not-found, multi-match — all silent.
- Composite mean: (8 + 6 + 3) / 3 = **5.67**.

### Q24: one-minute edit
Add a 6-line error contract: "All commands exit 0 on success, nonzero on failure. Human errors go to stderr. With `--json`, failures emit `{\"error\":{\"code\":\"...\",\"message\":\"...\"}}` on stdout with nonzero exit. `read` on missing path → `PathNotFound`. `write --find` with zero or multiple matches → `FindAmbiguous`. `check` nonzero = issue found."

## G. Output contracts

### Q25: predicted outputs for util.py
a. `8v read util.py` — symbol map. Exact text (best guess from the example format):
```
1  def add
```
Instructions don't say — my guess: module-level assignments like `result = add(1, 2)` are not symbols (symbols shown are fn/struct/impl only). Confidence 3.

b. `8v read util.py:1-2` — lines 1–2 of the file, 1-indexed, end inclusive. Exact text:
```
# util.py  (4 lines)
def add(a, b):
```
Confidence 4. The contract: "Single-file reads emit no header" — so no `=== ===` banner.

c. `8v search "add" util.py` — default format `<path>:<line>:<text>`. Exact text (my prediction):
```
util.py:2:def add(a, b):
util.py:5:result = add(1, 2)
```
Confidence 3. Instructions don't say whether pattern is regex by default (`(regex)` label in syntax suggests yes); `add` matches literally either way. Line numbering assumes the comment on line 1.

d. `8v read util.py --full` — entire file content. Exact text:
```
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```
Instructions don't say — my guess: `--full` on a single file emits no `=== ===` header (matches "Single-file reads emit no header"). Confidence 3.

### Q26: check exit/stream
Instructions state: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue." Gap: success exit code not stated (assume 0). Gap: whether error text is on stdout or stderr is not stated. Gap: whether `--json` alone contains the issues or whether stdout/stderr also populate is not stated.

### Q27: find/replace zero or many
Instructions state: "`8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found." Gap: behavior when `<old>` appears more than once is not specified. Gap: return/error shape is not specified. Instructions don't say — my guess: all occurrences replaced on >1, nonzero exit on zero-match; but this is genuinely unclear.

### Q28: read nonexistent path
Gap: Instructions don't say. No text addresses path-not-found. Instructions don't say — my guess: nonzero exit, error on stderr in human mode, `{"error":{...}}` on stdout with `--json`. But none of this is in the two surfaces under test.

## H. Contract reasoning

### Q29: silent exit-1 from check
Based only on the instructions: it should not be expected — the only documented `check` outcome for failure is "Non-zero exit on any issue," which implies *some* diagnostic is emitted. Silence on both streams with exit 1 is an unspecified state. Agent's next step: re-run with `--json` (every command accepts it) to get structured output. Gap: the instructions do not explicitly require `check` to print diagnostics, so this behavior is undefined rather than wrong.

## I. API coherence

### Q30: surface diffs
- Surface 2 opens with a one-sentence product framing: "8v — code reliability tool for AI agents. Designed to minimize round-trips." Surface 1 lacks this. Matters for: an agent seeing only Surface 1 loses the "round-trip minimization" framing.
- Surface 1 says "Use Bash only for git, process management, and environment operations." Surface 2 says "Use shell tools only for git, process management, and environment operations." Near-identical; "shell tools" is slightly broader.
- Surface 1 uses bullet lists under each section; Surface 2 uses bare lines. No semantic difference.
- Surface 2 has "Discovery — learn the repo in one call" subtitle; Surface 1 just says "Discovery". Trivial.
- Surface 2's Write section is titled "Write — prefer targeted edits"; Surface 1 just "Write". Surface 2 is marginally more instructive.
- Both surfaces have identical commands, flags, examples, batch contract, verify list, and "Typical flow" line. No factual divergence in command semantics.
Overall: the two surfaces are near-duplicates with minor prose differences; neither strictly dominates, but Surface 2 is slightly tighter framing. An agent that saw only one would not meaningfully misbehave.

## J. Tool-gap surfacing

### Q31: realistic uncovered task
Task: rename a Rust module file `src/foo.rs` to `src/foo_legacy.rs` and update every import. Missing capability: file rename/move. Closest substitute: Bash `git mv` (or `mv`) followed by `8v search "use crate::foo"` + `8v write --find/--replace` per match. Cost: extra shell round-trips and a multi-match `--find/--replace` whose semantics on >1 match are unspecified, so the refactor risks corruption without manual per-file confirmation.

## K. Behavioral dry-run

### Q32: 5-step task
1. `8v ls --match "*.go"` — 3 — `--match` is glob; Instructions don't say — my guess: it returns file paths. Alternative `8v ls --stack go` — 4, since `go` is a valid stack value, but this lists projects, not files. I'd pick `--match`.
2. `8v search "http.Get" -e go -C 2` — 4 — `-e` filters by extension, `-C 2` is 2 lines of context; explicit in the instructions.
3. `8v read <file>` (symbol map) — 4 — but choosing "the file with the most matches" requires parsing step-2 output, which isn't explicitly machine-readable; I'd use `--json` on step 2 to rank. Confidence on the *read* itself: 5.
4. `8v write <file>:<N> --find "http.Get" --replace "httpClient.Get"` — 2 — you cannot scope `--find/--replace` to a specific line per the docs; it operates on the file. To replace on one specific line I'd do `8v read <file>:<N>-<N>`, then `8v write <file>:<N> "<new line text>"`. Would likely need a fallback Bash/sed if I need a surgical line-level literal replacement preserving surrounding text — though line-level replace via `write <path>:<line> "<content>"` is sufficient if I know the new line.
5. `8v test .` — 5 — explicit.

## L. Memorability

### Q33: recalled write syntax
- Insert new line before line 42 of main.rs: `8v write main.rs:42 --insert "new line"` — confidence 5.
- Replace lines 10–20 with multi-line content: `8v write main.rs:10-20 "line1\nline2\nline3"` — confidence 4 (newline escape recalled; auto-trailing-newline still unspecified).

---

## 8v feedback
Friction encountered while answering:
- No error/exit-code contract anywhere. Five of the 33 questions (Q18, Q26, Q27, Q28, Q29) are blocked by the same gap.
- "Symbol map" is only defined by example (fn/struct/impl). An agent can't predict what Python/TS/Go will return — Q25a had to guess whether module-level assignments appear.
- `write --find/--replace` on multiple matches is the single most dangerous undefined behavior. It should be explicit.
- `ls --match` vs `ls --stack` semantics are unclear: `--stack` sounds like "project stack," `--match` sounds like "filename glob," but whether `--match` walks into non-stack folders is unstated.
- Two surfaces are 95% duplicate. The duplication is fine but the *single* place failure-mode contracts should live is missing from both.

**Model & run ID:** Claude Opus 4.7 — Run 3 — 2026-04-20.
