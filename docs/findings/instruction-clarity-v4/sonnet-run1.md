# Instruction Clarity v4 — Sonnet Run 1 — 2026-04-20
Model: Claude Sonnet 4.6
Surfaces read: ai_section.txt, instructions.txt

---

## A. Understanding

### Q1: summary
8v is a code-reliability tool for AI agents that replaces native file operation tools (Read, Edit, Write, Grep, Glob, Bash) with a single unified CLI. It should be used for all file reads, edits, searches, and inspections; shell tools remain only for git, process management, and environment operations. Two governing principles: Progressive (default output is minimum useful, flags escalate detail) and Batch (pass many inputs in one call to amortize per-call overhead).

### Q2: minimum-viable invocations
- `ls`: `8v ls --tree --loc` — returns full file hierarchy with line counts
- `read`: `8v read <path>` — returns a symbol map (function/struct/class definitions with line numbers)
- `search`: `8v search <pattern>` — returns matches grouped by file as `<path>:<line>:<text>`
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line, returns (implicitly) success/failure
- `check`: `8v check .` — runs lint + type-check + format-check; non-zero exit on any issue
- `fmt`: `8v fmt .` — auto-formats files in place; returns (implicitly) success/failure
- `test`: `8v test .` — runs project tests; returns (implicitly) pass/fail
- `build`: `8v build .` — compiles the project; returns (implicitly) success/failure

### Q3: two principles
**Progressive**: Default output is the minimum useful answer; you escalate with flags. Non-obvious example: `8v read file.rs` gives only a symbol map — you do NOT get any line of code unless you follow up with a range or `--full`. This prevents token waste on large files when you only need to know what functions exist.

**Batch**: Pass multiple inputs in a single call rather than calling N times. Non-obvious example: `8v read a.rs:1-200 a.rs:200-400` reads two disjoint ranges of the same file in one call — most tools require two sequential calls for this.

### Q4: when to use 8v vs. not
Use 8v for: reading files, writing files, searching content, discovering structure, listing files, linting, formatting, testing, building. Do NOT use 8v for: git commands, process management, environment variable operations. Those remain in Bash/shell.

### Q5: how to discover flags
Run `8v <cmd> --help` — stated explicitly: "Every command accepts `--json`. Run `8v <cmd> --help` for the full flag list."

---

## B. Ambiguity — quote it, show dual reading

### Q6: ambiguous phrases
1. **"Use Bash only for git, process management, and environment operations"** — "environment operations" could mean (a) reading/setting env vars, or (b) any OS-level operation not covered by 8v (e.g., network calls, file permissions).
2. **"Default output is the minimum useful answer"** — "minimum useful" is subjective. For `ls`, is that one line or a full tree? The docs clarify per-command but the principle statement itself is vague.
3. **"Each call costs overhead — amortize it"** — "overhead" is undefined. Could mean (a) latency/round-trip time, (b) token cost in MCP context, (c) both.
4. **"`--full` applies to every positional arg (repeats accepted, no-op beyond the first)"** — "repeats accepted, no-op beyond the first" could mean (a) `--full --full` is allowed and the second is ignored, or (b) passing the same file path twice, the second occurrence is a no-op.
5. **"Pass them as literal two-character sequences"** — for `\n`, "literal two-character sequence" could mean (a) type backslash then n in the shell without quoting, or (b) put the literal characters `\n` inside the quoted string argument.
6. **"Fails if `<old>` not found"** — "fails" could mean (a) non-zero exit code, (b) prints an error message, (c) both, (d) aborts without writing.

### Q7: implied but never stated behaviors
- What exit code does a successful command return? (0 is implied but never stated)
- What does error output look like — stderr vs. stdout? Neither surface says.
- What happens when a path does not exist in `8v read`? No description.
- What happens when `8v write` is given a line number beyond the file length? No description.
- Output encoding — UTF-8 assumed but never stated.
- Whether `8v search` respects `.gitignore` — stated in one surface's CLAUDE.md system reminder but not in ai_section.txt or instructions.txt.
- What the symbol map contains for languages that don't have clear symbols (e.g., a plain text or JSON file).
- Whether batch `read` output streams or buffers.
- What `--json` shape looks like for `write`, `check`, `fmt`, `test`, `build` — only `read` JSON shape is documented.
- Whether `8v fmt .` modifies files in place and what it returns to the caller.
- What happens on partial batch failure (e.g., 3 of 5 files exist in a batch read).

### Q8: undefined terms
- **"symbol map"** — used without a definition of what qualifies as a "symbol" in different languages. Example output given only for Rust-style syntax.
- **"stack"** — implies language/framework detection, but how detection works is never described.
- **"progressive"** — defined as a principle but its specific meaning per command (how much detail is "minimum useful") is not uniformly documented across all commands.
- **"overhead"** — used to justify Batch principle; not defined (latency? tokens? both?).
- **"compact mode"** — mentioned in a system reminder CLAUDE.md (`Omitting -C uses compact mode`) but not in the two tested surfaces.
- **"schema tax"** — not present in either tested surface.

### Q9: contradictions between surfaces
Surface 1 (ai_section.txt) and Surface 2 (instructions.txt) are nearly identical in content. One minor difference: Surface 2 (instructions.txt) omits the explicit sentence "One call beats N sequential calls" after the batch read example, while Surface 1 includes it. Not a contradiction, but Surface 1 is slightly more explicit in reinforcing the Batch principle. No factual contradictions found between the two tested surfaces.

### Q10: batch output interleaving
From the text, batch output concatenates with separators — it does not interleave. The contract states: "each file is preceded by `=== <label> ===` on its own line." So the output is sequential per file, not interleaved. You know this because the `===` delimiter contract is explicit and implies sequential concatenation.

### Q11: trailing newline and multi-line write
The instructions do not state whether a trailing newline is added automatically. Multi-line content is written by embedding `\n` as literal two-character sequences inside the content string: `8v write foo.rs:10-20 "line1\nline2\nline3"`. The surrounding quotes have shell meaning (grouping), but 8v itself parses the escape sequences — the instruction says "do not rely on shell interpolation" and "pass them as literal two-character sequences."

---

## C. Concrete commands + confidence

### Q12: scenarios

**a. Read 5 files at once.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs`
Confidence: 5 — explicitly documented with example.

**b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.**
Command: `8v write foo.rs:10-20 "line1\nline2\nline3"`
Confidence: 4 — range replace is documented; multi-line via `\n` is documented. Minor uncertainty on whether replacing 11 lines with 3 lines works correctly (no explicit statement about differing line counts).

**c. Find all functions named `handle_*` across the repo.**
Command: `8v search "handle_" -C 0`
Confidence: 3 — regex is supported, but the instructions show `<pattern> (regex)` without clarifying whether glob-style `*` or regex `.*` is the right syntax. I'd use `8v search "handle_"` (regex prefix match).

**d. Append one line to `notes.md`.**
Command: `8v write notes.md --append "my new line"`
Confidence: 5 — `--append` is explicitly documented.

**e. Symbol map for `bar.rs`, then read lines 100–150.**
Commands: `8v read bar.rs` then `8v read bar.rs:100-150`
Confidence: 5 — the canonical workflow is stated explicitly.

**f. Run tests and parse JSON output.**
Command: `8v test . --json`
Confidence: 4 — `--json` flag is stated as available on all verify commands; JSON shape for test output is not documented so parsing is uncertain.

**g. Check whether a file exists before reading.**
Command: Instructions don't say — my guess: there is no `8v exists` command. I'd use `8v read <path>` and check for error output. Confidence: 2 — no file-existence command is documented.

**h. Delete lines 50–60.**
Command: `8v write <path>:50-60 --delete`
Confidence: 5 — explicitly documented.

**i. Insert a new line before line 30.**
Command: `8v write <path>:30 --insert "new content"`
Confidence: 5 — explicitly documented.

**j. Search only Rust files, case-insensitive, for `TODO`.**
Command: `8v search "TODO" -i -e rs`
Confidence: 5 — both `-i` and `-e <ext>` flags are documented.

**k. Find all files by name matching `*_test*.md`.**
Command: `8v ls --match "*_test*.md"`
Confidence: 4 — `--match <glob>` is documented for `ls`; this is the intended use case.

**l. Run lint + format-check + type-check with one command.**
Command: `8v check .`
Confidence: 5 — explicitly stated: "lint + type-check + format-check."

**m. Replace `old_name` with `new_name` across a multi-file refactor.**
Command: No single 8v command handles multi-file find-replace. I'd run `8v write <file> --find "old_name" --replace "new_name"` per file after finding them with `8v search "old_name" --files`. Confidence: 3 — `--find/--replace` is documented as single-file only; multi-file requires iteration.

**n. Read just the symbols of 10 files in one call.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs f.rs g.rs h.rs i.rs j.rs`
Confidence: 5 — batch read without `--full` returns symbol maps; this is the default.

**o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.**
Command: `8v read big.rs:1-200 big.rs:500-600`
Confidence: 5 — explicitly documented: "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)."

### Q13: teaching method per command
- `ls`: example + description
- `read`: example + description (most detailed coverage)
- `search`: description-only (flags listed but no example of output with content shown)
- `write`: description-only (no example of actual before/after output)
- `check`: description-only
- `fmt`: description-only
- `test`: description-only
- `build`: description-only

---

## D. Behavioral prediction

### Q14: three likely mistakes
1. **Wrong escape handling for multi-line write**: I would likely write `8v write foo.rs:5 "line1\nline2"` and accidentally let the shell interpret `\n` as a newline before 8v sees it, violating the "literal two-character sequence" requirement. The instruction says not to rely on shell interpolation but doesn't give a concrete shell-safe quoting example.
2. **Using `8v search` with `*` glob syntax instead of regex**: The instruction says `(regex)` but gives no pattern examples. I might try `handle_*` instead of `handle_.*` and get no results or wrong results.
3. **Omitting `===` header parsing in batch read**: When batch-reading 5 files, I might not correctly split the output on `=== <label> ===` separators, especially if a file's content contains lines that look like that pattern. The instruction doesn't say whether the delimiter is escaped or unique.

### Q15: what causes fallback to Bash/native tools
- Checking whether a file exists before reading (no `8v exists` command)
- Multi-file find-replace in a refactor (requires iteration or scripting)
- Any operation that needs process lifecycle (e.g., killing a running server, checking ports)
- Git operations (explicitly excluded)
- Getting environment variables or system info
- When `8v` is not available as MCP and I don't know the binary path

### Q16: most vs. least used commands
Most used: `read` (symbol map + ranges), `search`, `write` — core coding loop.
Least used: `fmt` (usually run once at end), `build` (only needed to confirm compilation), `ls` (once per session for orientation).

### Q17: first command in a new repo
`8v ls --tree --loc` — explicitly recommended as "Start here." It gives the full file hierarchy with line counts, enabling informed decisions about what to read next.

### Q18: trust `8v write` errors
Partially. The only documented failure behavior is: "`--find`/`--replace` fails if `<old>` not found." For all other failure modes (bad line number, permission error, disk full, path missing), the instructions give no error format description. I would look at the error output but would not know whether it comes on stdout, stderr, as structured JSON, or as a human-readable message.

---

## E. Missing / wished

### Q19: what's missing
- **Error format documentation**: What does an error look like — exit code, stderr message format, JSON shape on failure?
- **`--json` output shapes for write/check/fmt/test/build** — only `read` JSON is documented.
- **File-existence check command** — no way to test existence without attempting a read.
- **Glossary**: "symbol map," "stack," "progressive" all need 1-sentence definitions.
- **Shell quoting example**: A concrete example of how to pass multi-line content without shell interpolation eating the escapes (e.g., using `$'...'` syntax or single quotes).
- **`8v search` output example**: The format `<path>:<line>:<text>` is stated, but no concrete example is shown.
- **Partial-failure behavior for batch reads**: What if 2 of 5 files don't exist?

### Q20: where I'd hesitate when teaching
- Explaining how to write multi-line content safely (shell quoting is a landmine)
- Explaining what "symbol" means for Python, JSON, Markdown vs. Rust
- Explaining batch `--json` output structure for non-read commands
- Explaining the difference between `8v check` and `8v fmt` (why both?)
- Explaining when `8v` is available as MCP vs. when to shell out

### Q21: 8v vs. native tools
**Better**: Batch operations (one call reads 10 files' symbol maps), progressive disclosure (symbol map before full content), unified interface (no switching between grep/cat/sed), consistent JSON output flag.
**Worse**: No file-existence check, no multi-file refactor in one call, no output format documentation for most commands, no streaming for large outputs, escaping rules for `write` are non-obvious.

### Q22: single highest-impact edit
Add a concrete before/after error example for `8v write` failure modes — specifically what the caller sees when `--find` text isn't found, when a line number is out of range, and when a path doesn't exist. Error output clarity is the largest gap in the current instructions.

---

## F. Overall

### Q23: clarity scores

- **Axis 1 — Input clarity**: 7/10 — Commands and flags are well-documented with examples for `read` and `ls`; `write` escape syntax is explained but lacks a concrete shell-safe example; `search` regex vs. glob ambiguity is unresolved.
- **Axis 2 — Output clarity**: 5/10 — `read` output is well-described (symbol map format, batch delimiters, JSON shape); all verify commands (`check`, `fmt`, `test`, `build`) and `write` output are entirely undocumented beyond exit-code hint for `check`.
- **Axis 3 — Failure-mode clarity**: 2/10 — Only one failure mode is documented (`--find` fails if not found); no error format, no stderr/stdout distinction, no behavior for missing paths, no partial-batch failure behavior.
- **Composite mean**: (7 + 5 + 2) / 3 = **4.67**

### Q24: one-minute improvement
Add a single "Error contract" paragraph: "On any error, 8v exits non-zero, prints a human-readable message to stderr, and (with `--json`) returns `{"error": "<message>"}` to stdout." This single addition would eliminate the largest ambiguity cluster across Q7, Q18, Q26, Q27, Q28, and Q29.

---

## G. Output contracts (predict from text only)

### Q25: Python fixture predictions

**a. `8v read util.py` — what is returned?**
```
1  def add
4  result
```
The symbol map. Lines: `<line-number>  <symbol>`. Exact format: two spaces between number and name. I'd expect `add` (the function) and possibly `result` (a top-level assignment, though whether assignments are "symbols" is not stated). Gap: instructions don't define what counts as a symbol in Python — only Rust-style examples are given. Confidence is low for `result`.

**b. `8v read util.py:1-2` — what is returned?**
```
def add(a, b):
    return a + b
```
The raw file content for lines 1 through 2 inclusive. No `===` header because it's a single file. This is inferred from: "line range (1-indexed, end inclusive)" and "Single-file reads emit no header."

**c. `8v search "add" util.py` — what is returned?**
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
Both lines containing "add" grouped under the file. Format: `<path>:<line>:<text>` as documented. No context lines (no `-C` flag used).

**d. `8v read util.py --full` — what is returned?**
```
def add(a, b):
    return a + b

result = add(1, 2)
```
Full file content. No `===` header (single file). Inferred from "`--full` — entire file" and "Single-file reads emit no header."

### Q26: `8v check .` exit codes and output location
- Exit code on lint error: non-zero (stated: "Non-zero exit on any issue")
- Exit code on success: instructions don't say — my guess: 0
- Where error text appears: **instructions don't say** — no mention of stdout vs. stderr. Gap: "Non-zero exit on any issue" is the only contract stated. `--json` output shape on error is not documented.

### Q27: `8v write --find --replace` behavior
- `<old>` appears zero times: "fails if `<old>` not found" — exits with failure. What "fails" means (exit code, message format, stderr vs. stdout) is not stated.
- `<old>` appears more than once: **instructions don't say** — my guess: replaces all occurrences, but this is not stated. Gap: no documentation on what happens with multiple matches.
- What is returned to caller: **instructions don't say** — no success/failure output contract documented.

### Q28: `8v read <path>` on non-existent path
**Instructions don't say** — no description of behavior when a file doesn't exist. My guess: non-zero exit code and an error message on stderr. The instructions do not describe this scenario at all.

---

## H. Contract reasoning

### Q29: `8v check .` — no stdout, no stderr, exit code 1
Based only on the instructions: the docs say "Non-zero exit on any issue" but say nothing about where error output appears. A exit code 1 with no visible output is **not explicitly excluded** by the instructions — the output destination is a gap. What the agent should do: check if `--json` flag was missed (perhaps errors appear only in JSON mode), or run `8v check . --json` to get structured output. Relevant quote: "Non-zero exit on any issue." Gap: no statement about stdout/stderr for error output.

---

## I. API coherence

### Q30: differences between Surface 1 and Surface 2
Surface 1 (ai_section.txt) vs. Surface 2 (instructions.txt):

1. **Introductory sentence**: Surface 1 says "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations." Surface 2 says "8v — code reliability tool for AI agents. Designed to minimize round-trips." Then restates similar scope. Surface 2 adds "minimize round-trips" framing; Surface 1 is more prescriptive about what NOT to use.

2. **Batch read reinforcement**: Surface 1 adds "One call beats N sequential calls" after the batch read example. Surface 2 omits this sentence. Matters for agents seeing only Surface 2 — slightly weaker Batch principle reinforcement.

3. **Section heading**: Surface 1 uses "## Write"; Surface 2 uses "## Write — prefer targeted edits." Surface 2 is more directive. Matters because an agent seeing Surface 1 gets no framing to prefer targeted over wholesale edits.

4. **Section heading**: Surface 1 uses "## Discovery"; Surface 2 uses "## Discovery — learn the repo in one call." Same pattern — Surface 2 more directive.

5. Otherwise the two surfaces are functionally identical in command coverage and examples. Surface 2 (MCP instructions) is slightly more terse in one place; Surface 1 is slightly more verbose in reinforcing Batch. Neither surface is strictly more complete — they cover the same command set.

---

## J. Tool-gap surfacing

### Q31: task that cannot be completed with only 8v
**Task**: Rename a file (move it to a different path).
**Missing capability**: No `8v move`, `8v rename`, or `8v cp` command exists in the instructions.
**Closest substitute**: Fall back to Bash `mv old_path new_path`. Cost: breaks the single-tool discipline, requires shell access, and forces the agent to track the rename manually for subsequent `8v write` calls. There is no 8v-native way to rename a file.

Other gaps: creating a new empty file (no `8v touch`), changing file permissions, checking disk space.

---

## K. Behavioral dry-run

### Q32: 5-step Go task

**Step 1: Find all Go source files in the repo.**
Command: `8v ls --match "*.go" --tree`
Confidence: 4 — `--match <glob>` is documented; combining with `--tree` is reasonable. Minor uncertainty: whether `--match` and `--tree` can be combined isn't stated.

**Step 2: Search for all usages of `http.Get` across Go files, with 2 lines of context.**
Command: `8v search "http\.Get" -e go -C 2`
Confidence: 4 — `-e <ext>` and `-C N` are both documented. Note: `http.Get` needs regex escaping (`\.`) since the pattern is regex.

**Step 3: Read the symbol map of the file with the most matches.**
Command: `8v read <path-from-step-2>`
Confidence: 5 — standard symbol map read.

**Step 4: Replace `http.Get` with `httpClient.Get` on a specific line.**
Command: `8v write <path>:<line> "    result, err := httpClient.Get(url)"`
Confidence: 3 — I'd need to know the exact current line content to replace it. Alternatively: `8v write <path> --find "http.Get" --replace "httpClient.Get"`. Confidence for `--find/--replace`: 4, but multiple occurrences behavior is undocumented (gap from Q27).

**Step 5: Run the tests and confirm they pass.**
Command: `8v test .`
Confidence: 4 — documented. "Confirm they pass" requires checking exit code; instructions say non-zero on failure for `check` but don't explicitly state this for `test`. My inference: exit code 0 = pass.

No steps require native tool fallback given these assumptions, but Step 1's `--match` + `--tree` combination is uncertain.

---

## L. Memorability

### Q33: write syntax from memory

**Insert a new line before line 42 of `main.rs`:**
```
8v write main.rs:42 --insert "new line content"
```
Confidence: 5 — the `--insert` flag syntax was clear and distinct.

**Replace lines 10–20 with multi-line content:**
```
8v write main.rs:10-20 "line one\nline two\nline three"
```
Confidence: 4 — the range-replace syntax `<path>:<start>-<end>` is memorable; the `\n` escape for newlines is documented. Slight uncertainty on whether shell quoting will interfere with the `\n` (the instruction says "literal two-character sequence" but doesn't give a shell-level example).

---

**Model & run ID:** Claude Sonnet 4.6 / clarity-v4 / sonnet-run1 / 2026-04-20

---

## 8v feedback

Smooth. No retries, no workarounds. All three files read in a single parallel batch using the Read tool. The `8v` MCP tool (mcp__8v-debug__8v) was available via ToolSearch but this task required reading source files, not running 8v commands — so native Read was appropriate per the benchmark constraint of treating the two surfaces as the only documentation. Directory creation used Bash (`mkdir -p`) which is within scope (filesystem management). The Write tool was used for the output file, which is also appropriate (creating a new findings file). No friction encountered in the tooling itself.
