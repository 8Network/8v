# Instruction Clarity v3 — Opus Run 3 — 2026-04-19
Model: Claude Opus 4.7
Surfaces read: ai_section.txt, instructions.txt

---

### Q1: summary-3-sentences
8v is a code reliability tool for AI agents that replaces native file operations (Read, Edit, Write, Grep, Glob, and file-inspecting Bash) with a single `8v` CLI / MCP interface, leaving only git / process / env work for the shell. You use it for every read, edit, search, or inspection of files, and you reach for Bash only when 8v doesn't cover it (git, processes, environment). The two principles are Progressive (minimum-useful output by default; flags escalate detail) and Batch (pass many inputs in one call to amortize overhead).

### Q2: minimum-viable invocations
- `8v ls` — returns a file listing of the current directory (default view; `--tree --loc` is the recommended first call but not required).
- `8v read <path>` — returns a symbol map (`<line-number>  <symbol>`).
- `8v search <pattern>` — returns matches grouped by file as `<path>:<line>:<text>`.
- `8v write <path>:<line> "<content>"` — replaces a single line; instructions don't say what it returns on success — my guess: nothing / an "ok" line.
- `8v check .` — runs lint + type-check + format-check; non-zero exit on issues. Output format not stated.
- `8v fmt .` — auto-formats files in place; idempotent. Return not stated.
- `8v test .` — runs project tests. Output not stated.
- `8v build .` — compiles. Output not stated.

### Q3: two-principles-explained
1. **Progressive** — every command starts by returning the smallest answer that is actually useful, and you opt into more with flags. Non-obvious example: `8v read foo.rs` does NOT dump the file; it gives you a symbol map you use to pick line ranges, so the "default" of a read command is fundamentally different from `cat`.
2. **Batch** — pass N inputs to one invocation instead of N invocations, because each call has overhead. Non-obvious example: you can mix distinct files and multiple ranges of the same file in one call, e.g. `8v read a.rs:1-200 a.rs:400-500 b.rs`.

### Q4: when-use-8v
Use 8v for anything that reads, edits, searches, or inspects files in the workspace. Don't use it for git, process management, or environment operations — those stay in Bash. If the 8v MCP tool is present, call it directly and never shell out.

### Q5: discover-flags
Run `8v <cmd> --help`. Instructions state this explicitly.

### Q6: ambiguity-quotes
- "Use Bash only for git, process management, and environment operations." — Reading A: every non-file Bash call is fine. Reading B: only exactly these three categories are fine; everything else is disallowed.
- "Default output is the minimum useful answer." — Reading A: a specific defined minimal shape per command. Reading B: a vibes-based "smallest thing we thought was useful," which differs per command.
- "All verify commands accept `--json` and run on the whole project by default. Pass a path to scope to a subtree." — Reading A: every verify command treats a trailing path as a scope argument. Reading B: only `.`-style paths are supported; arbitrary globs unclear.
- "Content arguments are parsed by 8v (not the shell)" — Reading A: 8v re-parses the string after the shell already processed quotes. Reading B: 8v receives the raw bytes untouched. The escape rules are stated, but who owns quoting in an MCP call vs a shell call is not.
- "Run `8v <cmd> --help` for the full flag list." — Reading A: help exists for every subcommand. Reading B: only for commands that are documented above.
- "`8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found." — Reading A: silent success on multiple matches. Reading B: multiple matches also fail. Undefined.

### Q7: implicit-never-stated
- Exit codes for every command except `check` (which just says "non-zero on issue").
- Whether output goes to stdout or stderr.
- Error message format.
- What happens when a path does not exist.
- What happens when a line number is out of range.
- Whether `8v write` creates missing parent directories.
- Whether `8v write` writes a trailing newline.
- Behavior on binary files.
- Unicode handling.
- Atomicity of writes (partial failure safety).
- JSON schema shape for `--json`.
- How batched `8v read` interleaves output (only a visual hint in the example).
- Concurrency / locking.
- What `8v search` regex flavor is (PCRE? RE2? Rust regex?).
- What line endings `\n` expands to on Windows.
- Whether `8v fmt` respects `.gitignore` / workspace config.
- Whether commands honor `CARGO_MANIFEST_DIR` / cwd / workspace root.
- Whether `--json` changes exit-code semantics.

### Q8: undefined-terms
- "symbol map" — format is shown by example, but what counts as a "symbol" (only top-level? methods? macros? types?) is unstated.
- "stack" — a controlled vocabulary is listed but the concept is never defined; is it language, framework, or build system?
- "progressive" — a principle name; meaning inferred.
- "overhead" — unquantified.
- "project" (as in `8v test .`) — how is "project" detected? Unstated.
- "batch" — term reused in two senses: multi-arg and one-call-many-files.
- "subtree" — undefined relative to workspace roots.
- "idempotent" (for `fmt`) — assumed CS meaning, but whether it applies to output or just filesystem state is unclear.

### Q9: contradictions
Mild divergences rather than hard contradictions:
- Surface 1 says "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations." Surface 2 says "Use `8v` for all file operations (read, edit, write, search, inspect)." Surface 1 enumerates native tools; Surface 2 enumerates verbs. Same intent, different vocabulary.
- Surface 1 has a `Write` subsection; Surface 2 titles it `Write — prefer targeted edits` with an extra nudge.
- Surface 2 drops the closing bullet "One call beats N sequential calls" from the read-batching example.
- Search description is identical; no contradiction.
No hard factual conflict between the two.

### Q10: batch-read-output-shape
Instructions don't say. The text says "batch any combination of paths and ranges in one call" but never describes how the output of multiple files is separated, interleaved, or framed. My guess: one symbol map (or range) per file, printed sequentially with some header like `=== path ===`, but this is not stated.

### Q11: write-content-semantics
Instructions don't say whether a trailing newline is added. Multi-line content is handled by passing `\n` as a literal two-character sequence which 8v converts to newline. The surrounding quotes in the docs are shell quotes (so shell escape rules apply at invocation time), but 8v itself does NOT interpret shell escapes — it parses `\n`, `\t`, `\\` on its own. Trailing-newline behavior is a gap — my guess: no implicit trailing newline is added.

### Q12: scenarios
a. **Read 5 files at once** — `8v read a b c d e` — confidence 5 — batching is the documented pattern.
b. **Replace lines 10-20 with 3-line content** — `8v write foo.rs:10-20 "line1\nline2\nline3"` — confidence 4 — range replace + `\n` escape documented; trailing-newline behavior unclear.
c. **Find functions named `handle_*`** — `8v search "fn handle_\w+" .` — confidence 3 — regex flavor unspecified; I'm guessing Rust-style `\w+`.
d. **Append one line to `notes.md`** — `8v write notes.md --append "new line"` — confidence 4 — documented flag; trailing-newline behavior unclear.
e. **Symbol map then lines 100-150 of bar.rs** — `8v read bar.rs` then `8v read bar.rs:100-150` (or batched `8v read bar.rs bar.rs:100-150`) — confidence 4 — documented.
f. **Tests + JSON** — `8v test . --json` — confidence 3 — docs say every verify command accepts `--json`, but the JSON schema isn't shown.
g. **Check whether a file exists before reading** — Instructions don't say — my guess: run `8v read path` and inspect non-zero exit / error message, or use `8v ls --match <path>`. I'd likely fall back to Bash `test -f`.
h. **Delete lines 50-60** — `8v write foo.rs:50-60 --delete` — confidence 5 — documented.
i. **Insert before line 30** — `8v write foo.rs:30 --insert "new line"` — confidence 5 — documented.
j. **Rust files, case-insensitive, TODO** — `8v search "TODO" . -i -e rs` — confidence 4 — flags are listed but `-e rs` vs `-e .rs` is a guess.
k. **Files matching `*_test*.md`** — `8v ls --match "*_test*.md"` — confidence 4 — documented `--match <glob>`.
l. **Lint + format-check + type-check in one** — `8v check .` — confidence 5 — explicitly described.
m. **Multi-file refactor `old_name` → `new_name`** — Instructions don't say — `8v write` `--find`/`--replace` only documents single-path usage; I'd fall back to a shell loop over `8v search --files` + `8v write --find --replace`, or native tools. Confidence 2.
n. **Symbols of 10 files in one call** — `8v read a b c d e f g h i j` — confidence 5.
o. **Lines 1-200 and 500-600 of big.rs in one call** — `8v read big.rs:1-200 big.rs:500-600` — confidence 5 — explicit example.

### Q13: example-vs-description
- `ls` — example (`8v ls --tree --loc`) + description.
- `read` — example + description.
- `search` — description-only with flag list; no concrete output beyond format string.
- `write` — multiple examples + description.
- `check` — description-only.
- `fmt` — description-only.
- `test` — description-only.
- `build` — description-only.

### Q14: three-most-likely-mistakes
1. Forgetting to use `--full` and writing code against the symbol map, missing lines I thought I'd seen. (The default output isn't "the file.")
2. Passing multi-line content to `8v write` with actual shell newlines in the quoted string instead of literal `\n`, producing a malformed edit because I forgot 8v parses the escapes, not the shell.
3. Using the wrong regex flavor in `8v search` (posix vs PCRE vs RE2) and getting empty results, then re-running 2-3 times.

### Q15: fallback-triggers
- I need to check file existence, permissions, or stat.
- I need to run git, env, or a process.
- I need to edit binary files.
- `8v write --find --replace` only handles one file and I need a multi-file refactor.
- I need to chain output with Unix pipes to a tool 8v doesn't expose.
- Regex flavor uncertainty after one failed search.

### Q16: most-least-used
Most: `8v read` (symbol map + ranges) and `8v search` — every task starts with understanding the code. Least: `8v build` — in Rust/TS workflows `8v test` typically implies build; explicit build is rare. `8v fmt` also rare because `8v check` likely runs format-check already.

### Q17: first-command
`8v ls --tree --loc` — instructions literally say "Start here," it shows the full hierarchy plus line counts so I can pick where to read next.

### Q18: trust-write-error
Partially. Instructions only give one explicit error contract — `--find`/`--replace` "fails if `<old>` not found" — everything else is silent. So I'd trust a clear message when it exists but expect to inspect and guess on less-covered failures (out-of-range lines, perms, missing parents).

### Q19: missing-wished
- Exit code table.
- JSON schema for every command.
- Explicit newline semantics on `write` (trailing newline? CRLF?).
- Behavior on missing paths / out-of-range lines.
- Regex flavor for `search`.
- How batch-read output is delimited.
- A glossary of "symbol," "stack," "project," "subtree."
- An example for every command, not just `ls`, `read`, `write`.
- Multi-file refactor example.
- `--help` output shape hint.

### Q20: teaching-hesitations
I'd hesitate at: (a) explaining why `8v read` doesn't dump the file — the "symbol map first" mental model is different from `cat` and learners will bounce off it; (b) explaining `\n` vs shell newline in `write` content — one of the most error-prone things here; (c) explaining when to keep reaching for Bash because the boundary is under-specified.

### Q21: 8v-vs-native
Better: one consistent interface, batching reduces round-trips, progressive output reduces token waste, symbol-map-first read is strictly more efficient than `cat`, project-level verify in one command.
Worse: less precise error contracts than `Read`/`Edit` which have well-known failure modes; regex flavor and JSON schema are implicit; no obvious "does this file exist" probe; single-file `--find --replace` is weaker than Grep+sed pipelines.

### Q22: one-edit-biggest-impact
Add a short "Contracts" section: exit codes, stdout vs stderr, JSON shape, and failure modes for each command (missing path, out-of-range line, zero/multi matches). That one table would remove most of my guesswork.

### Q23: overall-clarity-3-axis
- Axis 1 — Input clarity: **8/10** — arguments, flags, and typical invocations are well-covered with examples; the `<path>:<start>-<end>` syntax is unambiguous; only edge cases like content-escape rules need a second read.
- Axis 2 — Output clarity: **5/10** — the symbol-map and search default outputs are shown, but batched-read framing, `--json` schemas, verify command outputs, and write-success output are missing.
- Axis 3 — Failure-mode clarity: **3/10** — only `--find --replace` has a stated failure; exit codes, stderr behavior, missing paths, and out-of-range writes are entirely undefined.
- Composite mean: (8 + 5 + 3) / 3 = **5.33**.

### Q24: one-minute-edit
I'd paste a 12-line "Contracts" table: command → success output → error output → exit code → `--json` shape pointer. That is the one gap that wastes the most guesswork.

### Q25: python-fixture-output-prediction

a. `8v read util.py` — symbol map. Instructions say each line is `<line-number>  <symbol>` and the example shows `12  fn main`. For the fixture the only defined symbol is `def add`. My predicted output:

```
2  def add
```

Gap: instructions don't say whether the output includes a header (`util.py (4 lines)`) or whether it emits `def add` vs `add` vs `def add(a, b)`. My best single-line guess shown above; confidence 3.

b. `8v read util.py:1-2` — lines 1-2, 1-indexed, end inclusive:

```
# util.py  (4 lines)
def add(a, b):
```

Gap: output framing (with/without line numbers, with/without file header) not stated. Confidence 3.

c. `8v search "add" util.py` — matches grouped by file as `<path>:<line>:<text>`. Matches of `add`:

```
util.py:2:def add(a, b):
util.py:4:result = add(1, 2)
```

Gap: whether a file header line precedes the matches; whether `add` inside `def add` and as function call both match (I assume yes — plain regex). Confidence 3.

d. `8v read util.py --full` — the entire file. Instructions don't say whether line numbers are prefixed or whether a header is added. My prediction (raw file only):

```
# util.py  (4 lines)
def add(a, b):
    return a + b

result = add(1, 2)
```

Gap: output framing unspecified; the tool may or may not include line numbers or a path header. Confidence 3.

### Q26: check-exit-and-stream
Instructions say "Non-zero exit on any issue" — so exit 0 on success, non-zero on any issue. They do NOT say whether error text is on stdout, stderr, or only in `--json`. Gap: stream and format of error text. My guess: human-readable on stdout by default, structured on `--json` (schema unspecified).

### Q27: write-find-replace-contract
Instructions explicitly state: "fails if `<old>` not found" — so zero matches is a failure. They do NOT state what happens on multiple matches. Gap: multi-match behavior is undefined (silent replace-all? fail? replace-first?). They also do NOT state what "fails" looks like to the caller — exit code, stderr text, or JSON. My guess: non-zero exit, human-readable error on stderr.

### Q28: read-missing-path
Instructions don't say. Gap: no failure contract for missing paths. My guess: non-zero exit code, error text on stderr mentioning the path; no structured JSON unless `--json` was passed.

### Q29: check-silent-exit-1
Not clearly expected. Instructions only say "Non-zero exit on any issue" — so exit 1 means an issue exists, but silent stdout+stderr is not described. Gap: no contract for empty-output + non-zero-exit. Next step for the agent: re-run with `--json` to get a structured report, and/or run `8v check . --help` to see if verbosity flags exist.

### Q30: surface-differences
1. Surface 1 has the leading sentence "Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash" enumerating *native tools*. Surface 2 says "Use `8v` for all file operations (read, edit, write, search, inspect)" enumerating *verbs*. Surface 2 reader may not know which specific native tools to give up — matters if they keep using Read/Edit habitually.
2. Surface 1 adds "For anything that reads, edits, searches, or inspects files, use 8v — not Bash. If the `8v` MCP tool is available, call it directly — do not shell out via Bash." Surface 2 lacks the MCP-preference sentence. An agent seeing only Surface 2 might default to `bash("8v …")` instead of the MCP tool, adding shell overhead.
3. Surface 1's Write section is untitled beyond `## Write`; Surface 2 has `## Write — prefer targeted edits` (extra editorial nudge).
4. Surface 1's batch-read example ends with the didactic sentence "One call beats N sequential calls." Surface 2 drops it. Agents seeing only Surface 2 get less reinforcement of the batching principle.
5. Otherwise the two surfaces are substantively equivalent on commands, flags, examples.
Surface 1 is strictly more complete.

### Q31: tool-gap-task
Task: "Rename symbol `Foo` to `Bar` everywhere in the repo, but only in Rust files, and only when it refers to the struct (not the field or the local variable)."
Missing capability: scope-aware refactor. `8v write --find --replace` is per-file and string-based, with no AST awareness. Closest substitute: `8v search -e rs "\\bFoo\\b" --files` → shell loop → `8v write path --find "Foo" --replace "Bar"` per file. Cost: multiple round-trips, no semantic scoping, risk of replacing field names or locals, plus needing Bash for the loop.

### Q32: dry-run-5-step
1. **Find all Go files.** `8v ls --stack go` — confidence 4 — documented `--stack go`. (Instructions don't promise recursion from cwd; I'm assuming root.)
2. **Search `http.Get` with 2 lines context.** `8v search "http\\.Get" . -e go -C 2` — confidence 3 — flags documented but regex flavor / dot-escape behavior is a guess.
3. **Symbol map of file with most matches.** `8v read <that-file>` — confidence 4 — but identifying "most matches" requires parsing search output; no documented aggregator. Possible fallback to Bash `awk`/`sort`/`uniq -c` on the search output.
4. **Replace `http.Get` with `httpClient.Get` on a specific line.** `8v write path:<line> "…httpClient.Get(…)"` — confidence 3 — I need to know the full replacement line, not just the substring; `--find --replace` is file-scoped, so a line-level edit means rewriting the line.
5. **Run tests.** `8v test .` — confidence 5 — documented.
Fallback risk: step 3's aggregation; step 4's content reconstruction.

### Q33: memorability-no-re-read
Insert before line 42 of main.rs:
`8v write main.rs:42 --insert "new line"` — confidence 5.

Replace lines 10-20 with multi-line content:
`8v write main.rs:10-20 "line1\nline2\nline3"` — confidence 4 (slight uncertainty about trailing-newline behavior of the final line).

**Model & run ID:** Claude Opus 4.7 — opus-run3 — 2026-04-19

---

## 8v feedback

- **Friction 1 — symbol map on plain-text files is empty.** `8v read /path/to/ai_section.txt /path/to/instructions.txt /path/to/prompt-template-v2.md` returned `(no symbols found)` for all three. That is technically correct — these are prose files — but it cost me a retry turn to re-issue with `--full`. Expected: for non-source files with no parsable symbols, either (a) auto-fall-back to the file contents (perhaps capped), or (b) emit a clear hint like "no parsable symbols; use `--full` to see content" instead of a silent empty symbol map. Today the default is indistinguishable from "I analyzed this and there was nothing interesting."
- **Friction 2 — no hint that `--full` was needed.** Related to the above: the default output didn't nudge me toward `--full`, so the first call looked successful but returned nothing useful. A single-line suggestion in the output would have saved a round-trip.
- Otherwise 8v worked smoothly: the batched `--full` call returned all three files cleanly in one invocation, and the line-numbered output was easy to reference while writing the answers.
