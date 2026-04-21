# Instruction Clarity v4 — Opus Run 1 — 2026-04-20
Model: Claude Opus 4.7

### Q1: summary
8v is a code reliability tool for AI agents that replaces native Read/Edit/Write/Grep/Glob/Bash for file operations (Bash is kept only for git, process, env). Use it whenever you read, edit, search, or inspect files; call the MCP tool directly if available. Two principles: **Progressive** (default output is minimal; flags escalate detail) and **Batch** (pass many inputs in one call to amortize per-call overhead).

### Q2: minimum invocations
- `ls` → `8v ls` — Instructions don't say — my guess: lists projects/files. Surface 1/2 only show `8v ls --tree --loc` as the "start here" form.
- `read` → `8v read <path>` — returns a symbol map (`<line>  <symbol>`).
- `search` → `8v search <pattern>` — groups matches by file as `<path>:<line>:<text>`.
- `write` → `8v write <path>:<line> "<content>"` — replaces a single line (no true "minimum" — all forms require a target).
- `check` → `8v check .` — lint + type-check + format-check; non-zero exit on issue.
- `fmt` → `8v fmt .` — auto-format files in place, idempotent.
- `test` → `8v test .` — runs project tests.
- `build` → `8v build .` — compile.

### Q3: two principles
1. **Progressive.** Start minimal, escalate with flags. Non-obvious example: `8v read big.rs` doesn't dump 5000 lines of source — it hands back a symbol index so the agent can ask for a range, saving tokens on first contact.
2. **Batch.** Pack N inputs into one call. Non-obvious example: multiple ranges of the *same* file in one read (`8v read a.rs:1-200 a.rs:500-600`) — the batching works across ranges, not just across distinct files.

### Q4: when to use 8v / when not
Use 8v for anything that reads, edits, searches, or inspects files (reads, edits, writes, search, inspect, lint, test, build, format, directory listing). Do NOT use 8v for git, process management, or environment operations — those stay in Bash. If the 8v MCP tool is available, prefer it over shelling to Bash.

### Q5: discover flags
Run `8v <cmd> --help`. Also: every command accepts `--json`.

### Q6: ambiguous phrases
- "Use Bash only for git, process management, and environment operations." — (a) exclusive whitelist (nothing else may go through Bash), or (b) illustrative examples of the non-file domain.
- "Default output is the minimum useful answer." — (a) minimum for *this command*, or (b) minimum for *any* follow-up context.
- "Last resort" (`--full`) — (a) use only when range reads failed, or (b) soft guidance, still legal anytime.
- "Batch output contract: each file is preceded by `=== <label> ===`" — applies to batches, but "Single-file reads emit no header" — does a single file passed with `--full` emit a header? Text says `--full` uses the same `===` delimiter, contradicting the single-file exception.
- "`--full` applies to every positional arg (repeats accepted, no-op beyond the first)" — (a) duplicate `--full` flags no-op, or (b) duplicate positional args no-op.
- "fails if `<old>` not found" — (a) hard error + non-zero exit, or (b) warning + skip.
- "Non-zero exit on any issue" (check) — silent about whether issue text goes to stdout or stderr.

### Q7: implicit/unstated behaviors
- Exit codes for read/write/search/ls (only check specifies non-zero on issue).
- Error channel: stdout vs stderr for errors.
- JSON error shape.
- Behavior on nonexistent path (read).
- Interleaving order in batch reads (source order? alphabetical?).
- Trailing newline semantics for `write`.
- How multi-line `<content>` is really passed (quoting, shell-escape).
- Unicode / binary file handling.
- Behavior when `<start>` > `<end>` or out of bounds.
- Whether `.gitignore` is respected by `ls`/`search`.
- Concurrency / atomicity of `write`.
- What `ls` outputs *without* `--tree`.
- Whether `8v` auto-detects stack or needs a flag.
- Timeout defaults for test/build.
- Whether `search` regex is PCRE, RE2, ripgrep flavor.

### Q8: undefined terms
"symbol map" (format shown but not what counts as a symbol — does it include traits, macros, enums, variables?), "stack" (never defined; a list of valid values is given), "progressive" (principle name, not defined as a term of art), "overhead" (per-call cost, not quantified), "MCP tool" (assumed knowledge), "line range" conventions (1-indexed, end inclusive — stated, good), "compact mode" (not present in these two surfaces).

### Q9: contradictions
Surface 1 says "Use Bash only for git, process management, and **environment** operations"; Surface 2 (MCP instructions.txt) uses the same phrasing. No direct contradiction between the two surfaces I can spot. Internal tension: "Single-file reads emit no header" vs "`--full` uses the same `===` delimiter" — unclear whether a single-file `--full` read emits a header.

### Q10: batch read output shape
Interleave/concatenate is answered: "each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges." So one section per input, delimited by a header line. Order is implied to be the input order but not stated. JSON form: `{"Multi":{"entries":[{"label":...,"result":{...}},...]}}`.

### Q11: write content semantics
Instructions don't say — my guess: no automatic trailing newline (text doesn't mention one). Multi-line content is written by embedding literal `\n` in the argument; 8v parses escapes itself (`\n` → newline, `\t` → tab, `\\` → backslash). Quotes are shell quotes — they are not parsed by 8v; they're just the shell's way of passing one argument. That part is stated: "Content arguments are parsed by 8v (not the shell)".

## C. Concrete commands

### Q12: scenarios
a. Read 5 files at once — `8v read a.rs b.rs c.rs d.rs e.rs` — **5** — direct from batch example.
b. Replace lines 10–20 of `foo.rs` with 3 lines — `8v write foo.rs:10-20 "line1\nline2\nline3"` — **4** — range replace + escape rules stated; uncertain about trailing newline.
c. Find `handle_*` across repo — `8v search "handle_\w+" (regex)` — **3** — regex flavor unspecified; "(regex)" in usage string is opaque — is it a literal token?
d. Append one line to `notes.md` — `8v write notes.md --append "new line\n"` — **4** — append is listed; whether to include `\n` for newline is my guess.
e. Symbol map for `bar.rs` then lines 100–150 — `8v read bar.rs bar.rs:100-150` — **4** — batching same file across symbol+range isn't explicitly shown but implied.
f. Tests with JSON — `8v test . --json` — **5** — every command accepts `--json`; test is explicit.
g. Check file existence before reading — Instructions don't say — my guess: no dedicated existence check; would fall back to Bash `test -f` or attempt `8v read` and handle error — **2**.
h. Delete lines 50–60 — `8v write foo.rs:50-60 --delete` — **5** — direct.
i. Insert before line 30 — `8v write foo.rs:30 --insert "new"` — **5** — direct.
j. Search Rust files case-insensitive for TODO — `8v search "TODO" -i -e rs` — **4** — flags shown in usage; haven't seen combination explicitly.
k. Files matching `*_test*.md` — `8v ls --match "*_test*.md"` — **4** — `--match <glob>` is documented on `ls`.
l. Lint + format-check + type-check one command — `8v check .` — **5** — explicit.
m. Refactor `old_name` → `new_name` across files — Instructions don't say — my guess: `8v write` `--find`/`--replace` is single-path only; would need to loop per-file or fall back to Bash + sed. Fall back to native. — **2**.
n. Symbols of 10 files in one call — `8v read f1 f2 f3 f4 f5 f6 f7 f8 f9 f10` — **5** — batch read.
o. Lines 1–200 and 500–600 of `big.rs` — `8v read big.rs:1-200 big.rs:500-600` — **5** — explicit example.

### Q13: teach method per command
- `ls` — example (tree+loc, match, stack).
- `read` — example (strong: symbol map, range, batch, --full).
- `search` — example (pattern + flags shown).
- `write` — example (every subform shown).
- `check` — description-only (no example of output).
- `fmt` — description-only.
- `test` — description-only.
- `build` — description-only.

## D. Behavioral prediction

### Q14: three most likely mistakes
1. **Escape thrash on multi-line writes** — using shell-style `$'\n'` or real newlines instead of literal `\n`, causing silent single-line writes or parse errors.
2. **Regex assumption mismatch** — writing a Perl-ish lookbehind in `8v search` and getting zero hits without realizing the engine is RE2-style.
3. **Ambiguous batch `--full`** — mixing range args with `--full` and getting an unexpected dump of every positional because `--full` applies repo-wide to the call.

### Q15: what forces a fallback
- Multi-file find/replace refactor.
- Checking path existence without reading.
- Rename/move/delete whole files.
- Git diff/log/blame — explicitly Bash-only per instructions.
- Process management (kill, ps, long-running servers).
- File permissions / stat / symlink operations.
- Anything requiring regex features beyond the unspecified flavor.

### Q16: most/least used
Most: `8v read` (symbol maps are the main discovery primitive), `8v write` (edits), `8v search` (find usages), `8v check` (gate). Least: `8v fmt` (often subsumed by `check`) and `8v build` (tests usually imply a build for the stacks I'd use).

### Q17: first command in a new repo
`8v ls --tree --loc` — instructions literally say "Start here". It gives the shape of the repo plus line counts so I know which files warrant a symbol map.

### Q18: trust `8v write` errors?
Partially — the write errors I'd trust are the stated ones (`--find` "fails if `<old>` not found"). For others (out-of-range line, permission denied, path missing, concurrent write), the instructions say nothing, so I'd expect to see a bare message and have to guess. Confidence in the error telling me what to do next: **3**.

## E. Missing / wished

### Q19: wished-for additions
- Exit code table per command.
- Error stream (stdout vs stderr) convention.
- JSON error envelope example.
- Regex flavor statement for `search`.
- Behavior of `read` on nonexistent / binary / empty paths.
- A tiny worked example of multi-line write with embedded newlines.
- Whether `ls` respects `.gitignore`.
- Semantics of `--find`/`--replace` on multiple occurrences.
- Timeout defaults.
- Whether `write` is atomic.

### Q20: where I'd hesitate teaching
- Explaining `write`'s escape rules to someone who expects shell semantics — the literal `\n` convention is counterintuitive.
- Explaining what goes in a "symbol map" for languages that aren't shown (only Rust-ish examples).
- Explaining what to do when `check` exits non-zero but emits no text.
- Explaining why `ls` without flags is not documented.

### Q21: vs native
Better: single-command lint/type/format/test gate across stacks; progressive reads save tokens; batching amortizes overhead; one MCP schema instead of five.
Worse: no path existence primitive; no cross-file rename; regex flavor undocumented; error surfaces underspecified; no `mv`/`rm`/`cp`; `read`'s symbol map may not cover every symbol kind the way `grep + eyeballs` does.

### Q22: single highest-impact edit
Add a **contract table** at the bottom: for each command, the exit code on success/failure, where errors go (stdout/stderr), and the JSON error shape. That single addition closes about 60% of the unknowns I flagged.

## F. Overall

### Q23: three-axis clarity rating
- **Axis 1 — Input clarity: 8/10.** Arguments, flags, and positional forms are shown by example for every command; escape rules are explicit. Loses points for undocumented `ls` bare form and opaque `(regex)` token.
- **Axis 2 — Output clarity: 6/10.** Symbol-map and batch delimiter are well specified; JSON shape for `read` is given. But `search`, `check`, `fmt`, `test`, `build`, `ls` output shapes are mostly left to the agent's imagination.
- **Axis 3 — Failure-mode clarity: 3/10.** Only `check` ("non-zero exit on any issue") and `write --find` ("fails if `<old>` not found") mention failure. Exit codes, stderr/stdout split, missing path behavior, JSON error envelope — all absent.
- **Composite mean: (8 + 6 + 3) / 3 = 5.67.**

### Q24: 60-second edit
Append a one-paragraph failure-mode contract: "All commands: exit 0 on success, non-zero on failure. Errors to stderr, data to stdout. `--json` wraps failures as `{\"error\":{\"kind\":...,\"message\":...}}`." That single paragraph would raise axis 3 from 3 → 8.

## G. Output contracts

### Q25: predicted outputs for util.py
a. `8v read util.py` — symbol map. Predicted lines: `2  def add` (and possibly a line for `result = add(1, 2)` — Instructions don't say — my guess: top-level assignments aren't symbols, so only `add` appears). Format per instructions: `<line-number>  <symbol>`. So: `2  def add`.
b. `8v read util.py:1-2` — lines 1–2 of the file verbatim:
```
# util.py  (4 lines)
def add(a, b):
```
No header (single-file read).
c. `8v search "add" util.py` — grouped matches `<path>:<line>:<text>`. Instructions don't say whether the query is regex or literal, and `search` signature shows "(regex)". Likely matches:
```
util.py:2:def add(a, b):
util.py:4:result = add(1, 2)
```
d. `8v read util.py --full` — entire file; delimiter rule says `--full` uses `===` delimiter but single-file emits no header. Instructions don't say — my guess: contents only, no header:
```
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```

### Q26: check exit/stderr
Instructions state: "Non-zero exit on any issue." Success exit code not stated — Instructions don't say — my guess: 0. Channel not stated — Instructions don't say — my guess: findings to stdout, diagnostics to stderr. JSON form says "accepts `--json`" but the envelope shape is not specified. Gap quote: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue."

### Q27: find/replace semantics
Zero occurrences: instructions state "fails if `<old>` not found" — so a hard failure (presumably non-zero exit, text on stderr — Instructions don't say on the stderr bit). More than one occurrence: Instructions don't say — my guess: replaces all occurrences (typical `--find`/`--replace` semantics). Return: Instructions don't say — my guess: summary line + non-zero on failure.

### Q28: read on missing path
Instructions don't say. Gap: neither surface specifies behavior for a nonexistent path. My guess: non-zero exit, error message on stderr, no output on stdout; with `--json` probably a structured error — but this is inference.

## H. Contract reasoning

### Q29: silent check exit 1
Based on instructions: `check` says "Non-zero exit on any issue" but is silent on where the issue description goes. So exit 1 with no stdout/stderr is **not** explicitly documented either way — it's a gap. What the agent should do next: re-run with `--json` to try to get structured output; if still empty, run `8v check . --help` to see if there's a verbose flag; otherwise fall back to stack-native tools (cargo clippy, etc.) to locate the issue. Relevant quote: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue."

## I. API coherence

### Q30: Surface 1 vs Surface 2 differences
1. Surface 1 uses bullet-list form (`-` prefix) for commands under Discovery/Read/Write/Verify; Surface 2 uses unprefixed lines. Cosmetic, not semantic.
2. Surface 1 says "Use Bash only for **git, process management, and environment operations**"; Surface 2 says "Use shell tools only for **git, process management, and environment operations**" — "Bash" vs "shell tools" is a semantic widening in Surface 2 (more permissive — covers zsh/fish/pwsh agents).
3. Surface 2 section header is "Write — prefer targeted edits"; Surface 1 is just "Write". Surface 2 adds an implicit preference ordering.
4. Surface 2 section header is "Discovery — learn the repo in one call"; Surface 1 is just "Discovery". Surface 2 reinforces the batch principle.
5. Surface 1 intro mentions "For anything that reads, edits, searches, or inspects files, use 8v — not Bash"; Surface 2 phrases similarly but prepends the positioning sentence "8v — code reliability tool for AI agents. Designed to minimize round-trips."
6. Typical-flow line is identical.
7. Both have the same valid `--stack` values list.

More complete: **Surface 2**, because it includes the positioning sentence ("designed to minimize round-trips") that explains *why* an agent should use 8v, plus the "prefer targeted edits" guidance. An agent seeing only Surface 1 loses the motivation framing, which weakens compliance when the agent is tempted to fall back.

## J. Tool-gap surfacing

### Q31: task not completable with 8v
Task: "Rename `internal/auth/legacy.go` to `internal/auth/deprecated.go` and update all imports." Missing capabilities: (a) file rename/move primitive; (b) repo-wide find-replace (`--find`/`--replace` is single-path). Closest substitute: `git mv` (Bash) + a shell loop calling `8v write <path> --find ... --replace ...` on every matching file from `8v search --files`. Cost: at least N+1 calls (1 search + 1 rename + N writes) plus the brittleness of not having an atomic refactor.

## K. Behavioral dry-run

### Q32: 5-step walkthrough
1. Find all Go files — `8v ls --stack go --tree` — **3** — `--stack go` is documented; whether the output cleanly lists files (vs projects) is an assumption. Alt: `8v search "" --files -e go` — **3**.
2. `http.Get` usages with 2-line context — `8v search "http\.Get" -e go -C 2` — **4** — flags all documented; regex escaping of `.` is my convention.
3. Symbol map of top-match file — Instructions don't say — my guess: no aggregated "count by file" flag; I'd inspect step 2 output manually and pick the file with the most `<path>:<line>:...` lines, then `8v read <that-file>` — **3**.
4. Replace `http.Get` with `httpClient.Get` on a specific line in that file — `8v write <file>:<line> "<new-line-content>"` — **4** — straightforward range replace; requires reading the line first to preserve surrounding context.
5. Run tests and confirm — `8v test .` — **5** — direct; exit 0 = pass (Instructions don't say on exit semantics of test, but it's the established stack convention).

## L. Memorability

### Q33: exact syntax from memory
Insert new line before line 42 of `main.rs`: `8v write main.rs:42 --insert "<content>"` — confidence **5**.
Replace lines 10–20 with multi-line content: `8v write main.rs:10-20 "line1\nline2\nline3"` — confidence **4** (slight uncertainty on whether the shell-level quoting needs special handling for embedded newlines — instructions say pass literal `\n`, so the above should be correct).

---

**Model & run ID:** Claude Opus 4.7 — clarity-v4-opus-run1

## 8v feedback
- Surface 1 and Surface 2 are near-duplicates with small wording drifts (Bash vs shell tools, section titles). Pick one source of truth and derive the other mechanically, or the agent burns tokens reconciling them.
- `ls` has no documented bare form; the text jumps straight to `--tree --loc`. An agent asked "what does `8v ls` alone do?" has to guess.
- The `(regex)` token in the `search` usage line is opaque — it reads like a literal argument rather than "pattern is a regex". Replace with a prose note: "pattern is a regex (RE2 flavor)".
- Failure-mode coverage is the weakest axis (3/10). A two-line contract block (exit codes, stderr/stdout, JSON error envelope) would close most gaps.
- `--find`/`--replace` needs a one-line note on multi-occurrence semantics and whether it's whole-file or line-scoped.
- Batch-read delimiter rule has an internal tension: "single-file emits no header" vs "`--full` uses the same `===` delimiter" — clarify the single-file `--full` case.
- `write` escape rules are explicit and good — keep that paragraph.
- The "Typical flow" closing line is the single most memorable artifact in the doc; worth keeping at the bottom of both surfaces verbatim.
