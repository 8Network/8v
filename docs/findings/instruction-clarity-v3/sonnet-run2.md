# Instruction Clarity v3 — Sonnet Run 2 — 2026-04-19
Model: Claude Sonnet 4.6
Surfaces read: ai_section.txt, instructions.txt

---

## Q1: Summary

8v is a code reliability tool for AI agents that replaces native file-operation tools (Read, Edit, Write, Grep, Glob, Bash) with a single unified CLI. It is used whenever an agent needs to read, search, edit, or verify files in a codebase — leaving Bash only for git, process management, and environment operations. The two governing principles are Progressive (default output is the minimal useful answer; flags escalate) and Batch (pass many inputs in one call to amortize per-call overhead).

---

## Q2: Minimum-viable invocations

- `ls`: `8v ls --tree --loc` — returns full file hierarchy with line counts.
- `read`: `8v read <path>` — returns a symbol map (functions, structs, classes) with line numbers.
- `search`: `8v search <pattern>` — returns matches grouped by file as `<path>:<line>:<text>`.
- `write`: `8v write <path>:<line> "<content>"` — replaces the specified line.
- `check`: `8v check .` — runs lint + type-check + format-check; non-zero exit on any issue.
- `fmt`: `8v fmt .` — auto-formats files in place; idempotent.
- `test`: `8v test .` — runs project tests.
- `build`: `8v build .` — compiles.

---

## Q3: The Two Principles

**Progressive:** Default output gives the minimum useful answer. Each command starts at the lowest level of detail and flags escalate: `8v read` gives a symbol map; add `:start-end` for a range; add `--full` for the entire file. Non-obvious example: for a 3,000-line file, you get a ~20-line symbol map by default — you only pay for the lines you actually need by adding a range after inspecting symbols.

**Batch:** Pass multiple inputs in one call instead of calling N times, because each call carries overhead. Non-obvious example: you can mix a plain file and a range of the same file in one call — `8v read big.rs:1-200 big.rs:500-600 Cargo.toml` — getting three separate reads for the price of one call.

---

## Q4: When to use 8v vs. not

Use 8v for: reading files, searching content, editing/writing files, discovering repo structure, running lint/tests/build/fmt.

Do NOT use 8v for: git operations, process management (starting/stopping daemons), environment variable operations. Those belong to Bash/shell tools.

---

## Q5: Discovering flags

Run `8v <cmd> --help` for the full flag list for any command. Every command also accepts `--json`.

---

## Q6: Ambiguous phrases

- `"Use Bash only for git, process management, and environment operations"` — "environment operations" is undefined. Could mean: reading env vars (`printenv`), exporting variables, or managing `.env` files. The boundary is fuzzy.
- `"Each call costs overhead"` / `"schema tax"` (MCP surface) — two different words for the same concept with no quantification. An agent cannot tell whether overhead is 10ms or 1000ms, or token-count vs. latency.
- `"Start here"` (in `8v ls --tree --loc`) — implies this is the mandatory first command in every session, but the instructions don't say what "here" means for subsequent sessions.
- `"Typical flow: 8v ls → read → ranges → write → test → check"` — "typical" leaves open whether this is required or advisory.
- `"Pass a path to scope to a subtree"` (Verify section) — does this work for all four verify commands equally? Unconfirmed for `fmt` and `build`.
- `"symbol map"` in the Read section heading — defined by example but not by what it omits (e.g., does it include anonymous closures? top-level constants? imports?).
- `"8v read <path> --full — entire file. Last resort."` — "Last resort" is advice, not a rule. An agent may not know when it qualifies.

---

## Q7: Implied but never stated

- **Error format for failures**: instructions say `--find` "fails if `<old>` not found," but don't state what the failure looks like (exit code? stderr message? JSON error?).
- **Exit codes for success**: only stated that `check` returns "non-zero exit on any issue." Success = exit 0 is implied, not stated.
- **Where errors go** (stdout vs. stderr): never mentioned for any command.
- **JSON shape**: `--json` accepted by every command but the schema of the JSON is never shown.
- **What happens when a path doesn't exist**: `read`, `write`, `search`, `ls` — none describe the not-found behavior.
- **Output interleaving for batch reads**: when you read 3 files, does output appear as one block per file, or merged? The example format `=== filename ===` separator is not in the instructions (inferred from MCP tool description runtime behavior, not text).
- **Unicode handling**: never mentioned.
- **Shell quoting for content args**: instructions say "Pass them as literal two-character sequences — do not rely on shell interpolation" but don't address what happens if the shell already interprets the string.
- **Concurrency/atomicity of writes**: not mentioned.
- **Whether `--json` changes exit codes**: not mentioned.
- **Whether `8v fmt` modifies files in-place or prints diffs**: stated as "auto-format files in place" but no mention of a dry-run or diff mode.
- **Regex flavor for `search`**: not specified.
- **Line-ending behavior** (`\n` in write content): what happens on Windows?

---

## Q8: Undefined terms

- **"symbol map"** — described by example (fn, struct, impl) but never defined precisely. Does it cover: constants, macros, type aliases, closures, anonymous functions, enums, traits?
- **"stack"** — listed with valid values but never defined conceptually. An agent doesn't know whether `stack` means language, framework, or toolchain.
- **"progressive"** — used as a named principle but could mean "incremental," "layered," or "lazy."
- **"overhead"** / **"schema tax"** (two surfaces use different words for same thing) — not quantified.
- **"compact mode"** — not mentioned in either surface.
- **"most Bash"** — not present in either surface (question refers to possible phrasing; neither surface uses this exact term).
- **"symbol"** — what qualifies as a symbol in non-Rust languages (Python `def`, JS `function`, `class`, arrow functions)?

---

## Q9: Contradictions between surfaces

One difference:
- Surface 1 (ai_section.txt) writes `8v search <pattern> (regex) [path] [-i] [-e <ext>] [-C N] [--files] [--limit N]` and includes the default output format on the next line.
- Surface 2 (instructions.txt) writes `8v search <pattern> (regex) [path] [-i] [-e <ext>] [-C N] [--files] [--limit N]` followed by `Default output groups matches by file: <path>:<line>:<text>. --files lists only paths. -C N adds N context lines around each match.` — both surfaces agree.

Minor structural difference: Surface 1 uses bullet-list formatting for all sections; Surface 2 uses bare-text paragraphs. Substantively identical otherwise.

No factual contradictions found between the two surfaces. (Full comparison in Q30.)

---

## Q10: Batch read output — interleaving?

The instructions state `8v read a.rs b.rs Cargo.toml` is valid and "One call beats N sequential calls," but they do not describe the output format for multiple files. They do not say whether each file gets a header, whether results concatenate, or whether they are separated. An agent cannot know from the text alone. My guess: each file's output is clearly separated (e.g., a filename header), but this is not stated.

---

## Q11: Write content — trailing newline and multi-line

The instructions do not say whether `<content>` gets an automatic trailing newline. For multi-line content, Surface 1 states: `\n` becomes a newline. So multi-line: `"line1\nline2\nline3"`. The surrounding quotes are described as passed to 8v, not the shell — "do not rely on shell interpolation" — meaning the quotes are there to delimit the argument for the command invocation, but their shell-escape meaning is intentionally bypassed. Whether the shell still interprets them (e.g., on zsh) before 8v sees them is a gap: the instructions say to pass `\n` as literal two-character sequences, which would fail if the shell expands `\n` to a real newline first.

---

## Q12: Concrete commands + confidence

**a. Read 5 files at once.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs`
Confidence: 5 — explicitly described: "batch any combination of paths."

**b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.**
Command: `8v write foo.rs:10-20 "line1\nline2\nline3"`
Confidence: 3 — the range syntax is shown; multi-line via `\n` is stated; but whether the shell eats the backslashes before 8v sees them is unclear.

**c. Find all functions named `handle_*` across the repo.**
Command: `8v search "handle_" . --files` or `8v search "fn handle_" .`
Confidence: 3 — `search` supports regex but "regex" is labeled in the synopsis without explaining flavor or anchoring. The glob-style `handle_*` is a regex `.` in a regex engine, so the agent must know to use `handle_` (regex substring) not `handle_*`. Slight mismatch between the natural English phrasing and what the command actually needs.

**d. Append one line to `notes.md`.**
Command: `8v write notes.md --append "new line"`
Confidence: 5 — explicitly documented.

**e. Symbol map for `bar.rs`, then read lines 100–150.**
Command: `8v read bar.rs` then `8v read bar.rs:100-150`
Confidence: 5 — the instructions describe exactly this flow.

**f. Run tests and parse JSON output.**
Command: `8v test . --json`
Confidence: 4 — `--json` is documented as universal; shape of JSON is unknown.

**g. Check whether a file exists before reading.**
Command: Instructions don't cover existence checks. Would fall back to Bash: `test -f <path>`.
Confidence: 1 — 8v has no documented existence-check command.

**h. Delete lines 50–60.**
Command: `8v write foo.rs:50-60 --delete`
Confidence: 5 — explicitly documented.

**i. Insert a new line before line 30.**
Command: `8v write foo.rs:30 --insert "new line"`
Confidence: 5 — explicitly documented.

**j. Search only Rust files, case-insensitive, for `TODO`.**
Command: `8v search "TODO" . -i -e rs`
Confidence: 4 — `-e <ext>` and `-i` are in the synopsis; behavior is implied but not shown by example.

**k. Find all files by name matching `*_test*.md`.**
Command: `8v ls --match "*_test*.md"`
Confidence: 4 — `--match <glob>` is documented; behavior assumed to filter by filename glob.

**l. Run lint + format-check + type-check with one command.**
Command: `8v check .`
Confidence: 5 — explicitly stated: "lint + type-check + format-check."

**m. Replace `old_name` with `new_name` across a multi-file refactor.**
Command: `8v write <path> --find "old_name" --replace "new_name"` per file — but `--find/--replace` is per-file, not repo-wide. No documented multi-file replace. Would need to iterate per file or fall back to Bash.
Confidence: 2 — single-file only as documented; multi-file not addressed.

**n. Read just the symbols of 10 files in one call.**
Command: `8v read a.rs b.rs c.rs d.rs e.rs f.rs g.rs h.rs i.rs j.rs`
Confidence: 5 — batch is explicitly documented; default output is the symbol map.

**o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.**
Command: `8v read big.rs:1-200 big.rs:500-600`
Confidence: 5 — explicitly described: "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)."

---

## Q13: Teaching method per command

| Command | Method |
|---------|--------|
| `ls`    | Example (`8v ls --tree --loc`) + description |
| `read`  | Example (symbol map output) + description |
| `search`| Description + partial example (output format shown as `<path>:<line>:<text>`) |
| `write` | Description only (syntax shown, no worked examples with real content) |
| `check` | Description only |
| `fmt`   | Description only |
| `test`  | Description only |
| `build` | Description only |

---

## Q14: Three most likely mistakes

1. **Shell interpolation of backslashes in write content.** Passing `"line1\nline2"` to `8v write` via a shell that expands `\n` to a real newline before 8v processes it. The instructions warn against this but don't give a concrete safe invocation pattern (e.g., single-quotes, `$'...'`, or `--file`).

2. **Using `--find/--replace` for repo-wide refactors.** Instructions document it as per-file; an agent may attempt it without specifying a file path, expecting repo-wide substitution. The failure mode is not described.

3. **Assuming search returns line numbers in a stable format suitable for direct use in `8v read :<start>-<end>`.** The search output format `<path>:<line>:<text>` is stated, but whether the line number is 1-indexed and usable directly in a range is not explicitly confirmed.

---

## Q15: Fallback triggers

- File existence check before read (no 8v equivalent documented).
- Multi-file find-and-replace (8v write `--find/--replace` is per-file only).
- Git operations (explicitly excluded from 8v's scope).
- Process management (starting servers, killing PIDs).
- Environment variable inspection (`$HOME`, `$PATH`).
- Checking whether a command/binary is installed.
- Streaming output from a long-running process.
- Writing binary files.

---

## Q16: Most / least used commands

Most used: `read` (needed before almost every edit) and `write` (every edit). `ls` is needed at session start. `search` is frequent for cross-repo lookups.

Least used: `build` (implied by `check`; redundant in many flows) and `fmt` (can be folded into `check` or CI).

---

## Q17: First command in a new repo

`8v ls --tree --loc` — because the instructions say "Start here" and it returns the full hierarchy with line counts, giving a complete mental model of the project's layout before any other action.

---

## Q18: Trust `8v write` error messages?

Partially. The instructions state `--find` "fails if `<old>` not found" — so at least one failure mode is acknowledged. But the error format (message text, exit code, stdout vs. stderr) is not described. I would trust the error to signal that something went wrong but not necessarily to tell me what to do next. Confidence: low.

---

## Q19: Missing / wished for

- **Error output format** for every command (what goes to stdout/stderr, exit codes on success and failure).
- **JSON schema** for `--json` output — even one example.
- **Shell-quoting worked example** — a single concrete example of `8v write` with `\n` content invoked from a shell command line, showing the escaping correctly.
- **Existence check** — how to test whether a path exists before reading.
- **Multi-file find-replace** — either documented or explicitly noted as out of scope.
- **Regex flavor** for `search` (PCRE? RE2? basic?).
- **Symbol map coverage** — which constructs are included for each supported language.
- **What `--stack` means** conceptually (language? framework? toolchain?).
- **Glossary** for: symbol map, stack, progressive, batch, overhead/schema tax.

---

## Q20: Where I'd hesitate teaching 8v

- **Write content escaping** — I could not confidently teach the `\n` / `\t` / `\\` escaping without a concrete shell invocation example.
- **Search regex flavor** — I'd have to caveat that I don't know the engine.
- **Batch read output format** — I can't show what the output looks like when reading 3 files.
- **JSON shape** — I can say `--json` exists but not what it returns.
- **Not-found behavior** — I cannot say what happens if the path is wrong.

---

## Q21: 8v vs. native Bash+Read+Edit+Grep+Glob

**Better:**
- Batching (N files in one call vs. N tool invocations).
- Symbol-first reading (avoids sending full file content to the agent when only structure is needed).
- Progressive detail (agents get exactly what they need).
- Unified interface (one command set to remember instead of six tools).
- Explicit write semantics (range replacement, insert-before, delete — all documented unambiguously).

**Worse:**
- No existence check (Bash `test -f` is trivial; 8v has no equivalent).
- No multi-file find-replace (Bash + sed handles this easily).
- No streaming or interactive output.
- JSON shape opaque (native tools' output is well-known).
- Error format opaque (Bash exit codes and stderr are a known contract).

---

## Q22: One instruction edit for biggest positive impact

Add one worked example for `8v write` that shows multi-line content with explicit shell-safe quoting. The escaping caveat exists but gives no safe invocation pattern, which is the highest-friction point in the entire instruction set.

---

## Q23: Overall Clarity Scores

**Axis 1 — Input clarity: 8/10**
The instructions clearly document every command's input syntax with examples. Flags are listed. The only input gap is the shell-escaping ambiguity for write content and the undefined behavior for missing paths.

**Axis 2 — Output clarity: 4/10**
Symbol map output is shown by example (three lines). Search output format is stated. Batch read output format is not described. JSON shape is never shown. Verify commands give no sample output. The gap is wide for anything beyond read and search.

**Axis 3 — Failure-mode clarity: 2/10**
Only one failure is described explicitly (`--find` fails if not found). Exit codes (success and failure), stderr vs. stdout for errors, error message format, not-found behavior — all absent. This is the weakest axis.

**Composite mean: (8 + 4 + 2) / 3 = 4.67**

---

## Q24: One-minute improvement

Add a "Errors & exit codes" section (4 lines): success = exit 0, failure = exit non-zero, error text on stderr, `--json` wraps both output and errors in a consistent envelope. This single addition resolves the entire Axis 3 gap.

---

## Q25: Output contracts — util.py fixture

```python
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```

**a. `8v read util.py`**
Expected (based on symbol map description — line number + symbol per definition):
```
1  def add
4  result
```
Gap: instructions show Rust examples (`fn main`, `pub struct Args`, `impl Args`). Whether Python top-level assignments (`result = ...`) appear in the symbol map is not stated. I predict `def add` appears; `result` is uncertain. The exact format is `<line-number>  <symbol>` per the instructions.

**b. `8v read util.py:1-2`**
Expected: the raw file content of lines 1 and 2:
```
1  def add(a, b):
2      return a + b
```
Gap: line-number prefix format in range output is not documented (the `<line-number>  <text>` format shown for symbol maps may or may not apply to range output).

**c. `8v search "add" util.py`**
Expected format per instructions (`<path>:<line>:<text>`):
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
Gap: whether the pattern is literal string or regex, and whether full-line or just matched portion is returned.

**d. `8v read util.py --full`**
Expected: entire file content. The instructions do not describe the exact format (with or without line numbers). Gap: line-number prefix unknown.

---

## Q26: `8v check .` exit codes and error destination

**Exit code on lint error:** non-zero. The instructions state: "Non-zero exit on any issue." The specific non-zero value is not stated.

**Exit code on success:** 0 (implied — "non-zero on any issue" implies 0 on no issues). Not explicitly stated.

**Where error text appears:** Gap — the instructions do not say whether error text goes to stdout, stderr, or only in `--json` output. Quote: "Non-zero exit on any issue." That is the complete description.

---

## Q27: `8v write --find/--replace` behavior

**Zero occurrences:** The instructions state: "`8v write <path> --find \"<old>\" --replace \"<new>\"` — fails if `<old>` not found." So zero occurrences = failure. What "fails" means (exit code, error message, output) is a gap.

**More than one occurrence:** The instructions do not state this case. Gap — it is unknown whether all occurrences are replaced, only the first, or whether it also fails.

**What is returned to the caller:** Gap — not described.

---

## Q28: `8v read <path>` on nonexistent path

Gap — the instructions do not describe this case at all. No mention of: error on stdout, error on stderr, non-zero exit code, or structured JSON error. An agent has no basis from the instructions to predict the behavior.

---

## Q29: `8v check .` — exit 1, no stdout, no stderr

Based on the instructions: the only relevant text is "Non-zero exit on any issue." Exit code 1 with no output is therefore consistent with "an issue was found" per the documented contract. However, the instructions give no guidance on what the agent should do in this case. They don't say: retry, run with `--json`, or check a different output stream.

The agent should probably run `8v check . --json` to get structured output — but this is inference, not instruction. The instructions do not address this scenario.

---

## Q30: Surface 1 vs. Surface 2 — factual differences

| Item | Surface 1 (ai_section.txt) | Surface 2 (instructions.txt) | More complete |
|------|---------------------------|------------------------------|---------------|
| Opening sentence | "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations. For anything that reads, edits, searches, or inspects files, use 8v — not Bash. If the `8v` MCP tool is available, call it directly — do not shell out via Bash." | "Use `8v` for all file operations (read, edit, write, search, inspect). Use shell tools only for git, process management, and environment operations." | Surface 1 — more explicit about specific tools being replaced (Read, Edit, Write, Grep, Glob) and the MCP direct-call instruction. |
| `--stack` valid values | Listed explicitly: `rust, javascript, typescript, python, go, deno, dotnet, ruby, java, kotlin, swift, terraform, dockerfile, helm, kustomize, erlang` | Listed identically | Identical |
| `search` output format | Included in the `ls/search` section with a dedicated line: `Default output groups matches by file: <path>:<line>:<text>. --files lists only paths. -C N adds N context lines around each match.` | Same content present | Identical |
| `read` example output | Shows three example lines (`12  fn main`, `36  pub struct Args`, `58  impl Args`) | Identical | Identical |
| `write` content escaping | "Pass them as literal two-character sequences — do not rely on shell interpolation." | Identical | Identical |
| `verify` section | Bullet list format | Bare paragraph format | Surface 1 — marginally easier to scan |
| Write section header | `## Write` (no subtitle) | `## Write — prefer targeted edits` | Surface 2 — adds behavioral guidance in the header |
| Batch read note | "One call beats N sequential calls." | Not included in Surface 2 | Surface 1 — explicit payoff statement helps motivation |
| MCP direct-call | "If the `8v` MCP tool is available, call it directly — do not shell out via Bash." | Not present | Surface 1 — critical for MCP-aware agents |

No contradictions found. Surface 1 is strictly more complete in two areas: the MCP direct-call instruction and the explicit list of tools being replaced.

---

## Q31: Realistic task 8v cannot complete

**Task:** Rename a file from `old_name.rs` to `new_name.rs`.

**Missing capability:** 8v has no `mv` or rename command documented. `write` operates on existing file content; `ls` is read-only discovery. There is no documented way to create a new file at a new path or delete a file.

**Closest substitute:** Bash `mv old_name.rs new_name.rs`. Cost: requires a tool-switch to Bash (one extra call) and breaks the 8v-only discipline; the agent must context-switch and ensure the new path is consistent with the rest of the codebase.

---

## Q32: 5-step behavioral dry-run

**Step 1: Find all Go source files.**
Command: `8v ls --match "*.go" --stack go`
Confidence: 4 — `--match <glob>` and `--stack go` are documented. Uncertain whether both flags compose or only one is needed.

**Step 2: Search for `http.Get` across those files, 2 lines of context.**
Command: `8v search "http\.Get" . -e go -C 2`
Confidence: 3 — `-e <ext>` and `-C N` are documented. Uncertain whether `-e` takes the extension without dot (`go`) or with (`.go`). Also uncertain if `http.Get` needs regex escaping for the dot — regex flavor not documented.

**Step 3: Read the symbol map of the file with the most matches.**
Command: `8v read <path-with-most-matches>`
Confidence: 5 (for the read itself) — but to determine "which file had the most matches" requires counting the search output. This is a parsing step; 8v does not have a "count matches per file" flag. Would likely need to inspect search output manually or use `--json` if it includes counts. Partial fallback risk.

**Step 4: Replace `http.Get` with `httpClient.Get` on a specific line.**
Command: `8v write <path>:<line> "    httpClient.Get(url)"` (replace the specific line)
Or: `8v write <path> --find "http.Get" --replace "httpClient.Get"` (if only one occurrence in file)
Confidence: 3 — if `http.Get` appears more than once in the file, `--find/--replace` behavior for multiple occurrences is undocumented. Using line-replace is safer but requires knowing the exact replacement line content.

**Step 5: Run tests and confirm they pass.**
Command: `8v test .`
Confidence: 4 — documented. What "pass" looks like (exit code 0, output format) is partially inferred. Would check exit code = 0.

---

## Q33: Write syntax from memory (without re-reading)

**Insert a new line before line 42 of `main.rs`:**
```
8v write main.rs:42 --insert "new line content"
```
Confidence: 5 — this syntax was clearly stated.

**Replace lines 10–20 with multi-line content:**
```
8v write main.rs:10-20 "line one\nline two\nline three"
```
Confidence: 4 — the range syntax and `\n` escaping were both stated; the shell-quoting concern makes me slightly uncertain about real-world execution safety.

---

**Model & run ID:** Claude Sonnet 4.6 — sonnet-run2 — 2026-04-19

---

## 8v feedback

### What worked smoothly

The batch read call `8v read file1 file2 file3 --full` worked exactly as documented — all three files returned in one response, each with a clear `=== path/to/file ===` header. The MCP tool loaded and responded without error.

### Friction observed

**1. `--full` flag applies globally, not per-file in a batch call.**
Command: `8v read /path/a.txt /path/b.txt /path/c.txt --full`
What happened: All three files returned full content. This is the desired behavior here, but there is no way to say "full for file A, symbol map for file B" in one call. Expected: a per-file `--full` flag (e.g., `a.txt --full b.txt`). Not a bug, but a missing capability for mixed-detail batch reads.

**2. No documented way to test file existence.**
When writing Q12g, I had to fall back to Bash because `8v` has no existence-check command. Expected: `8v ls <path>` returning an error or empty result that is parseable; or a dedicated `8v exists <path>` command. The instructions don't address this at all.

**3. Output format for batch reads not in the instructions.**
The `=== path ===` separator that appears at runtime is not described in either surface. If I were an agent building a parser for `8v read` output, I would not know this format from the instructions alone.

**4. Shell escaping for write content — no safe example.**
The instructions warn not to rely on shell interpolation for `\n`, but give no safe invocation pattern. Writing multi-line content in a real shell session would require trial and error. Expected: one concrete shell example with proper quoting (e.g., single-quotes or `$'...'` syntax).

**5. `--json` shape unknown.**
Every command accepts `--json` but no example output is shown. When I wrote Q25-Q28, I could not predict what `--json` returns. This makes `--json` nearly unusable without prior experimentation.

**Overall:** 8v worked smoothly in this session (one call, three files, correct output). The friction points are all documentation gaps, not runtime bugs.
