# Instruction Clarity v3 — Opus Run 1 — 2026-04-19
Model: Claude Opus 4.7
Surfaces read: ai_section.txt, instructions.txt

---

### Q1: summary
8v is a code reliability tool for AI agents that replaces native file operations (Read, Edit, Write, Grep, Glob, and most Bash) with a single CLI; use it whenever you'd read, edit, search, or inspect files, and leave Bash for git/process/env. The two principles are (1) **Progressive** — defaults return the minimum useful answer and flags escalate detail, and (2) **Batch** — pass many inputs in one call to amortize per-call overhead.

### Q2: minimum-viable invocations
- `ls`: `8v ls` — returns a file listing (exact default shape not stated; `--tree --loc` adds hierarchy+line-counts).
- `read`: `8v read <path>` — symbol map (`<line>  <symbol>` per line).
- `search`: `8v search <pattern>` — matches grouped by file as `<path>:<line>:<text>`.
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line.
- `check`: `8v check .` — lint+type-check+format-check; non-zero exit on issues. Stdout content unstated.
- `fmt`: `8v fmt .` — auto-formats in place; idempotent.
- `test`: `8v test .` — runs project tests. Output shape unstated.
- `build`: `8v build .` — compiles. Output shape unstated.

### Q3: two principles + examples
- **Progressive**: commands return the smallest useful view by default; you opt into more. Non-obvious example: `8v read foo.rs` does NOT return file contents — it returns a symbol map, and you must then ask for `:start-end` to see code.
- **Batch**: multiple inputs in one call beat N sequential calls. Non-obvious example: you can batch *ranges of the same file* in one call: `8v read a.rs:1-200 a.rs:200-400`.

### Q4: when to use 8v / when not
Use 8v for anything that reads, edits, searches, or inspects files. Use Bash only for git, process management, and environment operations that 8v doesn't cover. If the `8v` MCP tool is available, call it directly — do not shell out via Bash.

### Q5: flag discovery
`8v <cmd> --help` returns the full flag list. Also: every command accepts `--json`.

---

### Q6: ambiguous phrases
- "Use Bash only for git, process management, and environment operations." — Dual reading: (1) *only these three categories* are Bash's turf; (2) Bash is *allowed* for these plus anything 8v can't do.
- "Default output is the minimum useful answer." — Dual reading: (1) literally minimum-sized; (2) minimum *necessary* context, which could be large for big files.
- "Each call costs overhead — amortize it." — Dual reading: (1) network/protocol overhead; (2) token/schema overhead. Not specified which.
- "batch any combination of paths and ranges in one call" — Dual reading: (1) output is one merged blob; (2) one symbol-map per input file concatenated.
- "`8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found." — Dual reading: (1) does nothing if zero matches; (2) behavior on multiple matches is unstated — replace all? first? fail?
- "end inclusive" on `:start-end` — clear, but silent on whether `start > end` errors or swaps.
- "`\n` becomes a newline" — ambiguous whether this applies to ALL content args (write only?) or also `--append` shell quoting rules.

### Q7: implied-but-unstated behaviors
- Exit codes (only `check` mentions non-zero).
- Error text location: stdout vs stderr vs `--json`-only.
- What happens when a path doesn't exist.
- JSON shape — `--json` is advertised but no schema.
- Output interleaving order for batched reads.
- Whether `8v write` creates the file if it doesn't exist (for `--append`, etc.).
- Whether `8v write` is atomic / how it behaves under concurrent edits.
- Whether trailing newlines are auto-added.
- Unicode handling (line counting in bytes vs chars).
- Symbol-map coverage per language (does it work for Python? YAML? JSON?).
- Line-range semantics beyond "1-indexed, end inclusive" (empty range? negative?).
- `8v search` regex flavor (PCRE? RE2? Rust regex?).
- Binary-file behavior for `read`/`search`.
- Permissions errors, symlink handling, `.gitignore` respect.

### Q8: undefined terms
- "symbol map" — partially defined by example (`<line>  <symbol>`), but the extraction rules per language are not defined.
- "stack" — the `--stack` flag lists valid values but doesn't define what a "stack" is (language? toolchain? build system?).
- "progressive" — named as a principle, not formally defined.
- "overhead" — referenced but not quantified.
- "subtree" — used in verify scoping without definition.
- "symbol" — what counts as one? Just top-level defs, or nested?
- "idempotent" — fine for experts, undefined for newcomers.

### Q9: contradictions between/within surfaces
- Surface 1 says "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations" AND "Use Bash only for git, process management, and environment operations." Surface 2 says "Use shell tools only for git, process management, and environment operations." Near-identical but Surface 1 has an extra sentence: "For anything that reads, edits, searches, or inspects files, use 8v — not Bash." Surface 2 omits the explicit list (Read/Edit/Write/Grep/Glob). An agent reading only Surface 2 wouldn't know the specific native tools being replaced.
- Surface 1 under Discovery mentions the `--files` and `-C N` behavior inline; Surface 2 has the same search paragraph. Both consistent here.
- No hard contradictions, but Surface 1 has a "Designed to minimize round-trips" framing only in Surface 2's first sentence — that rationale is missing from Surface 1.
- Surface 1 uses bullet markers (`-`); Surface 2 uses inline backticked commands. Semantically equivalent.

### Q10: batched read output shape
Instructions don't say — my guess: one symbol map per file concatenated, separated by a file header, because the text says "batch any combination of paths and ranges in one call" but never defines the delimiter or interleaving rule. Gap: no output contract for multi-input reads.

### Q11: write content semantics
- Trailing newline: Instructions don't say — my guess: no auto-newline; what you pass is what's written, because the examples all use literal quoted content without mentioning newlines.
- Multi-line content: explicitly stated — "`\n` becomes a newline, `\t` a tab, `\\` a literal backslash. Pass them as literal two-character sequences."
- Shell quote meaning: explicitly stated — "Content arguments are parsed by 8v (not the shell)." So quotes are the shell's job only to keep the arg as one token; 8v sees the inner string.

---

### Q12: concrete commands + confidence

a. Read 5 files at once — `8v read a.rs b.rs c.rs d.rs e.rs` — **5** — batch is the documented primary pattern.
b. Replace lines 10–20 of `foo.rs` with 3-line content — `8v write foo.rs:10-20 "line1\nline2\nline3"` — **4** — syntax is documented; unclear if trailing newline is added.
c. Find all functions named `handle_*` — `8v search "fn\s+handle_\w+"` — **3** — regex flavor not specified; this assumes PCRE-like.
d. Append one line to `notes.md` — `8v write notes.md --append "new line\n"` — **3** — unclear if `--append` adds a trailing newline automatically.
e. Symbol map then read 100–150 — `8v read bar.rs bar.rs:100-150` — **4** — batching a symbol map + range in one call is documented.
f. Run tests and parse JSON output — `8v test . --json` — **4** — JSON flag universal; schema unknown.
g. Check file exists before reading — Instructions don't say — my guess: fall back to Bash `test -f` because 8v has no stat/exists command in the text.
h. Delete lines 50–60 — `8v write <path>:50-60 --delete` — **5** — explicitly documented.
i. Insert new line before line 30 — `8v write <path>:30 --insert "<content>"` — **5** — explicitly documented.
j. Search Rust, case-insensitive, `TODO` — `8v search "TODO" -i -e rs` — **4** — flags documented; exact syntax of `-e` (value `rs` vs `.rs`) inferred.
k. Files matching `*_test*.md` — `8v ls --match "*_test*.md"` — **4** — documented pattern.
l. Lint+format-check+type-check in one — `8v check .` — **5** — documented as all three.
m. Multi-file refactor `old_name` → `new_name` — Instructions don't say — my guess: `8v search "old_name" --files` to find files, then per-file `8v write <path> --find "old_name" --replace "new_name"`; there's no repo-wide find/replace documented.
n. Symbol maps of 10 files in one call — `8v read f1 f2 f3 f4 f5 f6 f7 f8 f9 f10` — **5** — batch pattern.
o. Two ranges of `big.rs` in one call — `8v read big.rs:1-200 big.rs:500-600` — **5** — explicit example in docs.

### Q13: teaching mode per command
- `ls` — **example** (`8v ls --tree --loc`) + description.
- `read` — **example** (symbol-map sample output) + description.
- `search` — **description-only** with inline flag list; no full example invocation with a result.
- `write` — **example** (each sub-form has a syntax line) but no sample output.
- `check` — **description-only**.
- `fmt` — **description-only**.
- `test` — **description-only**.
- `build` — **description-only**.

---

### Q14: three most likely mistakes
1. Forgetting that `8v read <path>` returns a symbol map, not file content — I'd waste a turn expecting code and re-issuing with `--full` or a range.
2. Writing multi-line content with real newlines inside the quoted string instead of the literal `\n` sequence the parser expects, producing a malformed file.
3. Using `8v search` with a regex that assumes PCRE features (lookahead, backreferences) that the underlying engine (likely RE2 or Rust regex) doesn't support, causing silent mismatches.

### Q15: fall back to native tools when
- Need to check file existence / stat / permissions.
- Need to create a new file from scratch (unclear whether `write --append` or `write --insert` on a nonexistent file works).
- Need a repo-wide find-replace (no documented single command).
- Need to read directory metadata beyond name+LOC.
- Need to move/copy/delete files.
- Need to run an arbitrary build tool not covered by `build`/`test`/`check`/`fmt`.

### Q16: most/least used
- Most: `8v read` (symbol map + ranges) and `8v ls --tree --loc` — core discovery.
- Least: `8v fmt` and `8v build` — usually rolled into `check` or project Makefile.

### Q17: first command in a new repo
`8v ls --tree --loc`. The docs explicitly say "Start here" and the typical flow begins with it.

### Q18: trust write errors?
**Uncertain.** Instructions don't say what `8v write` error messages look like. They only state that `--find "<old>"` "fails if `<old>` not found" but not what the failure message contains. I would not fully trust the error to be self-diagnosing.

---

### Q19: missing / wished
- Exit-code table per command.
- `--json` schema samples.
- Behavior when path doesn't exist.
- Regex flavor for `search`.
- Repo-wide find-replace flag.
- Stat/exists/delete/mv file commands (or explicit "use Bash for these").
- Output-delimiter spec for batched reads.
- Glossary: "symbol", "stack", "subtree".
- Language-support matrix for symbol maps.
- Sample output for `check`/`test`/`build`.

### Q20: where I'd hesitate teaching
- The symbol-map default for `read` — surprising for newcomers.
- The `\n` literal-sequence rule — easy to violate.
- When to use `8v write --find/--replace` vs `:line-range` — the instructions don't give a decision rule.
- Whether `8v` works in non-MCP mode the same way (the Bash form vs MCP tool distinction is hinted but not taught).

### Q21: 8v better / worse vs native
- **Better**: one-call batching; symbol-map default dramatically cuts tokens; structured line-addressing (`:start-end`) is more precise than Edit's fuzzy matching; uniform `--json` flag.
- **Worse**: no stat/exists/mv/rm; no repo-wide find-replace; regex flavor undocumented; error contracts undocumented; can't handle interactive prompts or streaming; symbol-map coverage per language unknown.

### Q22: single biggest instruction edit
Add one "Output contracts" section: for each command, show a 3-line exact sample of (1) successful stdout, (2) error stderr + exit code, (3) `--json` shape. That single addition eliminates most behavioral guesswork.

---

### Q23: three-axis clarity rubric
- **Input clarity: 8/10** — command syntax and flag surface are well-explained with examples for `read`, `write`, `ls`, `search`; only `check`/`fmt`/`test`/`build` lack examples and a few flag values (`-e rs` vs `-e .rs`) are ambiguous.
- **Output clarity: 5/10** — `read` has a sample; `search` is described; but `check`/`fmt`/`test`/`build`/`ls` default outputs are not shown, and batched-read interleaving is undefined.
- **Failure-mode clarity: 2/10** — only `write --find/--replace` failure and `check` non-zero exit are mentioned; everything else (missing path, bad range, regex error, write conflict) is silent.
- **Composite mean: (8 + 5 + 2) / 3 = 5.00**

### Q24: one-minute improvement
Add a two-column table at the top: "Command | Default output | Exit-code on failure | Where errors go (stdout/stderr/json)". This single table cures most of the Output and Failure-mode gaps.

---

### Q25: predict outputs for util.py fixture

a. `8v read util.py`:
```
1  def add
3  result
```
Confidence 2 — docs only guarantee the shape `<line>  <symbol>`; they don't confirm whether Python top-level variables count as symbols, or whether the function header line is `1` (the `def`). Gap: language-specific symbol rules not specified.

b. `8v read util.py:1-2`:
```
def add(a, b):
    return a + b
```
Confidence 4 — "line range (1-indexed, end inclusive)" is explicit; I assume raw file lines are returned verbatim. Gap: format around the range (any header? line-number prefix?) is not specified.

c. `8v search "add" util.py`:
```
util.py:1:def add(a, b):
util.py:3:result = add(1, 2)
```
Confidence 4 — docs state "Default output groups matches by file: `<path>:<line>:<text>`".

d. `8v read util.py --full`:
```
def add(a, b):
    return a + b

result = add(1, 2)
```
Confidence 3 — docs say `--full` means "entire file" but don't confirm whether line numbers are prefixed or not. Gap: `--full` output framing undefined.

### Q26: `8v check .` exit/stream contract
Docs state: "Non-zero exit on any issue." Zero on success is implied, not stated. Where error text appears (stdout vs stderr vs `--json`-only) is **not specified**. Gap: no statement on error channel or default-vs-`--json` differences.

### Q27: `write --find --replace` edge cases
Docs state only: "fails if `<old>` not found." Zero-match behavior is therefore a failure, but:
- Error format/exit code: **not specified**.
- Behavior when `<old>` appears multiple times: **not specified** — replaces all? first? errors as ambiguous? Gap.
- Return value to caller: **not specified** (no count of replacements, no diff).

### Q28: `8v read` on nonexistent path
Docs don't say — my guess: a non-zero exit with an error message on stderr, but the instructions are entirely silent on this. No mention of stdout vs stderr, exit code, or structured JSON error shape. Gap: no nonexistent-path contract.

---

### Q29: silent exit-code-1 from `check`
Based only on the instructions, this is **not expected** — the docs say "Non-zero exit on any issue" but imply (via "lint + type-check + format-check") that the tool surfaces the issue somewhere. Silent stdout+stderr with exit 1 violates the implicit contract. The agent should re-run with `--json` (the docs promise `--json` on every command) to see if structured output appears there. If that also yields nothing, the agent should fall back to Bash and run the underlying linters directly. Gap: the instructions never say where `check` writes its findings.

---

### Q30: Surface-1 vs Surface-2 factual diffs
- **S1 has** an explicit native-tool list: "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations." **S2 says** only "Use `8v` for all file operations." S1 more complete. Matters because an agent reading S2 won't know Grep and Glob specifically are replaced.
- **S2 has** a rationale sentence: "Designed to minimize round-trips." S1 omits this. S2 more motivational; matters because it explains *why* batching matters.
- **S1 has** "If the `8v` MCP tool is available, call it directly — do not shell out via Bash." S2 lacks this routing rule. S1 more complete. Matters because an agent with both the MCP tool and a Bash channel could otherwise pick the wrong one.
- **Typical-flow line** is present in both surfaces, identical.
- **Valid `--stack` values list** is identical in both.
- **Read/Write/Verify blocks**: structurally identical; S1 uses bullet points, S2 uses bare lines. No factual diff in content.
- **S1 has** "Write the full flag list" guidance via `--help` — both do.

Net: S1 is marginally more complete (native-tool naming + MCP-vs-Bash routing); S2 has the round-trips rationale. An agent with only S2 would miss the specific tools being displaced.

---

### Q31: realistic task 8v can't do
**Task**: "Move `src/util.rs` to `src/utils/mod.rs` and update all its `use` imports across the repo."
- Missing capabilities: no `8v mv` / rename, no repo-wide find-replace command.
- Closest substitute: Bash `mv` for the file move; then `8v search "use crate::util"` to list sites; then per-file `8v write <path> --find ... --replace ...`. Cost: O(N) write calls (one per matching file), plus manual search-result parsing, versus a single `sed -i` or IDE refactor.

---

### Q32: 5-step Go-refactor dry-run
1. Find all Go source files — `8v ls --stack go` — confidence **4** (stack value `go` is in the allow-list; default output shape not confirmed).
2. Search `http.Get` with 2 lines of context — `8v search "http\.Get" -e go -C 2` — confidence **4**.
3. Read symbol map of file with most matches — `8v read <that_path>` — confidence **5** (default is symbol map).
4. Replace `http.Get` with `httpClient.Get` on a specific line — `8v write <path>:<line> --find "http.Get" --replace "httpClient.Get"` — confidence **2**. Gap: `--find/--replace` is documented at file level, not combined with `:line`. Likely correct form is `8v write <path> --find "http.Get" --replace "httpClient.Get"` at file scope, but then it affects every occurrence in the file, which may not be what step 4 asks. **Fallback to native Edit** here.
5. Run tests — `8v test .` — confidence **5**.

### Q33: memorability (no re-read)
- Insert before line 42 of main.rs: `8v write main.rs:42 --insert "<content>"` — confidence **4**.
- Replace lines 10–20 with multi-line content: `8v write main.rs:10-20 "line1\nline2\nline3"` — confidence **4**.

---

**Model & run ID:** Claude Opus 4.7 — opus-run1 — 2026-04-19

## 8v feedback

- **Command**: `8v read <path1> --full <path2> --full <path3> --full`
  **What went wrong**: `error: the argument '--full' cannot be used multiple times`.
  **Expected**: Either (a) `--full` should be implicitly per-path so a batch of `--full` reads works, or (b) the docs should explicitly say `--full` is a single global flag that applies to all paths in the batch. I had to retry with a single trailing `--full` that (per my observation) applied to all three files. This is a real friction during batched reads of small files where the symbol map is less useful than the content. Ambiguity matches Q10/Q25d gap.

- **Observation**: batching worked smoothly otherwise — three files returned in one call with clear `=== <path> ===` delimiters. That delimiter format is not documented in either surface; adding it to the instructions would close the Q10 gap.

- **No other friction** hit during this run. `8v read ... --full` with a single flag successfully returned all three files concatenated with headers.
