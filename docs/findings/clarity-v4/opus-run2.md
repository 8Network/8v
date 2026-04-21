# Instruction Clarity v4 — Opus Run 2 — 2026-04-20
Model: Claude Opus 4.7

## A. Understanding

### Q1: summary
8v is a CLI (with MCP-tool equivalent) for AI agents to do file operations — read, write, search, list, and verify — in place of native Read/Edit/Write/Grep/Glob/Bash. Use it for anything that reads, edits, searches, or inspects files; use Bash only for git, process management, and environment operations. Two principles: **Progressive** (default = minimum useful answer; flags escalate detail) and **Batch** (pass many inputs in one call to amortize per-call overhead).

### Q2: minimum-viable invocations
- `ls` → `8v ls --tree --loc` — returns the full file hierarchy with line counts. (Minimal form `8v ls` is not explicitly shown; instructions don't say — my guess: `8v ls` alone also works and lists the current dir.)
- `read` → `8v read <path>` — returns a symbol map (lines of `<line-number>  <symbol>`).
- `search` → `8v search <pattern>` — returns matches grouped by file as `<path>:<line>:<text>`.
- `write` → `8v write <path>:<line> "<content>"` — replaces a single line (no return value documented).
- `check` → `8v check .` — runs lint + type-check + format-check; non-zero exit on any issue.
- `fmt` → `8v fmt .` — auto-formats files in place; idempotent.
- `test` → `8v test .` — runs project tests.
- `build` → `8v build .` — compiles.

### Q3: the two principles
1. **Progressive** — every command starts terse and escalates only when you ask for more. Non-obvious example: `8v read foo.rs` returns only a symbol map (no source), *not* the whole file — so you never pay for bytes you didn't ask for. You must add `:start-end` or `--full` to see actual code.
2. **Batch** — pass N inputs in one call instead of N calls. Non-obvious example: `8v read a.rs:1-200 a.rs:200-400 b.rs` mixes ranges of the *same* file with a *different* file in a single invocation — the batching axis isn't "one file per call", it's "one call per agent turn".

### Q4: when to use 8v vs native
Use 8v for: reading, editing, writing, searching, inspecting, listing files, plus build/test/check/fmt. Do NOT use it for: git operations, process management, environment operations — those stay in Bash. If the 8v MCP tool is available, call it directly rather than shelling out via Bash.

### Q5: flag discovery
Run `8v <cmd> --help` for the full flag list of any command. Every command also accepts `--json`.

## B. Ambiguity

### Q6: multi-reading phrases
- "Use Bash only for git, process management, and environment operations" — dual reading: (a) these are the *only* Bash-permitted categories; (b) these are *examples* of allowed categories (what about network/curl, package installation, Docker?). Treated as exhaustive but not stated as such.
- "One call beats N sequential calls" — (a) always prefer batching; (b) beats them on what axis — latency, tokens, cost? Unstated.
- "Last resort" (for `--full`) — (a) literally never unless nothing else works; (b) avoid by default but ok when you need the file.
- "end inclusive" for `<start>-<end>` — clear, but paired with "1-indexed" — does `1-1` return exactly line 1 (one line) or zero lines? Inclusive says one line; worth double-checking.
- "Non-zero exit on any issue" — (a) exit code 1 specifically; (b) any non-zero, which could vary by issue class.
- "Idempotent" for `fmt` — (a) running twice yields the same bytes; (b) also exit-code idempotent? Unclear.
- "All verify commands accept `--json` and run on the whole project by default. Pass a path to scope to a subtree." — does `8v check` (no arg) work, or is `.` required? Text is ambiguous.
- "fails if `<old>` not found" — (a) non-zero exit; (b) throws structured error; (c) silent no-op returning a message. Unspecified.
- "`--full` applies to every positional arg (repeats accepted, no-op beyond the first)" — "repeats" meaning repeated `--full` flags, or repeated paths?

### Q7: implicit-but-unstated behavior
- Exit codes (only `check` mentions non-zero; others unspecified).
- stdout vs stderr split for errors.
- Error message format (plain text? structured? JSON when `--json`?).
- What happens when a path does not exist for `read`/`write`/`search`.
- Whether `write` creates a file that doesn't exist (e.g., `--append` on a non-existent file).
- Trailing newline behavior on `write`.
- Multi-line content semantics and shell-escape rules (only `\n`/`\t`/`\\` are mentioned).
- Unicode handling in content.
- Binary file handling.
- Output interleaving order in batch reads (file order? parallel?).
- JSON schema beyond the two examples shown.
- Whether `8v search` follows symlinks, respects .gitignore, hidden files.
- `ls` output format (not shown at all).
- Concurrency — two writes to same file.
- Default `--limit` for `search` if omitted.
- Regex dialect for `search` (the text says `(regex)` — PCRE? RE2? Rust regex?).
- `-C N` — is N max or exact context?
- What "symbol" means for each language (fn, struct, class, def, const, type alias, macro…).
- Whether `8v check` auto-detects stack or requires config.
- Format of the `=== <label> ===` delimiter in `--json` mode (is it still present, or replaced entirely by `{"Multi":...}` — one surface says replaced, the other repeats the phrase "uses the same `===` delimiter" for `--full`).

### Q8: undefined terms
- "symbol map" — defined by example only; unclear for non-Rust languages.
- "stack" — used but never defined; the list of values is given but not what "stack" means (toolchain? language? build system?).
- "progressive" — used as a principle name; the definition is given but "minimum useful answer" is itself vague.
- "overhead" — per-call overhead is mentioned without explaining the cost model (tokens? latency? MCP round-trips?).
- "subtree" — used in verify section; inferred as "subdirectory".
- "the whole project" — assumes a project boundary is detected (how?).

### Q9: contradictions
- Surface 1 adds a line about `search` default output (`<path>:<line>:<text>`) and `--files`/`-C N` explanation — Surface 2 has the same. Actually both include it. **No contradiction** on that line.
- Surface 1 vs Surface 2: Surface 1 opens with "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash…" while Surface 2 opens "8v — code reliability tool for AI agents. Designed to minimize round-trips. Use `8v` for all file operations…". Different framings (replacement vs. optimization), not a contradiction.
- Surface 2 headings use "Discovery — learn the repo in one call" and "Write — prefer targeted edits"; Surface 1 uses bare "Discovery" and "Write". Cosmetic difference, not contradictory.
- Surface 1 uses bullet markers (`- `) for most command lines; Surface 2 does not. Presentation, not content.
- No hard factual contradictions spotted.

### Q10: batch read output interleaving
The text says: "each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges. Single-file reads emit no header." So batch output is **concatenated, not interleaved**, with a `=== <label> ===` separator per entry. For `--json` batches it becomes `{"Multi":{"entries":[{"label":...,"result":...}, ...]}}`. The text does not state entry order (input order vs. some canonical order) — **gap**.

### Q11: content-writing details
- Trailing newline: **not stated**. Instructions don't say — my guess: no implicit trailing newline; you provide `\n` if you want one.
- Multi-line content: use the literal two-character sequences `\n`, `\t`, `\\` in the content string; 8v (not the shell) parses them.
- Surrounding quotes: those are shell quoting, not 8v syntax. 8v sees the parsed argument. So the example `"<content>"` shows shell double-quotes; single-quotes would also work as long as your shell delivers the literal `\n`/`\t` characters to 8v. The instructions explicitly warn: "do not rely on shell interpolation."

## C. Concrete commands + confidence

### Q12: scenarios
a. Read 5 files at once
   - `8v read a.rs b.rs c.rs d.rs e.rs`
   - Confidence: 5
   - Reasoning: batch read is explicitly documented with this exact pattern.

b. Replace lines 10–20 of `foo.rs` with 3 lines
   - `8v write foo.rs:10-20 "line1\nline2\nline3"`
   - Confidence: 4
   - Reasoning: range-replace syntax is documented, `\n` is 8v-parsed; unclear whether trailing newline is added.

c. Find all functions named `handle_*`
   - `8v search "fn handle_\w+"`
   - Confidence: 3
   - Reasoning: regex dialect not specified; Rust-style `fn` prefix assumed. Instructions don't say — my guess: Rust `regex` crate syntax.

d. Append one line to `notes.md`
   - `8v write notes.md --append "a new line\n"`
   - Confidence: 3
   - Reasoning: `--append` documented; trailing newline behavior unclear, so I include `\n` defensively.

e. Symbol map of `bar.rs`, then lines 100–150
   - `8v read bar.rs bar.rs:100-150`
   - Confidence: 5
   - Reasoning: the "mix of path and range for same file in one call" is explicitly documented.

f. Run tests and parse JSON
   - `8v test . --json`
   - Confidence: 4
   - Reasoning: `--json` is documented universally; exact shape of test JSON not specified.

g. Check whether a file exists before reading
   - Instructions don't say — my guess: no dedicated `exists` command. Closest: `8v ls --match <name> <dir>` or just attempt `8v read <path>` and handle failure. I would fall back to `test -f <path>` in Bash or just try `8v read`.
   - Confidence: 2
   - Reasoning: no existence predicate documented.

h. Delete lines 50–60
   - `8v write foo.rs:50-60 --delete`
   - Confidence: 5
   - Reasoning: exact syntax is documented.

i. Insert a new line before line 30
   - `8v write foo.rs:30 --insert "new line"`
   - Confidence: 5
   - Reasoning: exact syntax is documented.

j. Search Rust files, case-insensitive, for `TODO`
   - `8v search "TODO" -i -e rs`
   - Confidence: 5
   - Reasoning: `-i` and `-e <ext>` documented on the search line.

k. Find files by name matching `*_test*.md`
   - `8v ls --match "*_test*.md"`
   - Confidence: 4
   - Reasoning: `--match <glob>` is documented on `ls`. (Could also be `8v search --files` but docs show `--match` on `ls` — cleaner.)

l. Lint + format-check + type-check in one command
   - `8v check .`
   - Confidence: 5
   - Reasoning: exactly what `check` is documented to do.

m. Replace `old_name` with `new_name` across a multi-file refactor
   - Instructions don't say — my guess: `8v write` `--find`/`--replace` is single-file only. No multi-file refactor flag is shown. I would do `8v search "old_name" --files` to enumerate, then loop with `8v write <file> --find "old_name" --replace "new_name"` per file (via shell, or multiple MCP calls).
   - Confidence: 2
   - Reasoning: no documented batch-replace across files.

n. Symbol maps of 10 files in one call
   - `8v read f1.rs f2.rs f3.rs f4.rs f5.rs f6.rs f7.rs f8.rs f9.rs f10.rs`
   - Confidence: 5
   - Reasoning: batch read default is symbol map per file.

o. Lines 1–200 and 500–600 of `big.rs` in one call
   - `8v read big.rs:1-200 big.rs:500-600`
   - Confidence: 5
   - Reasoning: multiple ranges of same file in one call is explicitly given as an example.

### Q13: teach mode per command
- `ls` — **example** (`8v ls --tree --loc`, plus the filtered form).
- `read` — **example** (symbol-map sample output, range, full, batch, JSON shape).
- `search` — **example** (the output format line is described, plus flags).
- `write` — **example** (each subcommand has a pattern line).
- `check` — **description-only** (no sample output).
- `fmt` — **description-only**.
- `test` — **description-only**.
- `build` — **description-only**.

## D. Behavioral prediction

### Q14: three most likely mistakes
1. Assuming `8v read <path>` returns file content (it doesn't — it returns a symbol map), then being confused when I can't find the line I want to edit until I add `:start-end`.
2. Forgetting that `\n` is 8v-parsed, not shell-parsed — I might pass a real newline via shell heredoc or use `printf`, producing garbled multi-line writes.
3. Assuming `--find`/`--replace` replaces *all* occurrences across the repo, when it's single-file and fails if `<old>` not found — I'd miss refactor targets.

### Q15: fallback triggers
- Need to check file existence without reading.
- Need to read files outside the repo tree (unclear if 8v supports that).
- Need binary-file operations.
- Regex search fails silently or returns unexpected dialect behavior.
- Multi-file atomic refactor (find/replace across files).
- Need to manipulate files from outside a detected "project" (e.g., no stack).
- git operations (explicitly Bash territory).
- Any time the 8v output contract is unclear and I need to verify.

### Q16: most/least used
- Most: `8v read` (symbol-map-first cheap), `8v search`, `8v write :line`, `8v check .`. These are the inner loop for most code work.
- Least: `8v build .` (slower, rarely needed per-turn), `8v fmt .` (auto-run by check or CI usually), `8v ls --stack` (niche).

### Q17: first command in a new repo
`8v ls --tree --loc` — it's literally the recommended "Start here" and gives me full hierarchy with line counts, which directly informs which files to symbol-map first.

### Q18: trust the write error?
Partially. The instructions say `--find`/`--replace` "fails if `<old>` not found" but do not state the error format, exit code, or stream. Without knowing whether an error goes to stderr or is JSON-shaped, I'd trust *that* the operation failed but not the guidance on *what to do next*. I'd re-check with `8v read` or `8v search`.

## E. Missing / wished

### Q19: missing
- Sample error messages and exit codes per command.
- A glossary defining "symbol", "stack", "project", "subtree".
- Sample output for `ls`, `check`, `fmt`, `test`, `build`.
- Documented regex dialect for `search`.
- Existence predicate or a `--if-exists` flag on read/write.
- Multi-file `--find`/`--replace` or a `refactor` command.
- Trailing-newline semantics on `write`.
- Concurrency/atomicity guarantees on `write`.
- JSON schema reference (or link) for each command.
- Stack detection rules (how does 8v know a project is Rust vs. Go?).
- Behavior when inside nested projects or monorepos.

### Q20: hesitation when teaching
- Explaining what "symbol map" contains for each language (is a Python `class`'s methods listed? nested fns?).
- Explaining `--full` avoidance without sounding dogmatic — when is it actually fine?
- Explaining error handling when the docs are silent on error surfaces.
- Justifying why `write --find/--replace` is single-file when a learner will expect repo-wide refactor.
- Teaching the `\n`/`\t` parse rule — easy to forget under pressure.

### Q21: vs native
Better: token-efficient default (symbol maps + batching amortize overhead), one consistent CLI for polyglot repos, baked-in verify flow (`check`/`fmt`/`test`/`build`), JSON everywhere.
Worse: no documented error model, narrower write verbs (no patch/diff mode shown), no multi-file refactor primitive, no existence probe, regex dialect not specified, and the "learn by `--help`" fallback is another round-trip.

### Q22: one impactful edit
Add an **Errors & Exit Codes** section: for each command, state (a) success exit code, (b) failure exit code(s), (c) stderr vs stdout for error text, (d) JSON error schema shape, (e) one sample failure. That single section would turn every "trust the error?" question from "partial" to "yes" and would prevent most wrapper-script bugs.

## F. Overall

### Q23: three-axis clarity rating
- **Axis 1 — Input clarity: 8/10.** Commands, flags, and argument grammar (`:line`, `:start-end`, batching) are explained with concrete examples; stack list is enumerated; positional conventions are clear. Loses points for unstated regex dialect, ambiguous `\n` parsing edge-cases, and no flag-reference tables.
- **Axis 2 — Output clarity: 6/10.** Read (symbol map, batch delimiter, JSON shape) is excellent; search default format is shown; but `ls`, `check`, `fmt`, `test`, `build` have zero output samples. `--json` is promised universally with schema given only for read.
- **Axis 3 — Failure-mode clarity: 3/10.** Only `check` mentions a non-zero exit; no command documents error message format, stderr vs stdout, what happens on missing files, or how `write --find` failure surfaces. Large silent gap.
- **Composite mean: (8 + 6 + 3) / 3 = 5.67**

### Q24: one-minute improvement
Add a 6-line "Errors" block after "Verify": exit 0 = ok; exit 1 = user error (missing file, no match); exit 2 = internal; errors go to stderr as plain text, or to the `error` field under `--json`. That alone would raise failure-mode clarity from 3 to ~7 and would cost one minute to write.

## G. Output contracts

### Q25: predicted outputs for util.py fixture
a. `8v read util.py`
   - Prediction: a symbol map with one line for the function and possibly one for `result`. Most likely exact text:
     ```
     2  def add
     ```
     (The module-level `result = add(1, 2)` may or may not appear — instructions don't say whether top-level bindings count as "symbols". Instructions don't say — my guess: only `def` appears, so a single line `2  def add`.)
   - Gap quote: "symbol map. Each line: `<line-number>  <symbol>`" — no Python-specific rules given.

b. `8v read util.py:1-2`
   - Prediction: exact text of lines 1 and 2 (1-indexed, end inclusive):
     ```
     # util.py  (4 lines)
     def add(a, b):
     ```
   - Instructions do not state whether a trailing newline is appended or whether a `=== label ===` header appears for single-file range reads. The text says "Single-file reads emit no header." — so no header.

c. `8v search "add" util.py`
   - Prediction: grouped by file, `<path>:<line>:<text>`:
     ```
     util.py:2:def add(a, b):
     util.py:4:result = add(1, 2)
     ```
   - Gap: regex dialect unstated; assuming `add` matches literally. Also unclear if the file-path header is printed on its own line before matches (the "grouped by file" phrasing). Instructions don't say — my guess: per-line format as shown above, no separate header.

d. `8v read util.py --full`
   - Prediction: the full 4-line content of `util.py` verbatim, with a `=== util.py ===` header? No — "Single-file reads emit no header" says no header for single-file. So just:
     ```
     # util.py  (4 lines)
     def add(a, b):
         return a + b

     result = add(1, 2)
     ```
   - Gap: instructions say `--full` "uses the same `===` delimiter" — but also say "Single-file reads emit no header." These can coexist (delimiter only in batch), but the wording is subtle. Low risk.

### Q26: `8v check .` exit/stream
Instructions state: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue." No exit code on success is stated — presumed 0. No indication of stdout vs stderr for error text. No JSON-mode error schema. Gap: error-channel split is undocumented; only the exit-code direction (non-zero on issue) is stated.

### Q27: `--find`/`--replace` semantics
Instructions state: "fails if `<old>` not found." Nothing about multiple occurrences. Instructions don't say — my guess: all occurrences in the single file are replaced; fails (non-zero exit) with a plain-text stderr message when zero occurrences. Multi-occurrence behavior and the exact failure surface are both gaps.

### Q28: `8v read <nonexistent>` behavior
Instructions don't say. Gap. My guess: non-zero exit code with an error message on stderr; under `--json`, likely a structured error object. The doc gives a JSON success shape (`{"Symbols":{...}}`) but no error shape.

## H. Contract reasoning

### Q29: check silent + exit 1
Not explicitly expected. The instructions say "Non-zero exit on any issue" but also imply the command reports what issue it found (implicitly, since it is a reliability tool). A silent exit 1 with no stdout and no stderr is under-documented and likely a bug or a missing output contract. Next step for the agent: re-run with `--json` to force a structured result; if that is also empty, fall back to running the underlying tools or consult `8v <cmd> --help`. Relevant quote: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue." — no statement on where error text lands, which is the gap.

## I. API coherence

### Q30: differences between Surface 1 and Surface 2
- **Opening line.** Surface 1: "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations." Surface 2: "8v — code reliability tool for AI agents. Designed to minimize round-trips. Use `8v` for all file operations (read, edit, write, search, inspect). Use shell tools only for git, process management, and environment operations." → Surface 2 is more motivational ("minimize round-trips") but Surface 1 names the specific native tools being replaced. An agent seeing only Surface 2 doesn't know which native tools to suppress.
- **Section headings.** Surface 2 adds taglines: "Discovery — learn the repo in one call", "Write — prefer targeted edits". Surface 1 omits those taglines. Minor but Surface 2 conveys intent.
- **Bullet markers.** Surface 1 uses `- ` bullets; Surface 2 uses bare lines. Stylistic; no semantic difference.
- **Batch read contract sentence ordering.** Both surfaces contain the `=== <label> ===` explanation and the `--json` schema; wording is nearly identical. Surface 1 adds: "One call beats N sequential calls." Surface 2 omits that last sentence.
- **Typical flow line.** Both surfaces contain it identically.

Which surface is more complete: **Surface 1** (by a hair) — it names the exact native tools 8v replaces, which is action-guiding for an agent that only sees one surface. Surface 2's added "minimize round-trips" framing is useful but less prescriptive. An agent seeing only Surface 2 might still shell out via Bash for, say, `grep`, because the surface says "shell tools" not "Grep".

## J. Tool-gap surfacing

### Q31: realistic task 8v can't do
Task: **"Rename a symbol `handle_request` to `process_request` across the entire repo, respecting language-specific token boundaries, and update all imports."** Missing capability: multi-file find/replace with symbol-aware boundaries; 8v's `--find`/`--replace` is per-file literal. Closest substitute: `8v search "handle_request"` to enumerate files, then loop `8v write <file> --find "handle_request" --replace "process_request"` over each (cost: N calls + no boundary safety — "handled_request" would also match as a substring? Actually `--find` is literal; so the cost is N calls and risk of false positives on substrings like `handle_requests`). Alternative: fall back to `sed` / LSP rename, both outside 8v.

## K. Behavioral dry-run

### Q32: 5-step Go refactor walk-through
1. **Find all Go source files.**
   - `8v ls --stack go`
   - Confidence: 4
   - Reasoning: `go` is a valid `--stack` value per the enumerated list; returns filtered view. (Could also use `8v search --files "\.go$"` but `--stack` is cleaner.)

2. **Search `http.Get` with 2 lines of context.**
   - `8v search "http\.Get" -e go -C 2`
   - Confidence: 4
   - Reasoning: `-e go` filters by extension, `-C 2` adds two context lines. Regex-escape `.` defensively since dialect is unspecified.

3. **Read the symbol map of the file with the most matches.**
   - Inspect Q32.2 output → pick file `top.go`.
   - `8v read top.go`
   - Confidence: 5
   - Reasoning: default `read` = symbol map.

4. **Replace `http.Get` with `httpClient.Get` on a specific line in that file.**
   - `8v write top.go:<N> "<rewritten line content>"`
   - Confidence: 3
   - Reasoning: requires the exact new line text; alternatively `8v write top.go --find "http.Get" --replace "httpClient.Get"` but that replaces all occurrences (and the task says "on a specific line"). The line-replace form is correct but needs the full rewritten line.

5. **Run the tests and confirm they pass.**
   - `8v test . --json`
   - Confidence: 4
   - Reasoning: `--json` for machine-checkable exit/output; assume non-zero on fail. Exact JSON schema not documented — slight gap.

No step required a native fallback, though step 4 is awkward because I must read the line first to reconstruct it. A `--replace-on-line` variant would help.

## L. Memorability

### Q33: without re-reading
- Insert a new line before line 42 of `main.rs`:
  `8v write main.rs:42 --insert "new line content"`
  Confidence: 5
- Replace lines 10–20 of `main.rs` with multi-line content:
  `8v write main.rs:10-20 "line1\nline2\nline3"`
  Confidence: 5

---

## 8v feedback

- **Discovery step cost.** In this run I had to open the two instruction files to answer — normal for this benchmark — but it highlighted that `8v read` symbol-map-default is the right shape: when I needed to read the prompt template too, the batched call would have amortized the cost nicely. Confirmed the Batch principle pays off on 3+ files.
- **Output contract asymmetry.** `read` has a crisp JSON + text contract; `check`/`fmt`/`test`/`build` have no output contract at all. This shows up directly in Axis 2 and 3 of Q23. If the goal is "agent can predict output," the verify family is the weakest link — prioritize filling those.
- **Error-surface silence is the biggest clarity tax.** Five of my thirteen "gap" callouts (Q7, Q18, Q26, Q27, Q28, Q29) are all the same root cause: no documented error model. One 6-line "Errors & Exit Codes" block in the instructions would collapse that cluster.
- **`--find`/`--replace` scope is a booby trap.** Agents will reach for it expecting repo-wide rename; it is single-file literal. Either rename the flag (e.g., `--find-one`), or add a `refactor` command, or document the scope loudly in the write section. Current wording does not warn.
- **Regex dialect unstated.** For `8v search`, the regex engine is not named. This matters for `.`, `\d`, lookaround, etc. Suggest one line: "`search` uses Rust `regex` syntax (no lookaround, no backrefs)" — or whichever engine is actually used.
- **`--stack` list is great.** Enumerating valid values inline is exactly the right move; I used it in Q32 without ambiguity. Apply the same treatment elsewhere where enums exist.

**Model & run ID:** Claude Opus 4.7 — opus-run2 — 2026-04-20
