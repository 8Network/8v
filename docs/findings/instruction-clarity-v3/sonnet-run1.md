# Instruction Clarity v3 — Sonnet Run 1 — 2026-04-19
Model: Claude Sonnet 4.6
Surfaces read: ai_section.txt, instructions.txt

---

### Q1: Summary

8v is a code reliability tool for AI agents that replaces native file tools (Read, Edit, Write, Grep, Glob) with a single CLI that minimizes round-trips. Use it for all file operations — discovery, reading, writing, searching, and verifying code. It operates on two principles: Progressive (default output is minimal, flags escalate detail) and Batch (pass many inputs in one call to amortize per-call overhead).

---

### Q2: Minimum-viable invocations

- `ls`: `8v ls --tree --loc` — returns full file hierarchy with line counts
- `read`: `8v read <path>` — returns symbol map (function/struct/class names with line numbers)
- `search`: `8v search <pattern>` — returns matches grouped by file as `<path>:<line>:<text>`
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line in-place, no described return value
- `check`: `8v check .` — runs lint + type-check + format-check, non-zero exit on any issue
- `fmt`: `8v fmt .` — auto-formats files in place, idempotent, no described return value
- `test`: `8v test .` — runs project tests, no described return value
- `build`: `8v build .` — compiles the project, no described return value

---

### Q3: The two principles

**Progressive**: Default output is the minimum useful answer; flags escalate detail. Concrete non-obvious example: `8v read foo.rs` returns only a symbol map (line numbers + names), not the file body — you must request a range like `foo.rs:12-50` to see actual code. This prevents token waste on large files when you only need to orient yourself.

**Batch**: Pass many inputs in one call to amortize per-call overhead. Concrete non-obvious example: You can mix file paths AND line ranges of the same file in a single call: `8v read big.rs:1-200 big.rs:400-600 Cargo.toml` — three reads, one call, one schema/overhead cost.

---

### Q4: When to use 8v vs. native tools

**Use 8v** for: reading files, writing/editing files, searching file contents, discovering repo structure, running lint/format/test/build.

**Use native tools (Bash/shell)** for: git operations, process management, environment operations. Surface 1 explicitly names these: "Use Bash only for git, process management, and environment operations."

**Do NOT use 8v** for anything that is not a file operation — network calls, process spawning, system queries, etc. The instructions don't address those at all.

---

### Q5: How to discover flags

Run `8v <cmd> --help`. Both surfaces state: "Run `8v <cmd> --help` for the full flag list."

---

### Q6: Ambiguous phrases

1. **"Use Bash only for git, process management, and environment operations"** — "process management" could mean (a) starting/stopping system services, or (b) running any subprocess/command. Reading (b) would mean Bash is fine for running arbitrary commands, undermining 8v's scope.

2. **"Each call costs overhead"** / **"costs schema tax"** (MCP surface uses "schema tax", CLAUDE.md uses "overhead") — "overhead" is unquantified. It could mean (a) network latency, (b) token cost from schema inclusion, or (c) both. The MCP surface says "schema tax" more specifically, implying token cost.

3. **"Last resort"** (for `--full`) — could mean (a) literally never use it except when nothing else works, or (b) prefer symbol map + ranges but --full is acceptable when range is unknown.

4. **"Fails if `<old>` not found"** — "fails" could mean (a) non-zero exit code, (b) a printed error message, (c) both, or (d) silent no-op. The word "fails" is used without specifying the failure mode.

5. **"Default output groups matches by file"** — does "groups" mean (a) sorted by file with a header per file, or (b) just that all matches from the same file appear consecutively?

6. **"All verify commands... run on the whole project by default. Pass a path to scope to a subtree."** — unclear whether "whole project" means the current working directory, the nearest project root, or the path passed to `8v`. Ambiguous when called from a subdirectory.

---

### Q7: Implied but never stated behaviors

- **Exit codes for success**: `check` documents non-zero on error but never states exit 0 on success explicitly. Test/build/fmt exit codes not stated.
- **Error message format**: Where errors appear (stdout vs. stderr) is never stated.
- **JSON shape**: `--json` is offered everywhere but the schema for each command's JSON output is never described.
- **What happens when path doesn't exist**: No behavior described for any command.
- **Output interleaving in batch**: Not stated — does `8v read a.rs b.rs` output a.rs's map then b.rs's map, or interleaved?
- **Write return value**: Success confirmation, the new line count, or nothing — never stated.
- **Shell quoting of content strings**: Instructions say "do not rely on shell interpolation" but don't explain how to pass content with embedded quotes.
- **Unicode handling**: Not mentioned.
- **Encoding assumptions**: Not mentioned.
- **Whether `--json` changes exit codes**: Not stated.
- **Whether `8v fmt` modifies files in place silently or reports changes**: "idempotent" is stated but not whether it reports what changed.
- **Whether `8v test` and `8v build` pass through underlying tool output**: Not stated.
- **Concurrency / atomicity of writes**: Not stated.

---

### Q8: Undefined terms

- **"symbol map"**: Used but never formally defined. The example output shows `<line-number>  <symbol>` with function/struct names, but which constructs count as symbols across languages is not explained.
- **"stack"**: Used in `--stack <name>` filter without defining what a stack is (presumably a language/framework grouping).
- **"progressive"**: Used as a principle name; partially explained by contrast but the formal definition is implicit.
- **"overhead"** / **"schema tax"**: Two surfaces use different terms for the same concept; neither quantifies it.
- **"compact mode"**: Not mentioned in either surface.
- **"project"**: Used in "run on the whole project by default" without defining what constitutes project boundaries.
- **"symbol"**: What counts as a symbol in non-Rust languages is not defined.

---

### Q9: Contradictions between surfaces

No direct contradictions in content. Differences (see Q30) are omissions and wording variations, not logical contradictions. Both surfaces describe the same commands with the same syntax. The only tension is that Surface 1 says "Use Bash only for git, process management, and **environment** operations" while Surface 2 says "Use shell tools only for git, process management, and **environment** operations" — identical in substance.

One wording difference: Surface 1 uses "overhead" for the cost per call; Surface 2 uses "schema tax" (only in the MCP tool description title area). Within Surface 2 body it uses "overhead." No logical contradiction, but inconsistent vocabulary.

---

### Q10: Batch output interleaving

The instructions say "batch any combination of paths and ranges in one call" but never describe the output format when multiple files are passed. The example symbol map output shows a single-file format. From the text alone it's impossible to determine whether the outputs are concatenated with file headers, interleaved, or separated. **The instructions do not say.**

---

### Q11: Trailing newlines and multi-line content in write

**Trailing newline**: Not stated. The instructions do not say whether a trailing newline is automatically appended.

**Multi-line content**: Addressed explicitly — `\n` in the content string becomes a newline. Example: to replace a range with 3-line content, pass the content with `\n` separators.

**Shell quotes**: The instructions say: "Content arguments are parsed by 8v (not the shell): `\n` becomes a newline... Pass them as literal two-character sequences — do not rely on shell interpolation." This means the shell should NOT expand the backslash sequences; they should be passed as-is to 8v.

---

### Q12: Scenario commands + confidence

**a. Read 5 files at once.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs`
Confidence: 5 — explicitly shown in both surfaces.

**b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.**
Command: `8v write foo.rs:10-20 "line1\nline2\nline3"`
Confidence: 4 — range-replace and `\n` for multi-line are both documented; unclear if this replaces the exact range or inserts.

**c. Find all functions named `handle_*` across the repo.**
Command: `8v search "handle_" --files` or `8v search "fn handle_"` (for Rust)
Confidence: 3 — `search` supports regex patterns but the instruction for search shows `<pattern>` without language-aware function search. `--files` returns only paths not the function names themselves.

**d. Append one line to `notes.md`.**
Command: `8v write notes.md --append "new line content"`
Confidence: 5 — explicitly documented.

**e. Symbol map for `bar.rs`, then read lines 100–150.**
Commands: `8v read bar.rs` then `8v read bar.rs:100-150`
Confidence: 5 — canonical use case, both documented explicitly.

**f. Run tests and parse JSON output.**
Command: `8v test . --json`
Confidence: 3 — `--json` flag is documented as accepted by all commands, but JSON schema for test output is not described.

**g. Check whether a file exists before reading.**
Command: Instructions don't say — my guess: there is no dedicated existence check. I would use `8v read <path>` and see if it returns an error. Confidence: 2 — no file-existence primitive described.

**h. Delete lines 50–60.**
Command: `8v write foo.rs:50-60 --delete`
Confidence: 5 — explicitly documented.

**i. Insert a new line before line 30.**
Command: `8v write foo.rs:30 --insert "new line content"`
Confidence: 5 — explicitly documented.

**j. Search only Rust files, case-insensitive, for `TODO`.**
Command: `8v search "TODO" -i -e rs`
Confidence: 4 — `-i` and `-e <ext>` are both documented in the search flags.

**k. Find all files by name matching `*_test*.md`.**
Command: `8v ls --match "*_test*.md"`
Confidence: 4 — `--match <glob>` is documented for `ls`.

**l. Run lint + format-check + type-check with one command.**
Command: `8v check .`
Confidence: 5 — explicitly described as "lint + type-check + format-check."

**m. Replace `old_name` with `new_name` across a multi-file refactor.**
Command: `8v write file1.rs --find "old_name" --replace "new_name"` (repeated per file — no cross-file flag described)
Confidence: 2 — `--find/--replace` is documented but only for a single file path. No multi-file variant described. Would need to enumerate files from `8v ls` and loop.

**n. Read just the symbols of 10 files in one call.**
Command: `8v read f1.rs f2.rs f3.rs f4.rs f5.rs f6.rs f7.rs f8.rs f9.rs f10.rs`
Confidence: 5 — default `read` returns symbol map; batch is documented.

**o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.**
Command: `8v read big.rs:1-200 big.rs:500-600`
Confidence: 5 — explicitly demonstrated: "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)."

---

### Q13: How each command is taught

| Command | Taught by |
|---------|-----------|
| `ls` | Example (`8v ls --tree --loc`) + description |
| `read` | Example (symbol map output shown) + description |
| `search` | Example (output format shown) + description |
| `write` | Description-only (no full example showing before/after state) |
| `check` | Description-only |
| `fmt` | Description-only |
| `test` | Description-only |
| `build` | Description-only |

---

### Q14: Three most likely mistakes

1. **Shell-expanding `\n` in write content.** The instructions warn against this but a shell like bash will expand `\n` inside double quotes depending on quoting style. An agent might write `"line1\nline2"` and have the shell eat the backslash, resulting in `line1nline2`. The warning is there but easy to violate without knowing the exact quoting behavior needed.

2. **Using `8v write --find/--replace` expecting multi-file scope.** The command looks like a refactor tool but only operates on a single file. An agent trying to rename a symbol repo-wide would silently do only one file if they don't read carefully — or would have to loop, which requires knowing `ls` output format.

3. **Calling `8v read <path> --full` prematurely.** The "last resort" label means symbol map first, then range. An agent under time pressure might skip the symbol-map step and go straight to `--full`, wasting tokens on large files — violating the Progressive principle without a hard error.

---

### Q15: When I would fall back to native tools

- **File existence check**: No `8v` primitive described; I'd use `test -f <path>` in Bash.
- **Directory creation**: No `8v mkdir` described; I'd use Bash.
- **Environment variable inspection**: Explicitly scoped to Bash.
- **Git operations**: Explicitly scoped to Bash.
- **Cross-file find+replace**: `--find/--replace` is single-file only; I'd use `8v ls` to enumerate files and loop with Bash.
- **If `8v` returns an error I can't interpret**: With no error format docs, I'd fall back to native Read to verify the file state.

---

### Q16: Most/least used commands

**Most used**: `read` (every code task starts with orientation), `write` (every edit), `search` (cross-file navigation), `check` (verification gate).

**Least used**: `build` (usually implied by `check` or `test`), `fmt` (called once, idempotent, then rarely needed again during a session).

---

### Q17: First command in a new repo

`8v ls --tree --loc` — it gives the full file hierarchy with line counts in one call, satisfying the "start here" instruction and enabling all subsequent targeted reads without needing to enumerate directories.

---

### Q18: Trust `8v write` error messages

Partially. The instructions say `--find/--replace` "fails if `<old>` not found" — so at least one failure mode produces an error. But the error format, exit code, and whether the error goes to stdout or stderr is not described. I would trust that a failure signal exists but would not know whether to parse stderr vs. stdout or what structure to expect. Confidence in acting on the error: low.

---

### Q19: What's missing

- **Error format specification**: Where do errors go (stdout/stderr)? What is the structure? JSON shape when `--json` is passed on error?
- **Glossary**: "symbol map," "stack," "project," and "symbol" are all undefined.
- **Success output for write commands**: What does a successful `8v write` return to the caller?
- **Multi-file `--find/--replace`**: The most common refactor use case has no solution beyond manual looping.
- **File existence / stat**: No way to check if a file exists or get metadata.
- **JSON schemas**: `--json` is advertised for every command but no output schema is shown.
- **Language-specific symbol detection**: Which constructs are "symbols" in Python, Go, JS, etc.?
- **Exit codes table**: Stated for `check` only; missing for all other commands.

---

### Q20: Where I would hesitate while teaching

1. **"What does `8v write` return?"** — I don't know. I'd have to guess "nothing / silent on success."
2. **"How do I do a project-wide rename?"** — The tool doesn't directly support it; I'd have to explain a loop pattern using `ls` + repeated `write --find --replace`.
3. **"What is a symbol map for a Python file?"** — I can explain the concept from the Rust examples but cannot confidently describe Python behavior.
4. **"What happens if the path is wrong?"** — I don't know the error behavior and couldn't tell a student what to look for.

---

### Q21: 8v vs. native tools

**8v does better:**
- Batch reads: one call for N files vs. N separate Read calls
- Symbol maps: native Read has no equivalent; you'd have to grep for function definitions
- Progressive detail: you get exactly what you need without reading the full file
- Token efficiency: designed for the constraint native tools ignore

**8v does worse (or is unclear):**
- No file-existence check
- No cross-file atomic rename
- Error handling is opaque compared to native tools where exit codes and stderr are standard
- No metadata (file size, modification time, permissions)
- JSON output shape is undocumented, so `--json` mode is hard to use programmatically

---

### Q22: Single instruction edit with biggest positive impact

Add a one-paragraph **"Errors" section** that specifies: (1) errors go to stderr, (2) exit code 0 = success / non-zero = failure, (3) the JSON error shape when `--json` is passed. This would make the failure paths predictable and eliminate the largest category of ambiguity.

---

### Q23: Three-axis clarity scores

**Axis 1 — Input clarity: 8/10**
The instructions are precise about what to pass in for every documented command — path syntax, flag syntax, range syntax, and the `\n` escaping rule are all clear. Point deductions: `--json` JSON schema not shown, and what constitutes "whole project" scope is undefined.

**Axis 2 — Output clarity: 4/10**
Only `read` (symbol map format), `ls` (hierarchy with line counts), and `search` (path:line:text format) have described output shapes. Write, check, fmt, test, and build outputs are not described. The symbol map example is helpful but limited to Rust; multi-language output is unknown.

**Axis 3 — Failure-mode clarity: 2/10**
Only one failure mode is explicitly documented: `--find/--replace` fails if `<old>` not found. All other failures — bad path, missing file, syntax error in pattern, permission error — are completely undescribed. Error destination (stdout/stderr), structure, and exit codes are absent.

**Composite mean: (8 + 4 + 2) / 3 = 4.67**

---

### Q24: One-minute improvement

Add three lines at the end of the Write section:

> On success: silent (exit 0). On failure: error message on stderr, exit non-zero. With `--json`: `{"ok": false, "error": "<message>"}`.

This single addition would resolve the largest gap (failure mode opacity) without adding significant length.

---

### Q25: Predicted output for util.py

**a. `8v read util.py`**
The instructions describe symbol map output as `<line-number>  <symbol>`. For Python, I predict:
```
2  def add
```
(Line 4 `result = ...` is a module-level assignment, not a function — whether it appears as a symbol is unknown. The instructions only give Rust struct/fn examples.)
Gap: Python symbol detection behavior is not specified. Confidence the format is right but content coverage is uncertain.

**b. `8v read util.py:1-2`**
```
# util.py  (4 lines)
def add(a, b):
```
(Lines 1 and 2 of the file as shown. The instructions say line ranges are 1-indexed and end-inclusive.)

**c. `8v search "add" util.py`**
```
util.py:2:def add(a, b):
util.py:4:result = add(1, 2)
```
(Default output is `<path>:<line>:<text>` for each matching line. "add" appears on lines 2 and 4.)

**d. `8v read util.py --full`**
```
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```
(Entire file content. The instructions say `--full` returns the entire file.)

---

### Q26: `8v check .` exit codes and error output location

**Exit code on lint error**: Non-zero (the instructions say "Non-zero exit on any issue"). Specific code not stated.
**Exit code on success**: Instructions don't say — my guess: 0 (standard Unix convention, but not stated).
**Error text location**: Gap — the instructions do not specify whether error output goes to stdout, stderr, or only in `--json`. Quote: "Non-zero exit on any issue" is all that is stated.

---

### Q27: `8v write --find "<old>" --replace "<new>"` edge cases

**`<old>` appears zero times**: "fails if `<old>` not found" — exact quote. This is one of the few explicit failure modes.

**`<old>` appears more than once**: Instructions don't say — my guess: probably replaces all occurrences (standard behavior for find+replace), but the instructions are silent. This is a gap.

**What is returned to the caller**: Gap — success and failure return values are not described.

---

### Q28: `8v read <path>` on non-existent path

Gap — the instructions do not describe this behavior at all. No quote applies. My guess: error on stderr, non-zero exit, possibly `{"ok": false, "error": "path not found"}` with `--json`. But there is no instruction text to support this.

---

### Q29: `8v check .` — no stdout, no stderr, exit 1

Based only on the instructions: "Non-zero exit on any issue" — so exit code 1 is within the documented behavior for when an issue is found. However, the instructions say nothing about the case where exit is non-zero but output is absent. This scenario is not addressed.

**Is this expected behavior?** Partially — exit 1 is expected when issues exist, but silent exit 1 (no output) is not described as possible behavior.

**What should the agent do next?** Instructions don't say — my guess: retry with `--json` to get structured output, or inspect the files manually. No recovery path is documented.

Relevant instruction text: "Non-zero exit on any issue." That's the entirety of what applies.

---

### Q30: Factual differences between Surface 1 and Surface 2

| Item | Surface 1 (ai_section.txt) | Surface 2 (instructions.txt) | More complete |
|------|---------------------------|------------------------------|---------------|
| Opening scope statement | More detailed: "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations." Names specific tools. | Shorter: "Use `8v` for all file operations (read, edit, write, search, inspect)." | Surface 1 — naming the tools being replaced is more actionable. |
| `--stack` valid values | Listed explicitly in bullet form | Listed on same line inline | Surface 1 — easier to scan. |
| `search` output format | Documented inline after the command | Documented on separate line below | Functionally identical. |
| `read` batch note | Includes: "One call beats N sequential calls." | Does not include that sentence. | Surface 1 — reinforces Batch principle. |
| `write` content escaping note | Full paragraph: "Content arguments are parsed by 8v (not the shell)..." | Same text present. | Identical. |
| Write section header | `## Write` | `## Write — prefer targeted edits` | Surface 2 — the subtitle "prefer targeted edits" adds behavioral guidance absent in Surface 1. |
| Typical flow line | Present at bottom | Present at bottom | Identical. |

**Key agent impact**: An agent that only sees Surface 2 (MCP description) would not see the explicit list of tools being replaced (Read, Edit, Grep, Glob) and might not fully understand what 8v supplants.

---

### Q31: Task 8v cannot complete

**Task**: Rename a directory. Example: rename `src/handlers/` to `src/controllers/` across the repo, updating all import paths.

**Missing capability**: No `8v mv`, `8v rename`, or directory-operation command exists. `--find/--replace` works on file contents but cannot rename file paths or directories.

**Closest substitute**: Bash `mv` for the directory rename, then `8v ls --match "*.rs"` to enumerate files, then `8v write <file> --find "handlers" --replace "controllers"` per file. Cost: the loop must be done outside 8v (Bash), breaking the single-tool discipline; and `--find/--replace` must be applied file-by-file, which the instructions don't provide a batch variant for.

---

### Q32: Behavioral dry-run — 5-step Go task

**Step 1: Find all Go source files in the repo.**
Command: `8v ls --stack go`
Confidence: 4 — `--stack go` is listed as a valid stack value. Returns a filtered view of Go files. Not certain if it returns file paths or a tree.

**Step 2: Search for all usages of `http.Get` across those files, with 2 lines of context.**
Command: `8v search "http\.Get" -e go -C 2`
Confidence: 4 — `-e <ext>` for Go files and `-C N` for context lines are both documented. Regex needs the dot escaped.

**Step 3: Read the symbol map of the file with the most matches.**
Prerequisite: I need to identify which file had the most matches from Step 2 output (count occurrences per path from `<path>:<line>:<text>` output).
Command: `8v read <that-file.go>`
Confidence: 5 for the read itself; Confidence: 2 for counting matches — no `--count` or aggregation flag described. I would have to count manually from Step 2 output.
**No fallback needed for the read; fallback to manual counting needed for "most matches" determination.**

**Step 4: Replace `http.Get` with `httpClient.Get` on a specific line in that file.**
Command: `8v write <file.go>:<line-number> "    httpClient.Get(url)"` (with actual line content)
Or: `8v write <file.go>:<line>-<line> "    httpClient.Get(url)"`
Confidence: 4 — single-line replace is documented. I'd need the exact line content from Step 3 to construct the correct replacement.
Note: Using `--find "http.Get" --replace "httpClient.Get"` would replace ALL occurrences in the file, which may or may not be desired. For a specific line, the line-number form is safer.

**Step 5: Run the tests and confirm they pass.**
Command: `8v test .`
Confidence: 3 — command is documented but how to confirm "pass" vs. "fail" from output is not described. With `--json` the shape is undocumented, so parsing is a guess.

---

### Q33: Memorability — write syntax from memory

**Insert a new line before line 42 of `main.rs`:**
```
8v write main.rs:42 --insert "new line content"
```
Confidence: 5 — the pattern `<path>:<line> --insert "<content>"` is directly stated and distinctive enough to remember.

**Replace lines 10–20 with multi-line content:**
```
8v write main.rs:10-20 "line one\nline two\nline three"
```
Confidence: 4 — the range syntax `<path>:<start>-<end>` is consistent with read range syntax, making it memorable. The `\n` for newlines is stated. Minor uncertainty: whether the range is inclusive of line 20 (yes, it is — matching the read convention of "end inclusive") and whether the old content count matters vs. new content count.

---

## 8v feedback

### Commands used
1. `8v read <path1> <path2> <path3> --full` (batch read of three files in one call)

### What went well
The batch read worked exactly as documented — all three files returned in one call with file headers separating them. The `--full` flag correctly returned complete file contents. The MCP tool schema loaded cleanly via ToolSearch.

### Friction observed
1. **ToolSearch required before calling the MCP tool.** The 8v MCP tool (`mcp__8v-debug__8v`) is a deferred tool, requiring a ToolSearch lookup before it can be called. This is a Claude agent harness behavior, not an 8v behavior, but it adds one extra round-trip at the start of every session. Not a bug in 8v itself.

2. **No `8v write --append` equivalent through the MCP for new file creation with multi-KB content.** For writing a large new file, I had to use the native Write tool instead of `8v write --append` because appending to a non-existent file's behavior is not documented (does it create the file?). I used Bash `mkdir -p` for directory creation (allowed) and native Write for the file. If `8v write --append` creates files on first call, that should be stated explicitly.

3. **No `8v touch` or file-creation primitive.** When creating a new file (this output file), the instructions offer no `8v` path — only `--append`. Whether `--append` to a non-existent path creates it is a gap (see Q19).

4. **The tool name is `mcp__8v-debug__8v` not `mcp__8v__8v`.** The `-debug` infix is unexpected and would cause an agent to fail to find the tool on first attempt without the ToolSearch step. Not documented anywhere in the instruction surfaces.

Overall: 8v worked smoothly for the batch read task. The main friction was in the write step where I had to fall back to native tools due to the file-creation ambiguity.
