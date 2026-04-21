# Instruction Clarity v4 — Sonnet Run 2 — 2026-04-20
Model: Claude Sonnet 4.6

---

### Q1: Summary
8v is a code reliability tool for AI agents that replaces native file operations (Read, Edit, Write, Grep, Glob) with a single unified CLI interface. Use it for all file reading, editing, searching, and verification; reserve shell tools only for git, process management, and environment operations. Two core principles govern every command: **Progressive** (default output is the minimum useful answer, flags escalate detail) and **Batch** (pass many inputs in one call to amortize per-call overhead).

---

### Q2: Minimum-viable invocations

- `ls`: `8v ls --tree --loc` — returns full file hierarchy with line counts
- `read`: `8v read <path>` — returns a symbol map (line number + symbol name per line)
- `search`: `8v search <pattern>` — returns matches grouped by file: `<path>:<line>:<text>`
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line (no return value stated; assumed success/failure signal)
- `check`: `8v check .` — runs lint + type-check + format-check; non-zero exit on any issue
- `fmt`: `8v fmt .` — auto-formats files in place
- `test`: `8v test .` — runs project tests
- `build`: `8v build .` — compiles the project

---

### Q3: The two principles

**Progressive:** The default invocation returns the least information needed to proceed. Additional flags unlock more detail. Non-obvious example: `8v search "foo"` returns compact `path:line:text` by default — you don't get surrounding code unless you add `-C N`. The brevity is deliberate; adding context only when needed saves tokens.

**Batch:** Multiple inputs go in a single call rather than N sequential calls. Non-obvious example: `8v read a.rs:1-200 a.rs:200-400 b.rs` batches two ranges of the *same* file plus a different file in one invocation — the instructions explicitly show this pattern. Batching amortizes per-call overhead (MCP round-trip cost, process spawn, etc.).

---

### Q4: When to use 8v vs. not

**Use 8v for:** reading files (symbol maps, ranges, full), searching, writing/editing, running verify commands (check, fmt, test, build), and discovering project structure.

**Do NOT use 8v for:** git operations, process management (starting/stopping processes), and environment operations (env var inspection, process listing). The instructions are explicit: "Use Bash only for git, process management, and environment operations."

---

### Q5: Discovering flags

Run `8v <cmd> --help`. Both surfaces state: "Run `8v <cmd> --help` for the full flag list."

---

### Q6: Ambiguous phrases

1. **"Use Bash only for git, process management, and environment operations"** — Reading 1: Bash is forbidden for everything not in that list. Reading 2: Those are suggestions and Bash can be used more broadly when 8v is inconvenient.

2. **"Last resort"** (re: `--full`) — Reading 1: Never use `--full` unless symbol map + ranges have genuinely failed. Reading 2: Loosely means "prefer other options first" but `--full` is fine when convenient.

3. **"If an `8v` MCP tool is available (search tools for `8v`), call it directly"** — Reading 1: Always prefer the MCP tool over shell invocation when both are available. Reading 2: The parenthetical "search tools for `8v`" might mean the agent should literally search its tool list, implying it might not always be there.

4. **"Fails if `<old>` not found"** (for `--find/--replace`) — Reading 1: The command exits with non-zero and produces an error. Reading 2: "Fails" might mean it silently produces no change. The failure mode is not further specified.

5. **"Repeats accepted, no-op beyond the first"** (re: `--full` applied multiple times) — Reading 1: Passing `--full` twice is silently ignored. Reading 2: It might produce a warning.

6. **"overhead"** in the batch principle — Reading 1: MCP round-trip / process cost per call. Reading 2: Token cost of the invocation itself. Both readings suggest batching is good, but the underlying reason differs.

---

### Q7: Implied-but-unstated behaviors

1. **Exit codes for write commands** — The instructions never state what exit code `8v write` returns on success or failure.
2. **Error format for missing paths** — No description of what `8v read nonexistent.rs` returns (error on stdout? stderr? JSON? non-zero exit?).
3. **Whether `write` is atomic** — No statement about what happens if write fails mid-operation (partial write? rollback?).
4. **Output destination (stdout vs. stderr)** — Never stated which stream carries normal output vs. error output.
5. **Whether `--json` affects exit codes** — Unclear if `--json` changes exit behavior or only output format.
6. **Unicode handling** — No mention of character encoding, UTF-8 assumptions, or BOM behavior.
7. **What "symbol map" includes** — Stated as "functions, structs, classes" in some surfaces but not explicitly defined in the tested surfaces.
8. **Behavior when `--find` matches multiple times** — Stated it fails if not found, silent about multiple matches.
9. **Whether `search` respects `.gitignore`** — Mentioned in one surface (oss/vast CLAUDE.md) but not in the two tested surfaces.
10. **Line ending normalization** — `\n` in content is stated, but behavior on Windows-style `\r\n` files is not.
11. **Interleaving of stdout/stderr in verify commands** — `8v check .` might mix output streams in ways an agent parsing output needs to handle.

---

### Q8: Undefined terms

- **"symbol map"** — Used extensively but never formally defined in the tested surfaces. The example output (`fn main`, `pub struct Args`, `impl Args`) implies Rust, but what counts as a symbol in Python, Go, or JS is not stated.
- **"stack"** — Valid values are listed, but the term itself (what a "stack" means: language ecosystem? build system?) is not defined.
- **"progressive"** — Defined in the context of 8v, but the term is used as if self-explanatory.
- **"overhead"** — "Each call costs overhead" — which overhead: latency? tokens? tool-call budget? Not specified.
- **"compact mode"** — Mentioned in one alternate surface but not in the two tested surfaces. In the tested surfaces, `search` default output is described as `<path>:<line>:<text>` groups.
- **"schema tax"** — Not present in tested surfaces.
- **"most Bash"** — Not present in tested surfaces.

---

### Q9: Contradictions

Between Surface 1 (ai_section.txt) and Surface 2 (instructions.txt):

1. **Section headers differ slightly:** Surface 1 uses `## Write`, Surface 2 uses `## Write — prefer targeted edits`. The sub-heading in Surface 2 adds a directive that Surface 1 omits.
2. **Batch output contract wording:** Surface 1 says "Label is the relative path" explicitly, Surface 2 compresses slightly. No factual contradiction.
3. No direct contradictions in commands or flags found — both surfaces are nearly identical in content.

Within surfaces: No internal contradictions found.

---

### Q10: Batch output interleaving

From the instructions: "each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges. Single-file reads emit no header."

Conclusion: **Concatenated, not interleaved.** Each file's output block follows its `=== label ===` header sequentially. The output is an ordered stream: header, then that file's content, then the next header, then the next file's content. There is no interleaving — the delimiter protocol makes order deterministic.

---

### Q11: Trailing newline and multi-line in write

**Trailing newline:** Instructions don't say — the instructions state `\n` becomes a newline, but they do not specify whether a trailing newline is automatically appended to content. My guess: no automatic trailing newline is added; the content is written literally.

**Multi-line content:** Use `\n` inside the quoted content string. Example: `8v write foo.rs:10-20 "line one\nline two\nline three"`. The instructions explicitly state: "Content arguments are parsed by 8v (not the shell): `\n` becomes a newline."

**Shell-escape meaning of quotes:** No — the instructions say "do not rely on shell interpolation." The outer quotes are shell quoting to pass the argument; the `\n`, `\t`, `\\` sequences are interpreted by 8v, not the shell. The quotes themselves have standard shell meaning (delimit the argument) but the escape sequences inside are 8v's responsibility.

---

### Q12: Concrete commands + confidence

**a. Read 5 files at once.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs`
Confidence: 5 — The batch pattern is explicitly demonstrated.

**b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.**
Command: `8v write foo.rs:10-20 "line one\nline two\nline three"`
Confidence: 4 — Syntax is shown; `\n` for newlines is explicitly stated. Minor uncertainty: whether the 3-line content exactly replaces 11 lines or whether line count mismatch is valid.

**c. Find all functions named `handle_*` across the repo.**
Command: `8v search "handle_" --files` or `8v search "fn handle_"`
Confidence: 3 — `search` supports regex, so `8v search "fn handle_\w+"` would work for Rust. But the instructions don't confirm regex flavor or that function definitions are targeted vs. all call sites. `--files` shows only filenames if that's wanted.

**d. Append one line to `notes.md`.**
Command: `8v write notes.md --append "new line content"`
Confidence: 5 — Explicitly described.

**e. Symbol map for `bar.rs`, then read lines 100–150.**
Commands: `8v read bar.rs` then `8v read bar.rs:100-150`
Confidence: 5 — Both forms are explicitly described and the workflow is stated as the recommended pattern.

**f. Run tests and parse JSON output.**
Command: `8v test . --json`
Confidence: 5 — "Every command accepts `--json`" and `8v test .` is explicitly described.

**g. Check whether a file exists before reading.**
Instructions don't say — no `8v exists` or similar command is described. My guess: attempt `8v read <path>` and handle the error. I would fall back to Bash: `test -f <path>` or the Read tool. Confidence: 1.

**h. Delete lines 50–60.**
Command: `8v write foo.rs:50-60 --delete`
Confidence: 5 — Explicitly described.

**i. Insert a new line before line 30.**
Command: `8v write foo.rs:30 --insert "new line content"`
Confidence: 5 — Explicitly described.

**j. Search only Rust files, case-insensitive, for `TODO`.**
Command: `8v search "TODO" -i -e rs`
Confidence: 5 — Both `-i` and `-e <ext>` are explicitly listed.

**k. Find all files by name matching `*_test*.md`.**
Command: `8v ls --match "*_test*.md"`
Confidence: 4 — `--match <glob>` is described for `ls`. Minor uncertainty: whether `ls --match` searches recursively or only the current level.

**l. Run lint + format-check + type-check with one command.**
Command: `8v check .`
Confidence: 5 — Explicitly stated: "lint + type-check + format-check."

**m. Replace `old_name` with `new_name` across a multi-file refactor.**
Instructions don't say a multi-file `--find/--replace` exists. `--find/--replace` operates on a single `<path>`. For multi-file: run `8v search "old_name" --files` to find affected files, then loop `8v write <each_file> --find "old_name" --replace "new_name"`. Confidence: 3 — this is inferred, not stated; I'd consider Bash for automation.

**n. Read just the symbols of 10 files in one call.**
Command: `8v read f1.rs f2.rs f3.rs f4.rs f5.rs f6.rs f7.rs f8.rs f9.rs f10.rs`
Confidence: 5 — Default `read` is the symbol map; batch is explicitly supported.

**o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.**
Command: `8v read big.rs:1-200 big.rs:500-600`
Confidence: 5 — "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)" is explicitly shown as a supported pattern.

---

### Q13: Teaching method per command

| Command | Method |
|---------|--------|
| `ls`    | Example (`8v ls --tree --loc`) + description |
| `read`  | Example (symbol map output shown) + description |
| `search`| Description + partial example (output format shown) |
| `write` | Example (all variants listed) + description of escape rules |
| `check` | Description only (behavior stated, no sample output) |
| `fmt`   | Description only |
| `test`  | Description only |
| `build` | Description only |

---

### Q14: Three most likely mistakes

1. **Using `--find/--replace` assuming it's multi-file.** The syntax `8v write <path> --find "<old>" --replace "<new>"` operates on a single file. On a refactor across 20 files, I'd need 20 separate calls. If I try passing multiple paths, it may error or silently act on the first. This is not clearly spelled out.

2. **Shell-interpolating escape sequences.** Writing `8v write foo.rs:5 "line with\nnewline"` in a shell that interprets `\n` before 8v sees it. The instructions say "do not rely on shell interpolation" but don't say to use single quotes or how exactly to prevent the shell from eating the backslash. In practice, `$'...'` or single quotes may be needed depending on the shell.

3. **Assuming `8v read` on a non-existent file gives a clear error.** The instructions don't describe the failure mode for missing paths. I might write code that reads a potentially absent file and be surprised by an opaque error or an empty symbol map rather than a clear file-not-found signal.

---

### Q15: Fallback conditions

I would fall back to native tools when:

1. **File existence check** — No 8v command for this.
2. **Multi-file find-and-replace** — `write --find/--replace` is per-file; bulk refactors need a loop or sed/Bash.
3. **Process inspection or environment variables** — Explicitly excluded from 8v scope.
4. **Git operations** — Explicitly excluded.
5. **Error is unclear** — If `8v write` fails silently or with an opaque message, I'd fall back to Read/Edit to understand file state.
6. **Binary or non-text files** — No mention of how 8v handles them.

---

### Q16: Most and least used commands

**Most used:** `8v read` (symbol maps are the entry point for every file touch), `8v search` (finding relevant code before editing), `8v write` (making the actual changes), `8v check .` (verifying changes).

**Least used:** `8v fmt .` (probably run once at end, not per-edit), `8v build .` (subsumed by `check` in many workflows unless compilation is the blocker).

---

### Q17: First command in a new repo

`8v ls --tree --loc` — stated as "Start here." in the instructions. It gives the full hierarchy and line counts, which tells me what stacks exist, which files are large, and where to focus reading effort before any searching.

---

### Q18: Trust in write error messages

Instructions don't say — the only specific failure mode described for write is `--find "<old>"` failing when `<old>` is not found. For other write failures, no error format or guidance is given. My confidence that the error would be actionable: low. I would not fully trust the error to tell me exactly what to do next without also cross-checking file state with `8v read`.

---

### Q19: What's missing

1. **Error reference table** — What does each command return on failure? Exit codes, error format, stdout vs. stderr.
2. **Glossary** — "symbol map", "stack", "overhead" need one-line definitions.
3. **Multi-file write example** — How to do a refactor across N files.
4. **File-existence check** — A primitive to test whether a path exists before acting.
5. **`search` without a path** — The instructions show `8v search <pattern> [path]` but don't explicitly state the default search scope when `[path]` is omitted (presumably repo root / cwd).
6. **What `--json` returns for failures** — Is it `{"error": "..."}` or a non-JSON error?
7. **Line count vs. line number indexing** — For write, if a file has 50 lines and I write to line 60, what happens?

---

### Q20: Hesitation points when teaching

1. **Escape sequences in write** — Explaining when single quotes vs. double quotes are needed in the shell to prevent premature interpolation of `\n` and `\t`.
2. **Symbol map vs. full file for unfamiliar languages** — What symbols does Python or Go produce? The examples only show Rust (`fn`, `struct`, `impl`).
3. **Batch `--find/--replace`** — Teaching that this is per-file and showing the loop pattern.
4. **When to use `search --files` vs. `ls --match`** — Both can list files matching a pattern; the distinction is content-based vs. name-based, which is logical but easy to mix up.
5. **MCP vs. shell invocation** — The "if available, use MCP directly" instruction requires agents to know how to discover tools, which is itself a meta-capability.

---

### Q21: 8v vs. native tools

**Better:**
- Batch reads (N files in one call) — native tools require N separate Read calls.
- Symbol maps — native Read would require reading the whole file; 8v returns just the structure.
- Atomic write variants — single command handles replace, insert, delete, append; native requires Bash one-liners or sed.
- Verify pipeline — `8v check .` bundles lint + type + format in one invocation.

**Worse:**
- No file existence primitive — `test -f` is one Bash token; 8v has no equivalent.
- No multi-file `--find/--replace` — sed/Bash `find | xargs` handles this naturally.
- Escape syntax is non-obvious — `\n` as literal two chars is counterintuitive for shell-native users.
- No streaming output — verify commands presumably buffer until done; Bash pipelines can stream.

---

### Q22: Single highest-impact instruction edit

Add an error-behavior subsection under each command showing: (1) the exit code on success and failure, (2) what appears on stdout vs. stderr on failure, (3) the `--json` shape for errors. This one addition would eliminate the largest class of agent confusion: "the command failed, now what?"

---

### Q23: Overall clarity rating

**Axis 1 — Input clarity: 8/10**
The instructions do an excellent job showing exact syntax for every command variant, listing all flags with examples, and covering edge cases like multi-range batch reads and escape sequences. Minor deduction: flag discovery is deferred to `--help` rather than documented inline for less-common flags.

**Axis 2 — Output clarity: 6/10**
Read output is well-specified (symbol map format, batch `===` delimiter, `--json` shapes). But write, check, fmt, test, and build output formats are almost entirely unspecified — no example output, no description of what success looks like vs. failure on stdout/stderr.

**Axis 3 — Failure-mode clarity: 2/10**
Only one failure mode is explicitly described (`--find` fails if `<old>` not found). Exit codes for success/failure are not stated for most commands. Error message format, whether errors go to stdout or stderr, and behavior on missing paths are all gaps.

**Composite mean: (8 + 6 + 2) / 3 = 5.33**

---

### Q24: One-minute improvement

Add a single "Failure behavior" subsection after Verify:

> **Failure behavior:** All commands exit 0 on success, non-zero on failure. Error messages go to stderr. With `--json`, errors are returned as `{"error": "<message>"}` on stdout with a non-zero exit. If a path does not exist, `read` and `write` both exit non-zero with a `file not found` error on stderr.

This single paragraph would collapse the largest clarity gap.

---

### Q25: Output contract predictions

**a. `8v read util.py`**
Expected symbol map. Based on the format `<line-number>  <symbol>`:
```
1  def add
4  result
```
Uncertainty: whether `result = add(1, 2)` (a module-level assignment) counts as a symbol. The instructions say "functions, structs, classes" in other surfaces but don't define what Python symbols are included. Gap: symbol extraction rules for Python are not stated.

**b. `8v read util.py:1-2`**
Returns the raw content of lines 1–2 (1-indexed, end inclusive):
```
def add(a, b):
    return a + b
```
Confidence high — the range syntax and 1-indexed inclusive semantics are explicitly stated.

**c. `8v search "add" util.py`**
Default output groups matches by file. Predicted output:
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
Gap: the instructions don't specify whether the comment on line 1 (`# util.py  (4 lines)`) is treated as line 0 or whether comments are indexed — but treating `def add` as line 1 matches the fixture as written.

**d. `8v read util.py --full`**
Returns entire file content verbatim:
```
def add(a, b):
    return a + b

result = add(1, 2)
```
No header (single-file reads emit no header per the batch output contract). Instructions are explicit on this.

---

### Q26: `8v check .` exit codes and output destination

**Exit code when lint error found:** Non-zero. Stated: "Non-zero exit on any issue."
**Exit code on success:** 0 (implied by "non-zero on any issue").
**Where does error text appear?** — Gap. The instructions do not state whether output goes to stdout, stderr, or only appears in `--json` mode. Quote: "Non-zero exit on any issue" — nothing more is said about output streams.

---

### Q27: `8v write --find/--replace` edge cases

**If `<old>` appears zero times:** "fails if `<old>` not found" — the command fails. Exit code and error format are not specified.
**If `<old>` appears more than once:** Gap. The instructions say nothing about multiple matches. My guess: it replaces all occurrences (typical for find-and-replace semantics) but this is not stated.
**What is returned to the caller:** Gap. No description of success output for write commands.

---

### Q28: `8v read` on a non-existent path

Gap. The instructions do not describe this failure mode at all. No mention of error on stdout, stderr, non-zero exit code, or structured JSON error for missing paths. An agent would have no basis from the instructions alone to predict behavior.

---

### Q29: `8v check .` — no output, exit code 1

Based only on the instructions: "Non-zero exit on any issue." Exit code 1 with no stdout/no stderr output is a possible but unexplained outcome — the instructions do not state that output is guaranteed to accompany non-zero exit. This is a gap.

What should the agent do next? The instructions don't say. My inference: try `8v check . --json` to get structured output, or check if output was suppressed. But this is not instructed. The instructions provide no recovery guidance for this scenario.

Gap quote: The instructions state only "Non-zero exit on any issue" with no output destination, no guaranteed error message, and no recovery guidance.

---

### Q30: Surface 1 vs. Surface 2 differences

| Item | Surface 1 (ai_section.txt) | Surface 2 (instructions.txt) | More complete |
|------|---------------------------|-------------------------------|---------------|
| Write section header | `## Write` | `## Write — prefer targeted edits` | S2 adds directive, matters as behavioral nudge |
| Tool description intro | No intro paragraph | "8v — code reliability tool for AI agents. Designed to minimize round-trips." opening line | S2 has it; sets context immediately |
| Discovery section header | `## Discovery` | `## Discovery — learn the repo in one call` | S2 adds purpose clause |
| Batch output contract | Explicit: "Label is the relative path" spelled out in a separate bullet | Inline in the same line | S1 slightly more explicit formatting |
| `8v read a.rs b.rs c.rs` note | "One call beats N sequential calls" | Not present | S1 has this reinforcement |
| `8v fmt` description | "auto-format files in place. Idempotent." | Same | Identical |

**Impact:** An agent seeing only Surface 2 (MCP tool description) gets a slightly more compact but equally complete spec. The opening orientation sentence in S2 is a meaningful addition for first-impression context. No factual differences that would cause divergent behavior.

---

### Q31: Task 8v cannot complete alone

**Task:** Rename a file from `old_name.rs` to `new_name.rs` and update all import references across the project.

**Missing capability:** File renaming / moving. 8v has no `mv` or `rename` command. The write commands operate on existing file paths; there is no way to change a file's name or move it to a different path.

**Closest substitute:** Bash `mv old_name.rs new_name.rs`, then `8v search "old_name" --files` to find all files with references, then loop `8v write <file> --find "old_name" --replace "new_name"` for each.

**Cost:** Multi-step, requires shell tool for the rename primitive itself. Breaks the "use 8v for all file operations" rule — there is a genuine gap.

---

### Q32: Behavioral dry-run — 5-step Go task

**Step 1: Find all Go source files in the repo.**
Command: `8v ls --stack go`
Confidence: 4 — `--stack go` is explicitly listed as a valid value. Minor uncertainty: whether this returns individual `.go` files or project-level entries.

**Step 2: Search for all usages of `http.Get` across those Go files, with 2 lines of context.**
Command: `8v search "http\.Get" -e go -C 2`
Confidence: 5 — `-e <ext>` for extension filtering and `-C N` for context lines are both explicitly described. Regex dot must be escaped.

**Step 3: Read the symbol map of the file with the most matches.**
Command: `8v read <path-of-top-file>` (path determined from Step 2 output)
Confidence: 5 — Standard symbol map read; path comes from search output.

**Step 4: Replace the word `http.Get` with `httpClient.Get` on a specific line in that file.**
Command: `8v write <file>:<line> "    httpClient.Get(url)"` (replacing the full line) or `8v write <file> --find "http.Get" --replace "httpClient.Get"`
Confidence: 4 — Both forms work. `--find/--replace` is cleaner but behavior on multiple matches is unstated. Using line-replace requires knowing the exact line from Step 3.

**Step 5: Run the tests and confirm they pass.**
Command: `8v test .`
Confidence: 5 — Explicitly described. "Confirm they pass" requires checking exit code; instructions state non-zero on failure for verify commands.

No fallback needed for any step except a potential edge: if Step 1 returns project directories rather than individual files, Step 2's `-e go` handles the filtering directly anyway.

---

### Q33: Memorability — write syntax from memory

**Insert a new line before line 42 of `main.rs`:**
`8v write main.rs:42 --insert "new line content"`
Confidence: 5 — The `--insert` form is distinctive and I recall it clearly from reading.

**Replace lines 10–20 with multi-line content:**
`8v write main.rs:10-20 "line one\nline two\nline three"`
Confidence: 4 — The range syntax `<start>-<end>` and `\n` for newlines are both clearly stated. Minor uncertainty on whether the content argument requires any special shell quoting to prevent shell from consuming the `\n` before 8v sees it.

---

**Model & run ID:** Claude Sonnet 4.6 — Sonnet Run 2 — 2026-04-20

---

## 8v feedback

**Friction observed during this session:**

1. **No MCP tool invocation.** The task instructed to use "8v's two surfaces" as sources, but the actual file reads were done via native Read/Bash tools (Bash `cat` and Read tool), not via the `mcp__8v-debug__8v` MCP tool. The MCP tool was listed as a deferred tool but its schema was not pre-loaded, creating friction for the correct invocation path. If the intent was to use the MCP `8v read` command, the deferred schema loading step is a barrier.

2. **No `8v read` batch invocation of 3 source files.** The canonical workflow (`8v read file1 file2 file3`) was not followed because native tools were used. The batch instruction exists in the docs but the execution environment defaulted to native.

3. **Write target directory required a pre-check.** Used `mkdir -p` via Bash to ensure the output directory existed, then Write to create the file — this two-step pattern suggests 8v write has no equivalent to `--mkdir-p` for auto-creating parent directories.
