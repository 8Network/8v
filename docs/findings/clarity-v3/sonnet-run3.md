# Instruction Clarity v3 — Sonnet Run 3 — 2026-04-19
Model: Claude Sonnet 4.6
Surfaces read: ai_section.txt, instructions.txt

---

## Q1: Summary

8v is a code reliability tool for AI agents that replaces native file-operation tools (Read, Edit, Write, Grep, Glob) with a unified CLI optimized for minimal round-trips. It is used for all file reads, writes, searches, and inspections within a project — native shell tools are reserved only for git, process management, and environment operations. It operates on two core principles: Progressive (default output is the minimum useful answer; flags escalate detail) and Batch (many inputs in a single call to amortize per-call overhead).

---

## Q2: Minimum-viable invocations

- `8v ls` — directory listing; with `--tree --loc` shows full hierarchy with line counts. Default (no flags) behavior not explicitly stated.
- `8v read <path>` — returns a symbol map (functions, structs, classes), one line per symbol: `<line-number>  <symbol>`.
- `8v search <pattern>` — searches repo with regex; default groups matches by file as `<path>:<line>:<text>`.
- `8v write <path>:<line> "<content>"` — replaces a single line; multiple forms available.
- `8v check .` — runs lint + type-check + format-check; non-zero exit on any issue.
- `8v fmt .` — auto-formats files in place; idempotent.
- `8v test .` — runs project tests.
- `8v build .` — compiles.

---

## Q3: Two principles

**Progressive:** Default output is the minimum useful answer; flags escalate detail. The instructions exist so an agent can do cheap discovery without paying for data it doesn't need. Non-obvious example: `8v read <path>` returns only a symbol map, not file contents — you must explicitly request a line range or `--full` to see code. The default is deliberately impoverished.

**Batch:** Pass many inputs in one call instead of calling N times. Per-call overhead is real (described as "schema tax" in the MCP surface and "overhead" in the CLAUDE.md surface). Non-obvious example: You can batch multiple ranges of the *same* file — `8v read a.rs:1-200 a.rs:200-400` — in a single call, not just distinct files.

---

## Q4: When to use 8v vs. native tools

**Use 8v for:** All file operations — reading, writing, searching, editing, discovering structure, running checks/tests/builds.

**Use native tools (Bash/shell) for:** git operations, process management, environment operations. The instructions are explicit: "Use Bash only for git, process management, and environment operations." Everything file-related should route through 8v.

---

## Q5: Discovering flags

Run `8v <cmd> --help` for the full flag list. Both surfaces state: "Every command accepts `--json`. Run `8v <cmd> --help` for the full flag list."

---

## Q6: Ambiguous phrases

1. **"Use Bash only for git, process management, and environment operations"** — "process management" could mean (a) starting/stopping long-running processes or (b) any invocation of an external process. This matters when you need to, e.g., run a migration script: is that "process management" or a file operation adjacent task?

2. **"Default output is the minimum useful answer"** — "useful" is subjective. For `ls` with no flags, the instructions do not define what the minimum output actually is (tree? flat list? just names?).

3. **"Each call costs overhead — amortize it"** — Does "overhead" mean latency, token cost, or both? The MCP surface calls it "schema tax" but never defines what that is in measurable terms.

4. **"Typical flow: ... → `8v write` → `8v test .` → `8v check .`"** — "typical" implies other flows exist but none are shown. An agent may wrongly assume this is the only valid sequence.

5. **"`8v write <path>:<start>-<end> \"<content>\""** — does `<content>` replace all lines in the range with a single new line, or can it contain multiple lines (and if so, does the range size need to match the replacement size)?

6. **"Fails if `<old>` not found"** — "fails" is undefined: non-zero exit? error message? what format?

7. **"Last resort"** for `--full` — implies a preference but no rule about when "last resort" is actually triggered. An agent might interpret this as advisory rather than binding.

---

## Q7: Implied but never stated behaviors

- **Exit codes on success:** No surface states what exit code success produces (presumably 0, but not stated).
- **Error message format:** When a command fails, is the error on stdout, stderr, or only surfaced via `--json`? Never stated.
- **JSON shape:** `--json` is mentioned but the schema is never shown. An agent cannot predict the structure.
- **What happens when path doesn't exist:** Not stated. Error? Empty result? Partial results in batch?
- **Output interleaving in batch reads:** When batching multiple files, is output concatenated per-file or interleaved? Only the symbol-map format is shown for single files.
- **Whether write is atomic:** No mention of atomicity, partial writes, or rollback on failure.
- **What `8v ls` returns by default (no flags):** Not described.
- **Whether `8v search` has a default path (repo root?):** Not stated explicitly.
- **Unicode handling:** Not mentioned.
- **Whether `--json` changes exit code behavior:** Not stated.
- **Whether `8v fmt .` and `8v check .` accept a file path (not just `.`):** Implied by "Pass a path to scope to a subtree" for verify commands, but not shown with an example.
- **What counts as a "symbol":** The example shows `fn main`, `pub struct Args`, `impl Args` — but not enum variants, constants, type aliases, or non-Rust constructs.

---

## Q8: Undefined terms

- **"symbol map"** — used throughout but never defined. The example output shows `fn main`, `pub struct Args`, `impl Args` for Rust, but what constitutes a "symbol" for Python, TypeScript, Go, etc.?
- **"stack"** — valid values are listed but "stack" itself isn't defined. A new user might not know this means "language/tech stack."
- **"progressive"** — used as a principle name but the meaning must be inferred from the description.
- **"overhead" / "schema tax"** — two different words across surfaces for the same concept; neither is defined with a concrete cost (tokens? ms? API calls?).
- **"compact mode"** — appears in the benchmark question template but not in the instructions under test; not relevant here but worth flagging as a term an agent might search for.
- **"most Bash"** — the MCP surface says "Use shell tools only for git, process management, and environment operations"; the CLAUDE.md surface says "Use Bash only for git, process management, and environment operations." Neither defines "Bash" vs. "shell tools."
- **"verify commands"** — used collectively to mean check/fmt/test/build, but never labeled as such in the command descriptions.

---

## Q9: Contradictions between surfaces

No hard contradictions. Minor differences:

1. **Section header for Write:** CLAUDE.md calls it `## Write`; instructions.txt calls it `## Write — prefer targeted edits`. The "prefer targeted edits" guidance exists only in the MCP surface.
2. **`8v read` batch example trailing sentence:** CLAUDE.md adds "One call beats N sequential calls." instructions.txt omits this.
3. **Search description:** CLAUDE.md puts the default output description (`Default output groups matches by file: ...`) under a bullet; instructions.txt puts it as a continuation line without a bullet marker, making it marginally less scannable.
4. **The symbol map example in `read`:** CLAUDE.md prefixes lines with backtick-wrapped examples (`\`12  fn main\``); instructions.txt uses the same formatting. Consistent.
5. **Batch cost framing:** CLAUDE.md says "Each call costs overhead"; instructions.txt says "Each call costs overhead" — same wording. MCP tool description embedded in tool schema says "Each call costs schema tax" — a third variant not in either file but visible to agents using the tool.

---

## Q10: Batch read output format

The instructions state the single-file output format (`<line-number>  <symbol>` per line), and give a Rust example. They state you can batch "distinct files, multiple ranges of the same file, or a mix." However, **neither surface describes whether batch output is concatenated per-file with a header/separator, or interleaved**. The benchmark question asks exactly this — it is a genuine gap. My inference from the phrase "any combination of paths and ranges in one call" and the `=== filename ===` header I observed during this actual run is that each file gets a header, but that is from experience, not from the instruction text.

**Gap:** Instructions don't say how batch output is separated.

---

## Q11: Write content, trailing newline, multi-line

- **Trailing newline:** Not stated. The instructions say `\n` becomes a newline inside content, but do not say whether a trailing newline is appended automatically or not.
- **Multi-line content:** Implied via `\n` escape: "Content arguments are parsed by 8v (not the shell): `\n` becomes a newline, `\t` a tab, `\\` a literal backslash." So you embed `\n` literally as two characters to produce multi-line content.
- **Surrounding quotes:** The instructions explicitly say "do not rely on shell interpolation" and "Pass them as literal two-character sequences." This means the quotes are shell-level delimiters only; the escaping is done by 8v, not the shell.

**Gap:** Whether a trailing newline is added automatically is not stated.

---

## Q12: Concrete commands + confidence

**a. Read 5 files at once.**
Command: `8v read file1.rs file2.rs file3.rs file4.rs file5.rs`
Confidence: 5 — directly stated: "batch any combination of paths and ranges in one call."

**b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.**
Command: `8v write foo.rs:10-20 "line one\nline two\nline three"`
Confidence: 3 — the range-replace form is stated; embedding `\n` for multi-line is stated. But whether the range must match the replacement line count is not stated, so I'm uncertain whether 11 lines-in → 3 lines-out is valid or throws an error.

**c. Find all functions named `handle_*` across the repo.**
Command: `8v search "fn handle_" --files` or `8v search "handle_\w+" -e rs`
Confidence: 3 — search supports regex and `-e <ext>`. The exact pattern syntax for "all functions named handle_*" requires knowing the language, and the instructions only say regex is supported. `--files` would return just paths; omitting it returns the matching lines. I'd use the line-output form.

**d. Append one line to `notes.md`.**
Command: `8v write notes.md --append "new line content"`
Confidence: 5 — directly stated.

**e. Symbol map for `bar.rs`, then read lines 100–150.**
Commands: `8v read bar.rs` then `8v read bar.rs:100-150`
Or batched: `8v read bar.rs bar.rs:100-150`
Confidence: 5 — both forms are directly stated. Batching both in one call is shown in the batch example.

**f. Run tests and parse JSON output.**
Command: `8v test . --json`
Confidence: 4 — `--json` is stated to be accepted by every command. The JSON shape is not documented, so parsing requires inspecting actual output, but the command is correct.

**g. Check whether a file exists before reading.**
Command: Instructions don't say — my guess: There is no `8v exists` or `8v stat` command described. I would either attempt `8v read <path>` and handle the error (but error format is undocumented), or fall back to Bash `test -f <path>`.
Confidence: 1 — this capability is absent from the instructions.

**h. Delete lines 50–60.**
Command: `8v write <path>:50-60 --delete`
Confidence: 5 — directly stated.

**i. Insert a new line before line 30.**
Command: `8v write <path>:30 --insert "new line content"`
Confidence: 5 — directly stated.

**j. Search only Rust files, case-insensitive, for `TODO`.**
Command: `8v search "TODO" -i -e rs`
Confidence: 4 — `-i` and `-e <ext>` are both listed in the search flags. The combination is implied to work but no example shows both flags together.

**k. Find all files by name matching `*_test*.md`.**
Command: `8v ls --match "*_test*.md"`
Confidence: 4 — `--match <glob>` is listed for `ls`. The exact glob quoting is shell-dependent but the flag exists.

**l. Run lint + format-check + type-check with one command.**
Command: `8v check .`
Confidence: 5 — directly stated: "lint + type-check + format-check."

**m. Replace `old_name` with `new_name` across a multi-file refactor.**
Command: `8v write <path> --find "old_name" --replace "new_name"` — but this is per-file. For multi-file, must call once per file.
Confidence: 2 — `--find/--replace` is stated but only for a single path. There is no documented glob or recursive form. For a true multi-file refactor I would need to either script multiple write calls or fall back to Bash `sed`.

**n. Read just the symbols of 10 files in one call.**
Command: `8v read f1.rs f2.rs f3.rs f4.rs f5.rs f6.rs f7.rs f8.rs f9.rs f10.rs`
Confidence: 5 — default read returns symbol map; batching is explicitly stated.

**o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.**
Command: `8v read big.rs:1-200 big.rs:500-600`
Confidence: 5 — directly stated: "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)."

---

## Q13: Teaching method per command

| Command | Method |
|---------|--------|
| `ls`    | Example + description — `8v ls --tree --loc` shown; `--match` and `--stack` with values shown |
| `read`  | Example + description — symbol-map example output shown; range and batch forms shown |
| `search`| Description + partial example — flag list shown; default output format described; no full worked example |
| `write` | Description + example forms — six distinct forms listed with syntax; `\n`/`\t` escaping described |
| `check` | Description-only — "lint + type-check + format-check. Non-zero exit on any issue." |
| `fmt`   | Description-only — "auto-format files in place. Idempotent." |
| `test`  | Description-only — "run project tests." |
| `build` | Description-only — "compile." |

---

## Q14: Three most likely mistakes

1. **Using `--find/--replace` expecting it to work across all files.** The flag is described only for a single `<path>`. In a refactor, I would naturally try `8v write . --find "old" --replace "new"` expecting recursive behavior — the instructions do not say this works or doesn't work, so the first attempt would likely fail silently or with an undocumented error.

2. **Forgetting that `\n` must be passed as a literal two-character sequence (backslash + n), not a shell-expanded newline.** The instruction says "do not rely on shell interpolation" but an agent writing `$'line1\nline2'` in a shell-invoked context would produce the wrong input. The boundary between MCP-tool invocation (where shell doesn't apply) and Bash invocation (where it does) is unclear to a fresh agent.

3. **Assuming the symbol map covers all symbol types across all languages.** The example is Rust-specific (`fn`, `struct`, `impl`). A Python agent would not know whether `class`, `def`, or module-level variables appear, and might call `--full` unnecessarily or miss relevant symbols.

---

## Q15: When I'd fall back to Bash/native tools

- **File existence checks** — no `8v exists` or `8v stat` described.
- **Multi-file find/replace refactors** — `--find/--replace` is per-file; no recursive or glob form.
- **Running arbitrary shell commands** — e.g., database migrations, build scripts, environment setup.
- **Git operations** — explicitly delegated to Bash by the instructions.
- **When `8v write` fails with an undocumented error** — if the error message doesn't tell me what to do, I'd fall back to direct file editing via Write/Edit tools.
- **Reading binary files or images** — not mentioned; presumably not supported.
- **Watching files for changes** — no `8v watch` described.

---

## Q16: Most/least used commands

**Most used:** `8v read` (symbol map + range), `8v search`, `8v write`, `8v check .` — the core read-modify-verify loop is what every coding task reduces to.

**Least used:** `8v build .` and `8v fmt .` — build is implicit in test/check workflows; fmt is idempotent and would typically be run once at the end. `8v ls --tree --loc` is also infrequent (only at session start).

---

## Q17: First command in a new repo

`8v ls --tree --loc` — explicitly recommended as "Start here." It provides the full file hierarchy with line counts, giving context for all subsequent reads and searches without opening any file.

---

## Q18: Trust `8v write` failure error

Instructions don't say — my guess: Partially. The instructions say `--find/--replace` "fails if `<old>` not found" — which implies a meaningful error for that case. But for other failure modes (bad path, permission error, range out of bounds), nothing is documented. I would trust the error for `--find` failures and be uncertain for others. Confidence in the error being actionable: 2/5.

---

## Q19: What's missing

- **Error output contract:** Where do errors go (stdout/stderr)? What format? Are they machine-readable without `--json`?
- **`--json` schema documentation:** Even one example JSON shape per command would reduce guessing.
- **`ls` default behavior:** What does `8v ls` with no flags return?
- **Symbol coverage by language:** What counts as a symbol for Python, Go, TypeScript, Java?
- **Range validation:** What happens if start > end, or end > file length?
- **Multi-file `--find/--replace`:** Glob or directory form.
- **Atomicity/rollback for write operations.**
- **`--stack` filter working with `search`:** Can `--stack` scope a search to a language subset?
- **What "verify commands accept a path" means:** Is it a file or a directory or both?

---

## Q20: Where I'd hesitate while teaching

1. Explaining what a "symbol map" returns for non-Rust languages.
2. Explaining the exact multi-line write syntax — the `\n`-as-literal-two-chars distinction is easy to get wrong in shell contexts.
3. Explaining what to do when `8v write` fails with anything other than `--find` not found.
4. Explaining the batch output format — headers per file? Interleaved? Not documented.
5. Explaining what `8v ls` with no flags does.

---

## Q21: 8v vs. native tools

**Better:**
- Batching: one call for multiple files/ranges vs. N read calls.
- Symbol map: immediate structural overview without reading the full file.
- Progressive disclosure: default is cheap; detail is opt-in.
- `check` combines lint + type-check + format-check in one command.
- Consistent `--json` across all commands for machine parsing.

**Worse:**
- No multi-file find/replace in one command.
- No file existence check.
- Error contracts are undocumented (native tools have well-known exit codes and stderr conventions).
- JSON schema is undocumented (can't write a reliable parser).
- No support for arbitrary shell operations (must context-switch to Bash for git, env, process management).
- Symbol map coverage for non-Rust languages is unknown from instructions alone.

---

## Q22: Highest-impact single instruction edit

Add one worked example showing what the error output looks like when `8v write` fails, and what exit code `8v check` returns on success vs. failure, with `--json` shape. The single largest practical gap is: agents don't know where errors appear (stdout/stderr) or what format they take, which causes retry loops.

---

## Q23: Overall clarity scores

- **Axis 1 — Input clarity:** 8/10. The command syntax is well-specified with examples for most forms; the write escaping rules are explicit; batch syntax is clearly demonstrated. Loses 2 points because `ls` default behavior is absent, `write` range-vs-replacement-line-count behavior is unspecified, and multi-file `--find/--replace` scope is ambiguous.

- **Axis 2 — Output clarity:** 5/10. The symbol-map format is shown with a concrete example; search default output format is described. But `--json` schema is never shown, batch output separator format is never described, and check/fmt/test/build return nothing beyond "non-zero exit on any issue." Half of all commands have output that is only partially documented.

- **Axis 3 — Failure-mode clarity:** 2/10. The only documented failure mode is `--find/--replace` failing when `<old>` not found. No exit codes on success are given. No error format (stdout/stderr/json). No behavior when a path doesn't exist. No behavior when a range is out of bounds. This is the largest gap.

- **Composite mean:** (8 + 5 + 2) / 3 = **5.00**

---

## Q24: One-minute improvement

Add a three-row "Failure behavior" table:

| Condition | Exit code | Output location |
|-----------|-----------|-----------------|
| Success | 0 | stdout |
| Command error (bad args, path not found) | 1 | stderr |
| Check/lint failure | non-zero | stdout (and `--json`) |

This single addition would eliminate the largest class of agent retry loops.

---

## Q25: Output contract predictions

**a. `8v read util.py`**

Expected (based on documented symbol-map format `<line-number>  <symbol>`):
```
1  def add
```
Possibly also `4  result = add(1, 2)` if module-level assignments are treated as symbols. Instructions don't say — my guess: only function/class definitions appear, so only line 1 is returned. **Gap:** symbol coverage for Python is not specified; the example is Rust-only.

**b. `8v read util.py:1-2`**

Expected (line range, 1-indexed, end inclusive):
```
1  def add(a, b):
2      return a + b
```
Confidence: 4 — line range form is well-described; the content is the file's actual lines. The only gap is whether line numbers are included in the output (instructions show the symbol map has line numbers in the default view; range output format is not explicitly shown).

**c. `8v search "add" util.py`**

Expected (default format `<path>:<line>:<text>`):
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
**Gap:** The instructions say "Default output groups matches by file: `<path>:<line>:<text>`" but don't specify whether the path prefix is repeated per line or shown as a group header. My prediction assumes per-line.

**d. `8v read util.py --full`**

Expected:
```
1  def add(a, b):
2      return a + b
3  
4  result = add(1, 2)
```
**Gap:** Whether line numbers are included in `--full` output is not stated. The instructions never show `--full` output format — only the symbol-map format has an explicit example.

---

## Q26: `8v check .` exit codes and output location

Instructions state: "Non-zero exit on any issue." That is the only stated behavior.

**Gaps:**
- Exit code on success: not stated (presumably 0, but not documented).
- Specific non-zero exit code value(s) on failure: not stated.
- Where error text appears (stdout, stderr, or only `--json`): not stated.
- The `--json` flag is mentioned as accepted, but the shape is not shown.

Quote: `"8v check . — lint + type-check + format-check. Non-zero exit on any issue."`

---

## Q27: `8v write --find/--replace` edge cases

**If `<old>` appears zero times:** Stated — "fails if `<old>` not found." The failure format (exit code, message location) is not stated.

**If `<old>` appears more than once:** Instructions don't say — my guess: either replaces all occurrences or fails with an ambiguity error. There is no documentation for this case.

**What is returned to the caller on success:** Not stated. Presumably some confirmation or nothing, but the instructions give no example.

**Gap quote:** `"8v write <path> --find \"<old>\" --replace \"<new>\" — fails if \`<old>\` not found."` No other behavior is documented.

---

## Q28: `8v read <path>` on nonexistent path

Instructions don't say — my guess: a non-zero exit code and an error message on stderr. The format, structure, and whether `--json` transforms the error are entirely undocumented.

**Gap:** No instruction text addresses this case. The closest is the general principle "Every command accepts `--json`" which implies structured output is available, but what it looks like for an error is unknown.

---

## Q29: `8v check .` exits 1 with no stdout/no stderr

Based only on the instructions: **this is not expected behavior, but the instructions don't say it can't happen.** The instructions state "Non-zero exit on any issue" and "All verify commands accept `--json`" but do not specify that error output must always be present. If a lint tool itself crashes silently, `8v check` might propagate the non-zero exit without adding any output of its own.

**What the agent should do next:** Instructions don't say. My guess: re-run with `--json` to see if structured error output appears; if still empty, fall back to running the underlying lint tool directly (but that requires knowing which tool it wraps, which is not documented).

**Gap:** No instruction text addresses the silent-failure-with-exit-1 case.

---

## Q30: Surface 1 vs. Surface 2 differences

| Aspect | Surface 1 (CLAUDE.md / ai_section.txt) | Surface 2 (instructions.txt / MCP) | More complete |
|--------|----------------------------------------|-------------------------------------|---------------|
| Write section header | `## Write` | `## Write — prefer targeted edits` | Surface 2 — adds preference signal useful to agents |
| Batch read trailing sentence | "One call beats N sequential calls." | Absent | Surface 1 — reinforces the batching imperative |
| Search description formatting | Bullet list item | Continuation line (no bullet) | Surface 1 — slightly more scannable |
| Tool-scope instruction | "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations." | "Use `8v` for all file operations (read, edit, write, search, inspect). Use shell tools only for git, process management, and environment operations." | Surface 1 — names the specific tools being replaced (Read, Edit, Write, Grep, Glob), making displacement explicit |
| `--stack` valid values | Listed inline in Discovery section | Listed inline | Equal |
| Discovery section header | `## Discovery` | `## Discovery — learn the repo in one call` | Surface 2 — clarifies intent |

**Impact to an agent seeing only one surface:** An agent seeing only Surface 2 loses the "One call beats N" reinforcement and the explicit list of displaced tools (Read, Edit, Write, Grep, Glob), which is the mechanism that prevents Bash fallback. An agent seeing only Surface 1 loses the "prefer targeted edits" signal for write operations.

---

## Q31: Task 8v cannot complete

**Task:** Rename a file (e.g., move `old_module.rs` to `new_module.rs` and update all references in one atomic operation).

**Missing capability:** There is no `8v mv`, `8v rename`, or filesystem-move command described. The `write --find/--replace` only rewrites content inside files, not file paths themselves.

**Closest substitute and cost:** Bash `mv old_module.rs new_module.rs` for the rename, followed by `8v search "old_module" -e rs` to find all references, followed by per-file `8v write <path> --find "old_module" --replace "new_module"` calls for each reference. Cost: requires context-switching to Bash for the rename step plus N sequential write calls for references (no multi-file write form). High friction for a routine refactor operation.

---

## Q32: Behavioral dry-run — Go `http.Get` task

**Step 1: Find all Go source files in the repo.**
Command: `8v ls --stack go --match "*.go"`
Confidence: 3 — `--stack go` is listed as valid; `--match` takes a glob. The combination is implied to work but no example shows both flags. Alternatively: `8v ls --stack go` alone.

**Step 2: Search for all usages of `http.Get` across those files, with 2 lines of context.**
Command: `8v search "http\.Get" -e go -C 2`
Confidence: 4 — `-e <ext>` and `-C N` are both listed flags. `http.Get` needs the dot escaped in regex. The combination is not shown in an example but both flags are documented.

**Step 3: Read the symbol map of the file with the most matches.**
Command: First get match counts: `8v search "http\.Get" -e go --files` to get the list of files, then determine which has the most (the instructions have no `--count` flag documented, so I cannot get counts directly from 8v).
**Fall back needed:** To count matches per file, I'd need either a `--count` flag (not documented) or pipe `8v search` output through Bash `sort | uniq -c | sort -rn`. Falling back to Bash for count aggregation.
Once I know the file: `8v read <file_with_most_matches.go>`
Confidence: 2 for the count step (requires Bash); 5 for the symbol map read.

**Step 4: Replace `http.Get` with `httpClient.Get` on a specific line.**
Command: `8v write <file>:<line> "    result, err := httpClient.Get(url)"` (replacing exact line content)
Or: `8v write <file> --find "http.Get" --replace "httpClient.Get"` — but this replaces ALL occurrences in the file, not just the specific line.
Confidence: 3 — for the single-line replacement, I need the exact new line content, which means I must first read the line. For targeted single-line replacement: `8v write <file>:<line> "<new content>"` is 5/5 if I have the content; the `--find/--replace` form risks changing multiple occurrences.

**Step 5: Run the tests and confirm they pass.**
Command: `8v test .`
Confidence: 5 — directly stated. To confirm pass (vs. fail), rely on exit code 0 = pass (not stated but universally assumed).

---

## Q33: Memorability — write syntax from memory

**Insert a new line before line 42 of `main.rs`:**
```
8v write main.rs:42 --insert "new line content"
```
Confidence: 5 — this form is explicit and distinctive.

**Replace lines 10–20 with multi-line content:**
```
8v write main.rs:10-20 "line one\nline two\nline three"
```
Confidence: 4 — the range form is clear; embedding `\n` for multi-line is stated. The uncertainty is whether the number of replacement lines must match the replaced range size (11 lines replaced by 3 — is this valid?).

---

**Model & run ID:** Claude Sonnet 4.6 / sonnet-run3 / 2026-04-19

---

## 8v feedback

8v worked smoothly for this run. Single observation:

- **Batch read with `--full`:** The command `8v read file1 file2 file3 --full` was used to read all three inputs at once. This worked correctly — each file was returned with a `=== path ===` header separator, which is useful. However, this separator format is **not documented in either instruction surface**. An agent relying only on the instructions would not know to expect it, and could not reliably parse the output programmatically without seeing it first. Expected: the instructions to document the batch separator format (e.g., `=== <path> ===` before each file's content). No retry or workaround was needed.
