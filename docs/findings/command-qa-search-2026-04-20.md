# QA Audit: `8v search` — 2026-04-20

**Scope:** Pure observation. No code changes. All findings are reproducible.  
**Fixture:** `/tmp/8v-search-qa/` — git-initialized directory with: `main.rs`, `lib.py`, `subdir/nested.ts`, `.gitignore`, `ignored.txt`, `binary.bin` (NUL bytes), `longline.txt` (>200 chars per line), `latin1.txt` (non-UTF-8), `noperms.txt` (chmod 000), `emptydir/`.

---

## 1. Form-by-Form Table

| Form | Command | Exit | STDOUT summary | STDERR | Verdict |
|------|---------|------|----------------|--------|---------|
| Basic | `8v search "hello" .` | 0 | matches in 3 files, footer "2 skipped" | empty | NOISY — skips invisible without exit signal |
| Basic, no matches | `8v search "zznotfound" .` | 1 | `no matches found` | empty | AMBIGUOUS — exit 1 = no-match AND read-errors |
| Single file | `8v search "hello" main.rs` | 1 | `:3: println!("hello world")` | empty | BUG — path prefix missing; exit 1 despite match |
| Basic + `--files` | `8v search "hello" . --files` | 0 | file paths only, footer | empty | OK |
| Case insensitive | `8v search "HELLO" . -i` | 0 | matches across files | empty | OK |
| Extension filter | `8v search "hello" . -e rs` | 0 | only `.rs` files | empty | OK |
| Extension filter | `8v search "hello" . -e py` | 0 | only `.py` files | empty | OK |
| Extension filter | `8v search "hello" . -e ts` | 0 | only `.ts` files | empty | OK |
| Invalid extension | `8v search "hello" . -e xyz` | 1 | `no matches found` (no `.xyz` files) | empty | OK (silent but expected) |
| Context C=0 | `8v search "hello" . -C 0` | 0 | `file:line: text` per match | empty | OK |
| Context C=1 | `8v search "hello" . -C 1` | 0 | match + `>` before + `<` after | empty | WRONG — `>` = before, `<` = after (inverted convention) |
| Context C=3 | `8v search "hello" . -C 3` | 0 | 3 lines each side | empty | WRONG — same label inversion |
| Context C=50 | `8v search "hello" . -C 50` | 0 | clipped to file boundaries | empty | OK |
| Context C=-1 | `8v search "hello" . -C -1` | 1 | error message | empty | BUG — error to STDOUT not STDERR |
| Context C=abc | `8v search "hello" . -C abc` | 1 | parse error | empty | BUG — error to STDOUT not STDERR |
| Limit 1 | `8v search "hello" . --limit 1` | 0 | 1 result, footer "truncated" | empty | OK |
| Limit 0 | `8v search "hello" . --limit 0` | 0 | `no matches found` | empty | BUG — 0 silently accepted, returns nothing |
| Limit 1000000 | `8v search "hello" . --limit 1000000` | 0 | all matches | empty | OK |
| Multi-word pattern | `8v search "fn main" .` | 0 | matches | empty | OK |
| Anchored regex | `8v search '^impl' .` | 0 | matches at line start | empty | OK |
| Word boundary | `8v search '\bfoo\b' .` | 0 | matches | empty | OK |
| Invalid regex `(` | `8v search '(' . ` | 1 | `error: invalid regex pattern: ...` | empty | BUG — error to STDOUT not STDERR |
| Invalid regex `[` | `8v search '[' .` | 1 | `error: invalid regex pattern: ...` | empty | BUG — error to STDOUT not STDERR |
| Wildcard `.*` | `8v search '.*' .` | 0 | every line in every file | empty | NOISY — fire-hose, no warning |
| Single char `.` | `8v search '.' .` | 0 | every character match | empty | NOISY — fire-hose, no warning |
| Empty pattern `''` | `8v search '' .` | 1 | `error: pattern cannot be empty` | empty | BUG — error to STDOUT not STDERR |
| Nonexistent path | `8v search "hello" /no/such/dir` | 1 | `no matches found` | empty | BUG — path error indistinguishable from no-match |
| Gitignored file | `8v search "hello" . ` | 0 | `ignored.txt` excluded | empty | OK (correct behavior) |
| chmod-000 file | `8v search "hello" .` (with noperms) | 0/1 | matches found, footer "1 skipped" (or exit 1 if only match in noperms) | empty | BUG — permission error invisible |
| Binary file | `8v search "hello" .` (with binary.bin) | 0 | binary.bin not in results | empty | BUG — silently skipped, not counted in files_skipped |
| Long lines >200 chars | `8v search "longline" .` | 0 | text truncated at 200 chars with `…` | empty | OK (documented behavior) |
| Non-UTF-8 file | `8v search "hello" .` (with latin1.txt) | 0/1 | footer "1 skipped" | empty | PARTIAL — skipped counted, but exit ambiguous |
| `--json` basic | `8v search "hello" . --json` | 0 | `{files:[{path,matches}],...}` | empty | OK — schema correct |
| `--json` no match | `8v search "zzz" . --json` | 1 | `{files:[],total_matches:0,...}` | empty | OK |
| `--json` --files | `8v search "hello" . --files --json` | 0 | `{files:[string,...],total,...}` | empty | OK — different schema from content mode |
| `--json` invalid regex | `8v search '(' . --json` | 1 | `error: invalid regex pattern: ...` (plain text) | empty | BUG — plain text error, not JSON |
| `--json` single file | `8v search "hello" main.rs --json` | 1 | `"path":""` | empty | BUG — empty path in JSON |
| Empty dir | `8v search "hello" emptydir/` | 1 | `no matches found` | empty | OK (expected) |

---

## 2. Top 5 Issues Agents Will Hit

### Issue 1: Single-file search returns empty path

When `8v search <pattern> <file>` is called with a single file, the output contains no filename:

```
:3: println!("hello world")
```

And JSON has `"path": ""`. An agent parsing this output to find which file contains the match gets an empty string. The agent cannot proceed without knowing which file it searched — it would need to remember the input argument. This breaks stateless output parsing entirely.

**Severity:** High. Agents frequently search single files to locate a symbol before editing.

### Issue 2: Exit code 1 means three different things

Exit 1 is returned for:
- No matches found (search succeeded, nothing matched)
- Read errors occurred (permission-denied, non-UTF-8 files)
- Invalid arguments (bad regex, bad -C value)

There is no way to distinguish these from exit code alone. STDERR is always empty. An agent checking exit code to decide whether to retry cannot determine the cause.

**Severity:** High. Agents that exit-check will misclassify permission errors as "no results" and stop searching.

### Issue 3: Binary and permission-denied files are invisible in different ways

- `chmod 000` files: counted in `files_skipped`, appear in footer as "N skipped", but STDERR is empty and the specific path is never named
- Binary files (NUL bytes): NOT counted in `files_skipped`, not mentioned anywhere, exit code unchanged

An agent searching for content in a directory where some files cannot be read has no reliable signal that its search is incomplete. The binary-file case is especially dangerous because the agent has zero indication anything was skipped.

**Severity:** High. Silent incompleteness is a correctness bug for agents doing exhaustive searches.

### Issue 4: `--json` returns plain-text error for invalid regex

All other error conditions in `--json` mode return structured JSON. Invalid regex returns a plain-text `error: ...` string. An agent that assumes `--json` always produces parseable JSON will crash on JSON decode.

**Severity:** Medium. Agents using `--json` for automation need consistent output format.

### Issue 5: `--limit 0` is silently accepted and returns 0 results

`8v search "hello" . --limit 0` exits 0 with "no matches found". No error, no warning. An agent that passes `--limit 0` by accident (off-by-one, miscalculation) gets a result that looks valid but contains nothing. The exit 0 makes it look like the search found nothing to report rather than that the limit was invalid.

**Severity:** Medium. Hard to debug because the output is indistinguishable from a legitimately empty result in the same directory.

---

## 3. Partial-Failure and Silent-Skip Classification

| Scenario | Current behavior | Classification | Required fix |
|----------|-----------------|----------------|--------------|
| `chmod 000` file — path never named | Footer shows "N skipped", no path | render-fix | Include skipped paths in footer or JSON `skipped_files` array |
| Binary file (NUL bytes) — not counted | No signal at all | behavior-fix | Increment `files_skipped` for binary files OR add separate `binary_files_skipped` counter |
| Non-UTF-8 file — counted but opaque | Footer shows count, no path | render-fix | Same as chmod case — surface the path |
| Nonexistent path — looks like no-match | Exit 1, "no matches found" | behavior-fix | Distinguish path-not-found from search-found-nothing |
| Single-file search — empty path | `path: ""` in output and JSON | behavior-fix | Use input filename as path when walking single file |
| `--json` + invalid regex — plain text | `error: ...` as plain text string | render-fix | Return `{"error": "...", "code": "invalid_pattern"}` |
| `--limit 0` — silently accepted | Exit 0, "no matches found" | behavior-fix | Reject as invalid argument; exit 1 with error message |
| Error messages to STDOUT | All errors on STDOUT, STDERR empty | render-fix | Errors to STDERR, results to STDOUT |

---

## 4. Noise

### 4.1 Context direction labels are inverted

`-C 1` output:

```
main.rs:3: println!("hello world")
  > // comment before match
  < // comment after match
```

`>` conventionally means "output" / "after" (diff, ripgrep). `<` conventionally means "input" / "before". The 8v implementation has them reversed: `>` is used for lines BEFORE the match, `<` for lines AFTER. An agent that interprets context lines using the conventional meaning will have before/after flipped.

### 4.2 `--files` footer uses Debug-format regex

Footer: `Found 1 files matching "\.py$"`

The pattern is shown with raw regex syntax inside Debug-format double quotes. A display-facing message should either use the user's original input verbatim or omit the pattern. The extra quoting adds visual noise and is inconsistent with content-mode footers.

### 4.3 Footer on empty result

When there are no matches, output is: `no matches found` (no newline at end on some terminal widths, no footer). The footer with file counts only appears when there are matches. An agent looking for `files_searched` count always gets it — except on zero-match results. JSON mode always includes `files_searched`, so this is a text-mode-only inconsistency.

### 4.4 Compact mode (default) omits match text

Default (`-C` absent) output: `file:line` — no text. This forces agents to do a second read to get the actual content. The default is optimized for "does this pattern exist?" use cases, not "show me the match" use cases. This is by design but worth documenting: agents wanting text must always pass `-C 0`.

### 4.5 Trailing newline inconsistency

`no matches found` has no trailing newline in STDOUT (confirmed by hex). All match output ends with `\n`. An agent splitting on newlines gets an empty final token for match output but not for the no-match case — inconsistent.

---

## 5. Proposed Next Slice

**Slice: Error contract for `search`**

Priority order based on agent impact:

1. **Fix empty path on single-file search** (behavior-fix, BUG-1). Single-file is a common agent pattern. Empty path breaks all output parsing.

2. **Errors to STDERR** (render-fix, BUG-5). Invalid regex, bad `-C`, empty pattern — all go to STDOUT. Move to STDERR. Standard Unix contract. Enables agent stdout/stderr separation.

3. **`--json` error shape** (render-fix, BUG-4). Return `{"error": "...", "code": "..."}` for all error cases in `--json` mode. Follows the error-contract design doc that was written today.

4. **`--limit 0` rejection** (behavior-fix). Return exit 1 with error message. Prevents silent zero-result bugs.

5. **Binary file visibility** (behavior-fix, BUG-3). Either count in `files_skipped` or add `binary_files_skipped` field. The completely invisible case is the hardest for agents to debug.

Items NOT in this slice (out of scope until error contract is stable):
- Context direction labels (separate cosmetic slice)
- Skipped-file path surfacing (separate detail-level slice)
- Fire-hose pattern warning (would need pattern heuristic — design needed)

---

## 6. 8v Feedback — Friction During This Audit

This section documents friction encountered while using `8v` tooling to conduct this audit. Every item below is an observation, not a feature request.

### 6.1 CWD dependency not documented

`8v search <pattern> <absolute-path>` run from outside a git repo returns "no matches found" even when the path has matching files. The command silently fails. The requirement to run from within a git repo is not documented anywhere in `8v search --help` or the CLAUDE.md instruction surface. This cost approximately 20 minutes of debugging during the audit.

**Impact:** Agents running `8v search` with absolute paths (common in agent contexts where CWD varies) will get silent false negatives.

### 6.2 No way to distinguish "0 files walked" from "0 files matched"

When `8v search "hello" /tmp/some-nongit-dir/` is run from outside a git project and returns "no matches found", there is no indication whether the directory was walked at all (0 files walked) or walked but nothing matched (N files walked, 0 matched). The footer only appears when there are matches. The JSON `files_searched: 0` field distinguishes these — but only if `--json` is used.

### 6.3 `8v read` on search.rs required multiple range calls

`search.rs` is 414 lines. The symbol map (`8v read search.rs`) gave line numbers for each function. Getting the full `search_file_contents` implementation required three separate range calls because the function body spans lines that the symbol map shows as a single entry. This is expected progressive behavior, but the round-trip cost is non-trivial for audit work where you need to read the full implementation in one pass.

### 6.4 `8v search` has no `--count` mode

During audit, counting matches per file required parsing the footer number from full output. A `--count` mode (like `grep -c`) would have made audit work faster and would also serve agents that need "how many occurrences" without the full match list.

### 6.5 Exit code 1 means too many things

Documented above as BUG-2. Also a direct friction point during the audit: writing test cases that check exit codes was complicated because the same exit code meant different things in different scenarios. The audit needed to cross-reference stdout content to classify what kind of "failure" exit 1 represented. This is the same problem agents face at runtime.

---

## Appendix: Test Fixture Contents

```
/tmp/8v-search-qa/
├── .git/                   (git init, no commits needed)
├── .gitignore              (contains: ignored.txt)
├── main.rs                 (contains "hello world", "fn main", "impl Foo")
├── lib.py                  (contains "hello", "def foo")
├── subdir/
│   └── nested.ts           (contains "hello", "interface Bar")
├── ignored.txt             (contains "hello" — gitignored)
├── binary.bin              (contains "hello\x00binary\x00data")
├── longline.txt            (contains 10000-char line with "longline" in it)
├── latin1.txt              (contains latin-1 encoded bytes, not valid UTF-8)
├── noperms.txt             (contains "secret", chmod 000)
└── emptydir/               (empty directory)
```

All tests run from `/tmp/8v-search-qa/` as CWD using `8v` at `/Users/soheilalizadeh/.8v/bin/8v`.
