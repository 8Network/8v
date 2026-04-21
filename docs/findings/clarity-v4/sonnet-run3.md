# Instruction Clarity v4 — Sonnet Run 3 — 2026-04-20
Model: Claude Sonnet 4.6

---

### Q1: summary
8v is a code reliability tool for AI agents that replaces native file operations (Read, Edit, Write, Grep, Glob, Bash) with a single unified CLI. Use it for all file reads, edits, searches, and inspections; use shell tools only for git, process management, and environment. The two governing principles are Progressive (default output is minimal; flags escalate detail) and Batch (pass many inputs in one call to amortize per-call overhead).

---

### Q2: minimum invocations

- `ls`: `8v ls --tree --loc` — full file hierarchy with line counts
- `read`: `8v read <path>` — symbol map (functions, structs, classes)
- `search`: `8v search <pattern>` — matches grouped by file as `<path>:<line>:<text>`
- `write`: `8v write <path>:<line> "<content>"` — replaces a single line
- `check`: `8v check .` — lint + type-check + format-check, non-zero exit on issue
- `fmt`: `8v fmt .` — auto-format in place
- `test`: `8v test .` — run project tests
- `build`: `8v build .` — compile

---

### Q3: two principles

**Progressive**: default output is the minimum useful answer; flags escalate detail. Non-obvious example: `8v read a.rs` returns only a symbol map (line numbers + names), not any code body — an agent must use `:start-end` to see the actual implementation. The default is intentionally less than what most tools show.

**Batch**: pass many inputs in one call rather than looping. Non-obvious example: `8v read a.rs:1-200 a.rs:500-600` — two disjoint ranges of the same file in a single call. This is not possible with native cat/head; here it's a first-class feature that halves round-trips.

---

### Q4: when to use 8v vs. native

**Use 8v** for: reading files (symbol map or ranges), searching content or file names, writing/editing/deleting lines, running lint/tests/builds/format.

**Do NOT use 8v** (use Bash instead) for: git commands, process management (starting/killing processes), environment operations (env vars, PATH, etc.).

---

### Q5: discovering flags

Run `8v <cmd> --help` — stated explicitly: "Run `8v <cmd> --help` for the full flag list." Every command also accepts `--json`.

---

### Q6: ambiguous phrases

1. **"Use Bash only for git, process management, and environment operations"** — "environment operations" is undefined. Could mean: setting env vars, sourcing files, inspecting `$PATH`, or anything OS-level. Dual reading: (a) only env-var manipulation; (b) anything the OS exposes that 8v doesn't cover.

2. **"Pass them as literal two-character sequences"** (re: `\n`, `\t`) — ambiguous about quoting. Dual reading: (a) you must pass the string with a backslash-n literally, relying on 8v to interpret it; (b) shell escaping is your responsibility and these are just examples.

3. **"One call beats N sequential calls"** — "beats" is vague. Could mean: (a) one call is always faster; (b) one call is preferred for token/latency efficiency but not always required.

4. **"--full applies to every positional arg (repeats accepted, no-op beyond the first)"** — "repeats accepted, no-op beyond the first" — repeats of what? The `--full` flag itself, or the file path? Dual reading: (a) passing `--full` twice is harmless; (b) listing the same file twice is a no-op.

5. **"Last resort"** (for `--full`) — does this mean 8v will refuse `--full` if a symbol map suffices? Or just an instruction to agents? Dual reading: (a) normative guidance only; (b) the tool may throttle or warn.

6. **"fails if `<old>` not found"** — "fails" meaning exit code, error message, or exception? Unclear what the failure surface looks like.

---

### Q7: implied but never stated

- Error output format: where do errors appear (stdout vs. stderr)? Not stated.
- Exit codes: non-zero on failure for `8v check .` is stated; for all other commands, not stated.
- What happens when a path doesn't exist: not stated for any command.
- JSON error envelope: `--json` shapes for success described; error JSON shape not described.
- Whether `8v write` is atomic (all-or-nothing on multi-line replace): not stated.
- Whether `8v search` respects `.gitignore`: not stated in Surface 1 or 2 (Surface 2 omits this entirely; a third surface in system-reminder mentions it, but that's not under test).
- Output stream for verify commands (`8v test .`, `8v build .`): stdout pass-through, suppressed, or only in `--json`? Not stated.
- Whether `--json` disables human-readable output or adds a parallel stream: implied replace, not confirmed.
- Unicode support: not mentioned.
- Whether `8v write` with a range replaces with exactly the given lines (newline-terminated) or joins with a trailing newline.
- Default `--limit` for `8v ls` and `8v search`: stated for search (default 20 files) but not for ls.
- Whether `8v search` without `-C` returns any source text or just metadata.

---

### Q8: undefined terms

- **"symbol map"**: used without definition. Inferred from example (`fn main`, `pub struct Args`) to mean a list of top-level definitions with line numbers. No explicit definition given.
- **"stack"**: defined implicitly by the list of valid values but never defined conceptually (language ecosystem? project type?).
- **"progressive"**: defined as "minimum useful answer" — but "useful" is subjective and not grounded.
- **"overhead"** (in "each call costs overhead"): not quantified. Token cost? Latency? Both?
- **"compact mode"**: mentioned in the system-reminder surface (outside the two surfaces under test) for `search` without `-C`. Not defined in either surface under test.
- **"schema tax"**: not present in either surface under test.
- **"most Bash"**: not present; the instruction says "shell tools only for git, process management, and environment operations."

---

### Q9: contradictions between surfaces

Surface 1 (ai_section.txt / CLAUDE.md):
- Uses bullet list format with `-` for all commands.
- Explicitly says "Use Bash only for git, process management, and environment operations."

Surface 2 (instructions.txt / MCP description):
- Uses bare lines (no `-` bullets) for commands — same content, different formatting.
- First line says "Use shell tools only for git, process management, and environment operations." — synonymous but slightly different phrasing.
- Surface 2 omits "One call beats N sequential calls" sentence from the batch section.
- Surface 2 omits the `8v read a.rs b.rs c.rs --full` clarification line that appears in Surface 1.
- Surface 1 has explicit note: "Batch output contract: each file is preceded by `=== <label> ===`..." — Surface 2 has the same content but compressed differently.

No factual contradictions found; differences are presentational and one missing sentence in Surface 2.

---

### Q10: batch read output interleaving

The instructions state: "each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges. Single-file reads emit no header."

This means output is **concatenated with delimiters**, not interleaved. Each file's content follows its `=== label ===` header sequentially. I know this from the explicit batch output contract in both surfaces.

---

### Q11: write content and newlines

- **Trailing newline**: not stated. Instructions don't say whether 8v appends a trailing newline to replaced content.
- **Multi-line content**: use `\n` as a literal two-character sequence inside the content argument; 8v parses it as a newline. Example: `8v write foo.rs:10-12 "line1\nline2\nline3"`.
- **Quotes**: "do not rely on shell interpolation" — the surrounding quotes are shell quoting to pass the argument; 8v does its own parsing of the content string. Shell escape meaning is explicitly disclaimed.

---

### Q12: scenarios

a. Read 5 files at once.
   Command: `8v read a.rs b.rs c.rs d.rs e.rs`
   Confidence: 5 — explicitly documented as batch behavior.

b. Replace lines 10–20 of `foo.rs` with new content spanning 3 lines.
   Command: `8v write foo.rs:10-20 "line1\nline2\nline3"`
   Confidence: 4 — range replace is documented; multi-line via `\n` is documented. Slight uncertainty on whether trailing newline is added automatically.

c. Find all functions named `handle_*` across the repo.
   Command: `8v search "handle_\w+" --files` or `8v search "fn handle_"`
   Confidence: 3 — regex support implied by "(regex)" in search signature, but no explicit regex example for function names. `--files` returns file paths only; omit for line content.

d. Append one line to `notes.md`.
   Command: `8v write notes.md --append "new line"`
   Confidence: 5 — documented explicitly.

e. Symbol map for `bar.rs`, then read lines 100–150.
   Command: `8v read bar.rs` then `8v read bar.rs:100-150`
   Confidence: 5 — the exact workflow described in "symbol map first, range second."

f. Run tests and parse JSON output.
   Command: `8v test . --json`
   Confidence: 4 — `--json` flag is stated for all verify commands. JSON schema for test output not documented, so parsing requires guessing field names.

g. Check whether a file exists before reading.
   Command: Instructions don't say — my guess: `8v ls --match filename` or attempt `8v read` and handle error. No explicit "file exists" check documented.
   Confidence: 1 — gap.

h. Delete lines 50–60.
   Command: `8v write foo.rs:50-60 --delete`
   Confidence: 5 — documented explicitly.

i. Insert a new line before line 30.
   Command: `8v write foo.rs:30 --insert "new line content"`
   Confidence: 5 — documented explicitly.

j. Search only Rust files, case-insensitive, for `TODO`.
   Command: `8v search "TODO" -e rs -i`
   Confidence: 5 — both flags documented.

k. Find all files by name matching `*_test*.md`.
   Command: `8v ls --match "*_test*.md"`
   Confidence: 4 — `--match <glob>` is documented for `ls`. Works at the ls level, not search. Slightly uncertain whether it's a content search or name filter; context implies name filter.

l. Run lint + format-check + type-check with one command.
   Command: `8v check .`
   Confidence: 5 — documented as "lint + type-check + format-check."

m. Replace `old_name` with `new_name` across a multi-file refactor.
   Command: Instructions don't say — `--find/--replace` operates on one file at a time. For multi-file: loop with `8v write <file> --find "old_name" --replace "new_name"` per file, or use `8v search` to identify files first.
   Confidence: 2 — no multi-file find/replace documented; would need to script the loop or fall back to Bash sed.

n. Read just the symbols of 10 files in one call.
   Command: `8v read a.rs b.rs c.rs d.rs e.rs f.rs g.rs h.rs i.rs j.rs`
   Confidence: 5 — default read is symbol map; batch is documented.

o. Read lines 1–200 and lines 500–600 of `big.rs` in one call.
   Command: `8v read big.rs:1-200 big.rs:500-600`
   Confidence: 5 — explicitly documented: "multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`)."

---

### Q13: teaching method per command

| Command | Method |
|---------|--------|
| `ls`    | Example (`8v ls --tree --loc`) + description |
| `read`  | Example (symbol map output, range syntax, batch) + description |
| `search`| Example (flag list) + description of output format |
| `write` | Example (6 variants) + description of content parsing |
| `check` | Description only — "lint + type-check + format-check" |
| `fmt`   | Description only — "auto-format files in place. Idempotent." |
| `test`  | Description only — "run project tests" |
| `build` | Description only — "compile" |

---

### Q14: three most likely mistakes

1. **Using `\n` with shell interpolation instead of literal two-character sequence.** The instructions say "do not rely on shell interpolation" but most agents will write `"line1\nline2"` expecting the shell to expand it. In double quotes in bash, `\n` is NOT expanded, so this may accidentally work — but relying on shell behavior is exactly what the instructions warn against. Risk: inconsistent behavior across shells.

2. **Calling `8v write` with a range for multi-file find/replace.** The `--find/--replace` form only operates on one file. An agent trying to rename a symbol across the codebase will apply it to one file, miss the rest, and not realize the gap because no error is raised for other files.

3. **Expecting `8v read` (symbol map) to show function bodies.** The symbol map shows only definitions and line numbers. An agent may try to read logic from a symbol map output and miss that it needs a second `:start-end` call to see the implementation. The "symbol map first" instruction implies two calls, but agents used to tools that return full content by default may skip the second.

---

### Q15: fallback triggers

I would fall back to Bash/native tools when:
- Checking if a file exists (no documented 8v command for existence check).
- Multi-file find/replace across an entire repo in one operation.
- Reading a file's error output (stderr) separately from stdout.
- Running git operations (explicitly delegated to Bash).
- Process management: starting/stopping services.
- JSON field introspection when `--json` output schema is undocumented.

---

### Q16: most/least used

**Most used**: `8v read` (symbol map + ranges) and `8v write` — the core editing loop. `8v search` for navigation. `8v check .` after every change.

**Least used**: `8v build .` — only needed when compilation is uncertain. `8v fmt .` — usually run once at the end. `8v ls` — used at session start, rarely repeated.

---

### Q17: first command in a new repo

`8v ls --tree --loc` — gives the full file hierarchy with line counts in one call. The instructions explicitly say "Start here." It orients me to project structure, stack, and file sizes before any targeted read.

---

### Q18: trusting write error output

Partially. The instructions state `--find/--replace` "fails if `<old>` not found" — so I know a failure signal exists for that case. But the failure format (exit code, message text, stderr vs stdout) is unspecified. I would trust that a failure signal is emitted but would not know how to parse or act on it without trial. Confidence: 2/5 on actionability of error output.

---

### Q19: what's missing

- **Error output format**: what does a failure look like? Exit code? Stderr text? JSON `{"error": ...}`? None specified.
- **File-not-found behavior**: for every command, unspecified.
- **`--json` error schema**: success JSON shapes documented; error JSON not.
- **Multi-file find/replace**: no documented path.
- **`8v search` without `-C`**: instructions say compact mode exists (system-reminder surface) but neither surface under test defines what compact mode output looks like.
- **Glossary**: "symbol map," "stack," "progressive" need definitions.
- **Range write newline semantics**: does the replacement include a trailing newline per line?
- **`8v ls` default (no flags)**: what does bare `8v ls` return? Only `--tree --loc` is shown.

---

### Q20: teaching hesitation points

1. "Symbol map" — I would have to define what it means before showing the example, or students would think it returns code.
2. Multi-line write — the `\n` as literal sequence vs. shell behavior is confusing; I'd need a concrete before/after example.
3. When to batch vs. loop — the principle says to batch, but for write operations, no batch form exists. Students would ask "can I batch writes?"
4. `--find/--replace` failure behavior — I can't explain what the agent sees when it fails.
5. MCP vs. shell invocation — "call it directly" vs. shell out is mentioned but the practical difference for an agent is not illustrated.

---

### Q21: 8v vs. native — better and worse

**Better**:
- Batch reads in one call (native requires N separate cat/head calls).
- Symbol map without grep (native requires ctags or language-specific tooling).
- Unified verify pipeline (`8v check .` = lint + type + format in one).
- Consistent `--json` output across all commands.

**Worse**:
- No multi-file atomic write operation (native sed -i with glob covers this).
- No existence check command (native: `[ -f file ]`).
- Unknown error surface makes recovery harder than with well-documented CLI tools.
- No streaming output for long-running test/build (presumably).

---

### Q22: highest-impact single edit

Add a **Failure behavior** section that documents: (1) exit codes for each command, (2) where errors appear (stdout vs. stderr), and (3) the JSON error envelope shape. This is the single largest gap — agents can't recover from errors they can't parse.

---

### Q23: clarity ratings

**Axis 1 — Input clarity** (what to pass in): **8/10**
The command syntax is well-documented with concrete examples for nearly every form. The `write` variants are enumerated. The main gap is that `<content>` escaping rules are explained but the newline/trailing-newline semantics for range replacements aren't pinned down.

**Axis 2 — Output clarity** (what comes back on success): **6/10**
The symbol map format is shown with example output. Batch delimiter (`=== label ===`) is specified. JSON shape for read is given. But: `test`, `build`, `fmt`, `check` output formats are entirely undocumented. The `search` compact-mode output isn't described in the surfaces under test. Nearly half the commands have opaque success output.

**Axis 3 — Failure-mode clarity** (what happens when something goes wrong): **2/10**
Only one failure mode is documented: `--find/--replace` fails if `<old>` not found. No exit codes (except non-zero for `check .`). No error format. No file-not-found behavior. No indication of stderr vs. stdout. This is the dominant gap.

**Composite mean**: (8 + 6 + 2) / 3 = **5.33**

---

### Q24: one-minute improvement

Add a two-line **Failure** section at the end of each command block:
```
Error: non-zero exit + message on stderr. --json: {"error": "<message>"}.
```
This single pattern, repeated consistently, would close the failure-mode gap from Axis 3 (score 2 → ~7) and raise the composite from 5.33 to ~7.

---

### Q25: output contract prediction — util.py

```python
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```

a. `8v read util.py` — returns a symbol map. Predicted output:
```
1  def add
4  result
```
(Line numbers pointing to definitions; exact format: `<line-number>  <symbol>`. Gap: whether module-level assignments like `result = add(1, 2)` appear as symbols is not guaranteed — instructions only show `fn`, `struct`, `impl` examples. My guess: `result` may not appear as a symbol.)

b. `8v read util.py:1-2` — returns the line range, 1-indexed, inclusive:
```
# util.py  (4 lines)
def add(a, b):
```
Gap: instructions don't specify whether the original file's comment header is included or whether output is raw lines. Predicted as raw lines from the file.

c. `8v search "add" util.py` — returns matches grouped:
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
Format: `<path>:<line>:<text>`. Both occurrences of "add" would match (regex). Gap: whether the search is literal or regex by default — instructions say "(regex)" suggesting regex, so `add` as a literal pattern would match both lines.

d. `8v read util.py --full` — returns entire file content:
```
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```
No `=== label ===` header for single-file reads (stated explicitly).

---

### Q26: `8v check .` exit codes

- **Lint error found**: "Non-zero exit on any issue" — stated. Specific code not given.
- **Success**: implied exit 0 by contrast, not explicitly stated.
- **Error text location**: not stated. Instructions don't say whether output goes to stdout, stderr, or only in `--json`.
- **Gap**: no documentation of where error text appears or what the non-zero code value is.

Quoted: "Non-zero exit on any issue."

---

### Q27: `8v write --find/--replace` edge cases

- **Zero occurrences**: "fails if `<old>` not found" — stated. What "fails" means (exit code, message, format) is not stated.
- **More than one occurrence**: Instructions don't say — my guess: replaces all occurrences (typical find/replace semantics) or fails with an ambiguity error. Gap: not documented.
- **Return to caller**: not stated beyond "fails." No success return value described either.

---

### Q28: `8v read` on nonexistent path

Instructions don't say. Gap: not documented. My guess: non-zero exit with an error message on stderr, possibly `{"error": "..."}` under `--json`. No basis in the instructions to confirm.

---

### Q29: `8v check .` — no output, exit code 1

Based only on the instructions: this is **partially expected** — "non-zero exit on any issue" is documented — but the absence of any output on both stdout and stderr is not addressed. The instructions don't describe where error output appears, so this scenario falls into a gap.

What the agent should do: the instructions say "Non-zero exit on any issue" but don't say what to do when output is empty. My inference: try `8v check . --json` to get structured output, or run `8v check .` with a specific path to narrow scope. But this is not stated — it's a gap.

Relevant quoted text: "Non-zero exit on any issue."

---

### Q30: Surface 1 vs. Surface 2 differences

1. **Formatting**: Surface 1 uses bullet (`-`) lists; Surface 2 uses bare lines. Functionally identical but visually different. Matters if an agent uses formatting as a signal.

2. **Missing sentence in Surface 2**: Surface 1 includes "One call beats N sequential calls." after the batch read example; Surface 2 omits this reinforcement sentence. Surface 1 is more complete here — without it, the rationale for batching is implicit.

3. **Missing line in Surface 2**: Surface 1 includes: "`8v read a.rs b.rs c.rs --full` — full content of multiple files in one call. `--full` applies to every positional arg (repeats accepted, no-op beyond the first)." Surface 2 includes this. Both have it — confirmed present in both.

4. **Header phrasing**: Surface 2 opens with a one-line tagline: "8v — code reliability tool for AI agents. Designed to minimize round-trips." Surface 1 has no such tagline. Surface 2 is more complete here for orientation.

5. **Write section header**: Surface 1 uses "## Write"; Surface 2 uses "## Write — prefer targeted edits". Surface 2's header is more prescriptive and guides behavior.

Net: no factual contradictions. Surface 1 has one additional reinforcement sentence for batch rationale. Surface 2 has a stronger orientation tagline and a more behavioral write header. An agent seeing only Surface 2 loses one explicit rationale sentence but gains orientation context.

---

### Q31: task 8v cannot complete

**Task**: Rename a function across 15 files simultaneously with a preview of all changes before committing.

**Missing capability**: `8v write --find/--replace` operates on one file at a time. There is no multi-file batch write, no dry-run/preview flag, and no transactional "apply all or none" operation documented.

**Closest substitute**: Run `8v search "old_name" --files` to find affected files, then loop `8v write <file> --find "old_name" --replace "new_name"` for each. Cost: N round-trips (one per file), no atomicity, no preview, potential partial state if one write fails mid-loop.

---

### Q32: 5-step dry-run — Go repo, `http.Get`

**Step 1**: Find all Go source files.
Command: `8v ls --stack go --match "*.go"`
Confidence: 4. `--stack go` is documented; `--match` is documented. Uncertain whether `--stack` and `--match` can combine.

**Step 2**: Search for `http.Get` across Go files with 2 lines of context.
Command: `8v search "http\.Get" -e go -C 2`
Confidence: 4. `-e go` filters by extension; `-C 2` adds 2 context lines. Regex escaping of `.` is my interpretation of "(regex)" in the signature.

**Step 3**: Read symbol map of the file with the most matches.
Command: `8v read <path-from-step2>`
Confidence: 5 for the read itself. Confidence 1 for identifying "most matches" — must count manually from Step 2 output; no `--sort-by-count` flag documented.

**Step 4**: Replace `http.Get` with `httpClient.Get` on a specific line.
Command: `8v write <path>:<line> "    httpClient.Get(url)"` (replacing the exact line) or `8v write <path> --find "http.Get" --replace "httpClient.Get"`
Confidence: 4. `--find/--replace` is simpler but replaces all occurrences in the file — may be too broad. Line-specific replace is safer but requires knowing the full line content.

**Step 5**: Run tests and confirm they pass.
Command: `8v test .`
Confidence: 5. Documented. Gap: no documented way to confirm "pass" vs. "fail" from the output without knowing exit code semantics or output format. Inference: exit 0 = pass.

---

### Q33: memorability — write syntax from memory

**Insert before line 42 of `main.rs`**:
`8v write main.rs:42 --insert "new line content"`
Confidence: 5 — the `--insert` flag and "insert before a line" phrasing are memorable and unambiguous.

**Replace lines 10–20 with multi-line content**:
`8v write main.rs:10-20 "line1\nline2\nline3"`
Confidence: 4 — the range syntax is clear; slight uncertainty on whether `\n` as literal two-character sequence works correctly in all shells vs. needing quoting adjustments.

---

Model & run ID: Claude Sonnet 4.6 / sonnet-run3 / 2026-04-20

---

## 8v feedback

**Friction encountered during this session**:

1. **No output directory confirmation needed** — `8v ls` or a lightweight existence check before writing would have been useful; had to fall back to Bash `ls` to verify the `clarity-v4/` directory existed. Gap: 8v has no file-existence check command.

2. **Batch read worked as expected** — reading all three source files in parallel via native Read (since 8v MCP was not called for this session) was smooth. If using the MCP tool, a single `8v read` call with all three paths would have been the right move.

3. **No friction on write** — single Write call to the target path worked cleanly. No retry needed.
