# Instruction Clarity v3 — Opus Run 2 — 2026-04-19
Model: Claude Opus 4.7
Surfaces read: ai_section.txt, instructions.txt

---

### Q1: summary
8v is a code-reliability tool for AI agents that replaces native file tools (Read/Edit/Write/Grep/Glob and file-touching Bash) with one CLI. Use it for any file read, edit, search, or inspection; keep Bash only for git/process/env. Two principles: **Progressive** (minimum useful default, flags escalate detail) and **Batch** (pass many inputs per call to amortize overhead).

### Q2: minimum-viable invocations
- `ls` → `8v ls` — returns a file listing (flags add `--tree`, `--loc`, `--match`, `--stack`). The minimum form is not explicitly shown; text leads with `8v ls --tree --loc`.
- `read` → `8v read <path>` — returns symbol map (`<line>  <symbol>`).
- `search` → `8v search <pattern>` — returns `<path>:<line>:<text>` matches grouped by file. (Path arg appears optional from the `[path]` signature.)
- `write` → `8v write <path>:<line> "<content>"` — replaces one line. (No zero-arg form shown.)
- `check` → `8v check .` — lint + type-check + format-check; non-zero exit on any issue.
- `fmt` → `8v fmt .` — auto-formats in place; idempotent.
- `test` → `8v test .` — runs project tests.
- `build` → `8v build .` — compiles.
All verify commands accept `--json` and default to whole project.

### Q3: the two principles
1. **Progressive** — every command returns the smallest useful payload by default and escalates on demand. Non-obvious example: `8v read foo.rs` does NOT return the file contents — it returns a symbol map; you must opt into a range or `--full` to see text.
2. **Batch** — one call takes many arguments. Non-obvious example: you can mix distinct files and multiple ranges of the same file in a single call: `8v read a.rs b.rs a.rs:1-200 a.rs:500-600`.

### Q4: when to use 8v vs native
Use 8v for all file operations: read, edit, write, search, inspect. Use Bash only for git, process management, and environment operations. Explicit rule: "If the `8v` MCP tool is available, call it directly — do not shell out via Bash."

### Q5: flag discovery
`8v <cmd> --help` for the full flag list. Every command also accepts `--json`.

---

### Q6: ambiguous phrases
- "**Default output is the minimum useful answer.**" — Reading A: minimum text volume. Reading B: minimum semantic answer to the question (could still be large).
- "**Each call costs overhead — amortize it.**" — Reading A: per-process launch cost. Reading B: per-MCP-round-trip / token cost. The text does not pin this down.
- "**`8v write <path>`**" with `--append` / `--find`/`--replace` — Reading A: these are path-only forms with no line spec. Reading B: line spec is optional. The two are not disambiguated.
- "**Non-zero exit on any issue**" (check) — Reading A: specifically exit code 1. Reading B: any non-zero value (could be 2, 3, …).
- "**`--files` lists only paths**" — Reading A: paths of files that matched. Reading B: the file list itself (no match filtering).
- "**Pass a path to scope to a subtree**" — Reading A: any relative path works. Reading B: must be a directory, not a file.
- "**fails if `<old>` not found**" — Reading A: only zero-match fails. Reading B: says nothing about multi-match, which is itself ambiguous.
- "**One call beats N sequential calls.**" — Reading A: always faster. Reading B: cost heuristic, not a correctness claim.

### Q7: implied-but-unstated behaviors
- Exit codes for `read`, `write`, `search`, `ls`, `fmt`, `test`, `build` when they fail (only `check` is specified as non-zero on issues).
- stdout vs stderr split.
- Error format (plain text? JSON with `--json`? schema?).
- Behavior when path doesn't exist / is binary / is a symlink.
- Behavior of `8v read` on a directory.
- Whether `write` creates missing files or fails.
- Whether `write --find/--replace` replaces all or first; whether it is regex or literal.
- Whether `search` pattern is regex-only (it says `(regex)` — presumably mandatory regex, but unstated).
- Newline handling on single-line replace (trailing `\n` appended? stripped?).
- Unicode / non-UTF-8 handling.
- Concurrency / file-locking.
- Output ordering when batching.
- Whether `--json` is stable schema across versions.
- Whether absolute vs relative paths both work.
- Whether `8v` respects `.gitignore`.
- Glob syntax for `--match` (shell glob? bracket expansion? case sensitivity?).
- What "stack" detection means and how it finds files.
- Default `search` scope when `[path]` omitted.

### Q8: undefined terms
- **symbol map** — exemplified (`12  fn main`) but not defined; unclear which symbol kinds are included (macros? consts? methods? private items?) or which languages are supported.
- **stack** — enumerated but not defined (is it a language? build system? framework?).
- **progressive** — used as a principle name; the instructions explain by example only.
- **overhead** — unspecified (time? tokens? context window?).
- **symbol map "line-number"** — implicitly the definition line but not formally specified for multi-line definitions.
- **"project"** (in `8v test .` runs project tests) — what counts as a project? auto-detected?
- **"subtree"** (verify commands) — path-based? git-based?
- **"idempotent"** (fmt) — what about files it cannot format?

### Q9: contradictions between surfaces
- **Bash-usage scope**: Surface 1 says "Use Bash only for git, process management, and environment operations." Surface 2 says "Use shell tools only for git, process management, and environment operations." Minor terminology drift (Bash vs shell tools).
- **MCP-vs-Bash preference**: Surface 1 includes the "If the `8v` MCP tool is available, call it directly" rule; Surface 2 does not.
- **`8v ls` top-level**: Surface 1's heading is "Discovery"; Surface 2's is "Discovery — learn the repo in one call". Same info.
- **Write section heading**: Surface 2 adds "prefer targeted edits"; Surface 1 does not. Minor, not a contradiction.
- **Read batch example**: Surface 1 adds "One call beats N sequential calls" emphasis; Surface 2 drops that clause.
No hard contradictions, but scope drift.

### Q10: `8v read a.rs b.rs Cargo.toml` output shape
The instructions do not say. Gap. My guess: one symbol map per file, concatenated in argument order, with some file-header separator. Cannot be determined from the text alone.

### Q11: `8v write <path>:<line> "<content>"` newline/multi-line semantics
The text says content arguments are parsed by 8v (not the shell): `\n` → newline, `\t` → tab, `\\` → backslash, passed as literal two-character sequences. The instructions do **not** say whether a trailing newline is appended automatically. To write multi-line content, use `\n` literally inside the string. The surrounding quotes are shell quoting (standard) — they do not have 8v-specific meaning.

---

### Q12: scenario commands

a. **Read 5 files at once** → `8v read a.rs b.rs c.rs d.rs e.rs` · **5** · explicit batch example.
b. **Replace lines 10–20 of foo.rs with 3-line content** → `8v write foo.rs:10-20 "line1\nline2\nline3"` · **4** · range-replace form documented; `\n` escape rule documented. Confidence not 5 because trailing-newline behavior is undefined.
c. **Find all `handle_*` functions** → `8v search "fn handle_\w+"` · **3** · `search` is regex; Rust convention is `fn name` — but multi-language repos would need different patterns. Would also try `8v search "handle_\w+\s*\("` .
d. **Append one line to notes.md** → `8v write notes.md --append "new line"` · **4** · append form documented; trailing-newline behavior still unclear.
e. **Symbol map of bar.rs then lines 100–150** → `8v read bar.rs bar.rs:100-150` · **5** · batch with mixed forms is explicit.
f. **Run tests, parse JSON** → `8v test . --json` · **4** · `--json` is universal; shape not documented.
g. **Check file existence before reading** → Instructions don't say — my guess: `8v ls <path>` or rely on `8v read <path>` error. Fall back to Bash `test -f`. · **2** · no dedicated existence check documented.
h. **Delete lines 50–60** → `8v write foo.rs:50-60 --delete` · **5** · explicit.
i. **Insert a new line before line 30** → `8v write foo.rs:30 --insert "new line"` · **5** · explicit.
j. **Case-insensitive TODO in Rust only** → `8v search "TODO" -i -e rs` · **4** · flags documented; unclear whether `-e` takes `rs` or `.rs`.
k. **Find files matching `*_test*.md`** → `8v ls --match "*_test*.md"` · **3** · `--match <glob>` documented but glob dialect unspecified; may need to scope to `.md`.
l. **Lint + format-check + type-check** → `8v check .` · **5** · explicitly defined as "lint + type-check + format-check".
m. **Refactor `old_name` → `new_name` across many files** → Instructions don't say — my guess: loop over files via shell, calling `8v write <path> --find "old_name" --replace "new_name"` per file. `--find/--replace` is documented per-file only. · **2** · no repo-wide replace.
n. **Symbol maps of 10 files in one call** → `8v read a b c d e f g h i j` · **5** · explicit batch.
o. **Read lines 1-200 and 500-600 of big.rs in one call** → `8v read big.rs:1-200 big.rs:500-600` · **5** · explicit example.

### Q13: how each command is taught
- `ls` — **example** (`8v ls --tree --loc`, `8v ls --match … --stack …`).
- `read` — **example** (symbol map sample output included).
- `search` — **example** (signature + `<path>:<line>:<text>` default).
- `write` — **example** (six forms listed).
- `check` — **description-only** ("lint + type-check + format-check").
- `fmt` — **description-only** ("auto-format in place. Idempotent").
- `test` — **description-only** ("run project tests").
- `build` — **description-only** ("compile").

---

### Q14: three likeliest mistakes
1. Calling `8v read foo.rs` and expecting file contents, then being confused by a symbol map.
2. Using `\n` in single-line `write` replace, unsure whether a trailing newline is auto-appended, producing off-by-one blank lines.
3. Running `8v write <path> --find "x" --replace "y"` in a repo-wide refactor and realizing it only touches one file, then looping in Bash — missing a proper bulk-rename flow.

### Q15: fallback triggers
- Checking whether a file exists (no explicit command).
- Repo-wide find/replace (single-file form only).
- Anything requiring structured error output — errors are not documented.
- Binary/image files.
- Git operations (explicitly Bash territory).
- Running an arbitrary shell command or process management.
- Environment manipulation.

### Q16: most/least used
- Most: `read` (symbol map is the cheapest way to learn a file), `search`, `write`, `check`.
- Least: `build` (slower than `test`/`check`), `fmt` (implicit in `check`).
- `ls` is middle — only at repo entry.

### Q17: first command in a new repo
`8v ls --tree --loc` — explicitly labeled "Start here" and gives hierarchy + line counts in one call.

### Q18: trust error from failed `8v write`?
Partially. `--find/--replace` is documented to fail if `<old>` not found, so that error is predictable. For line-range writes, unexpected-path or out-of-range behavior is not documented, so I would not fully trust the error message to be actionable without `8v <cmd> --help`.

---

### Q19: missing / wished
- Error-format spec (exit codes, stdout vs stderr, JSON schema).
- Path-not-found semantics.
- Symbol-map language coverage list.
- Repo-wide find/replace.
- `exists` / `stat` equivalent.
- Glob dialect spec for `--match`.
- `search` pattern: is it regex-only or literal-fallback?
- Trailing-newline semantics for `write`.
- Output ordering / interleaving for batch reads.
- `--json` schema per command.
- Stack-detection rules.

### Q20: where I'd hesitate teaching
- Explaining `search`'s `(regex)` token — is it required?
- Demonstrating `write --find/--replace` without giving false confidence that it scales to refactors.
- Explaining `--json` shapes.
- Telling the learner what exit codes mean.

### Q21: 8v vs native — better / worse
- Better: batched reads, symbol-map-first reads, unified `check/fmt/test/build`, consistent `--json`, one-call file operations (fewer round-trips).
- Worse: no existence check, no bulk rename, opaque errors, no streaming output documented, unclear regex dialect, unclear glob dialect.

### Q22: one highest-impact edit
Add a dedicated "Errors & Exit Codes" section specifying: exit code on success, on "not found", on "ambiguous", on "invalid args"; whether errors print to stdout or stderr; and the shape of `--json` error objects. This single block would close the largest class of unknowns.

---

### Q23: three-axis clarity scores
- **Axis 1 — Input clarity: 8/10.** Commands, flags, ranges, batch forms, and the escape rules for content arguments are laid out concretely with examples.
- **Axis 2 — Output clarity: 5/10.** Symbol-map format and search line format are shown, but output for `ls`, batched `read`, `check`, `fmt`, `test`, `build`, and the `--json` shape are left implicit or unstated.
- **Axis 3 — Failure-mode clarity: 2/10.** Only one failure is documented (`--find` with no match); exit codes, stderr vs stdout, path-not-found, ambiguous-match, invalid-range, and error-JSON shape are all absent.
- **Composite mean: (8 + 5 + 2) / 3 = 5.00.**

### Q24: one-minute improvement
Add a 10-line "Errors & Exit Codes" block plus a one-line "when path doesn't exist" note to each of `read`/`write`/`search`. That alone lifts Axis 3 from 2 to 6 and removes the biggest class of agent guesswork.

---

### Q25: util.py predictions

a. `8v read util.py` — symbol map, approximately:
```
1  def add
```
Instructions say "symbol map. Each line: `<line-number>  <symbol>`". Python coverage is not explicitly stated (gap), but `add` is a function and should appear. Whether the top-level assignment `result = add(1, 2)` is listed is not stated.

b. `8v read util.py:1-2` — literal lines 1-2, 1-indexed, inclusive:
```
def add(a, b):
    return a + b
```

c. `8v search "add" util.py` — default output is `<path>:<line>:<text>` grouped by file:
```
util.py:1:def add(a, b):
util.py:4:result = add(1, 2)
```
Gap: instructions do not state whether the pattern is treated as regex or literal when it contains only word characters. Here both readings yield the same matches.

d. `8v read util.py --full` — entire file (4 lines) verbatim:
```
def add(a, b):
    return a + b

result = add(1, 2)
```
Gap: whether a header like `=== util.py ===` is prepended is not stated.

### Q26: `8v check .` exit codes and streams
Text quoted: "`8v check .` — lint + type-check + format-check. Non-zero exit on any issue."
- Success exit code: not stated. Gap.
- Specific non-zero value: "Non-zero" only — could be 1, 2, etc. Gap.
- Whether error text appears on stdout, stderr, or only with `--json`: not stated. Gap.

### Q27: `--find/--replace` zero-match and multi-match
Text quoted: "`8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found."
- Zero matches: fails. "fails" but the error shape/exit code is not stated. Gap.
- More than one match: **not stated**. Could replace all, could replace first, could fail as ambiguous. Gap.
- Caller return value: not stated. Gap.

### Q28: `8v read <path>` on missing path
Not stated. Gap. My guess: non-zero exit code and an error message on stderr; with `--json`, a structured error. But the text says nothing about path-not-found behavior, stream routing, or exit codes for `read`.

---

### Q29: `8v check .` silent, exit 1
Instructions do not cover this case. Quoted text is only: "Non-zero exit on any issue." That implies exit 1 means an issue exists, but the silent-output scenario is unaddressed.
Agent action: re-run with `--json` to get structured output (every command accepts `--json`), then consult `8v check --help` for more flags. Also fall back to native per-stack linters to locate the issue. This is a gap.

---

### Q30: factual differences between surfaces
1. **MCP-preference rule**: Surface 1 has "If the `8v` MCP tool is available, call it directly — do not shell out via Bash." Surface 2 does not. An agent seeing only Surface 2 might still shell out to 8v via Bash, losing a round-trip optimization.
2. **Bash vs shell tools**: Surface 1 says "Use Bash only for …"; Surface 2 says "Use shell tools only for …". Surface 2 is more general and more accurate for non-Bash environments; Surface 1 is more concrete.
3. **Opening framing**: Surface 1 calls 8v a replacement for "Read, Edit, Write, Grep, Glob, and Bash for file operations"; Surface 2 says "8v — code reliability tool for AI agents. Designed to minimize round-trips." Surface 1 is more actionable (it names the native tools being replaced); Surface 2 is more strategic.
4. **"One call beats N sequential calls"**: only in Surface 1. Losing this line weakens the batching intuition for an agent seeing only Surface 2.
5. **Write section subtitle**: Surface 2 adds "prefer targeted edits"; Surface 1 omits. Minor.
6. **Typical flow**: identical in both.

Surface 1 is the more complete agent-facing document because of items 1, 3, and 4. Surface 2 is otherwise parallel.

---

### Q31: realistic tool-gap
**Task**: rename a symbol `processOrder` to `processCustomerOrder` across all TypeScript files in a repo, where the symbol appears in ~20 files.
**Missing capability**: repo-wide find/replace. `8v write <path> --find/--replace` is per-file; there is no multi-file / glob form.
**Closest substitute**: combine `8v search "processOrder" -e ts --files` (to list files) with a Bash loop calling `8v write` per file. Cost: N extra calls (violates Batch principle), no atomicity, no dry-run.

---

### Q32: 5-step dry-run
1. **Find all Go files** → `8v ls --stack go` · **4** · `--stack go` is an explicit listed value, but this may or may not return a flat list suitable for piping (output shape unstated).
2. **Search `http.Get` with 2 lines of context across those files** → `8v search "http\.Get" -C 2 -e go` · **4** · `-C N` and `-e <ext>` are documented; unclear if `-e go` vs `-e .go`.
3. **Symbol map of the file with the most matches** → cannot be determined from 8v alone — would pipe step 2 output through Bash (`sort | uniq -c | sort -rn | head -1`) to find the top file, then `8v read <that-file>` · **2** · counting matches is not an 8v capability.
4. **Replace `http.Get` with `httpClient.Get` on a specific line** → `8v write <file>:<line> "…httpClient.Get…"` or `8v write <file> --find "http.Get" --replace "httpClient.Get"` · **4** · both forms documented; multi-match behavior of `--find` unknown.
5. **Run tests and confirm pass** → `8v test .` and check exit code · **4** · documented, though success exit code is not explicitly stated.

Fallback flags: step 3 requires Bash aggregation.

---

### Q33: memorability (no re-read)
- Insert before line 42 of main.rs: `8v write main.rs:42 --insert "new line"` · **5**.
- Replace lines 10–20 with multi-line content: `8v write main.rs:10-20 "line1\nline2\nline3"` · **4** (confident on syntax; unsure about trailing-newline behavior).

**Model & run ID:** Claude Opus 4.7 — opus-run2-2026-04-19

---

## 8v feedback

- `8v read` on the three input files returned "no symbols found" with no content. That is correct behavior — `ai_section.txt`, `instructions.txt`, and `prompt-template-v2.md` have no language symbols — but for plain-text / Markdown files the symbol-map default is close to useless. Expected: either a line-count + preview, or an automatic fall-through to `--full` for small text files. Workaround: re-ran all three with `--full`, paying a second round trip. Command: `8v read a b c` → empty symbol maps, then three separate `8v read <path> --full` calls (could not batch `--full` across three paths since `--full` is a file-level flag and I wasn't sure whether it applied to all positional args or only the last; safer to split).
- Batch + `--full` interaction is undocumented — this is exactly the ambiguity I flagged in Q10 and Q7, hit live during this run.
- Otherwise 8v worked smoothly: `8v read` with batched arguments ran in one call, and the `:start-end` form would have been usable if symbols had existed.
