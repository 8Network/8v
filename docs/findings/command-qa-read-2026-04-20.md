# QA Audit: `8v read` — 2026-04-20

Pure observation. No code changes. All findings are from live binary runs.

Binary under test: `/Users/soheilalizadeh/8/products/vast/oss/8v/target/debug/8v`

---

## Form-by-Form Table

| Form | Verdict | Reason |
|------|---------|--------|
| `8v read <rust-file>` | ✓ | Line numbers precise, all pub/fn/impl/struct/enum captured |
| `8v read <python-file>` | ✓ | Functions and classes captured; methods listed flat (no nesting indication) |
| `8v read <typescript-file>` | ◐ | `export function` captured; `export const`, `import` statements NOT captured |
| `8v read <go-file>` | ✓ | `func` and `struct` captured correctly |
| `8v read <plain-text>` | ✓ | "no symbols found — use --full" hint fires correctly |
| `8v read <empty-file-within-project>` | — | Untestable: mktemp outside project dir rejected; no way to test without writing a file |
| `8v read <binary-file>` | ✗ | Error message on stdout, exit 0 — should be exit 1 |
| `8v read <path>:1-10` | ✓ | Correct lines returned, 1-indexed |
| `8v read <path>:1-1` | ✓ | Single-line range works |
| `8v read <path>:1-1000000` | ✓ | Silently clamps to EOF — safe |
| `8v read <path>:0-10` | ◐ | Silently corrects 0→1; no warning emitted |
| `8v read <path>:-10` | ✗ | "not found" error is misleading — this is a parse error, not a missing file |
| `8v read <path>:abc-xyz` | ✗ | Parsing bug: `:a` consumed into path, error says "not found: /path/symbols.rsbc-xyz" |
| `8v read <path> --full` | ✓ | Full content, correct |
| `8v read <path> --json` (symbol map) | ◐ | No trailing newline; `kind` field present (absent in text mode) |
| `8v read <path>:1-10 --json` | ✓ | Correct JSON shape `{"Range":{...}}` |
| `8v read a b c` (batch 3 files) | ◐ | Double header: `=== label ===` delimiter + per-file header inside content = redundant noise |
| `8v read a b c --full` | ✓ | `===` delimiters only, clean separation |
| `8v read a b c --json` | ◐ | Asymmetric shape: single=`{"Symbols":{...}}` vs batch=`{"Multi":{"entries":[...]}}` — agents must branch on array length |
| `8v read a a a` (duplicates) | ◐ | Silently repeats output 3×; no dedup or warning |
| `8v read a.rs a.rs:1-10 a.rs:5-15` (range overlap) | ✗ | Third entry (`a.rs:5-15`) produces empty output — rendering gap |
| `8v read <nonexistent>` | ✓ | Exit 1, clear "not found" error |
| `8v read valid.rs nonexistent.rs` (partial failure) | ✓ | Continues past error, shows result for valid file, exit 1 |
| `8v read <1409-line file>` | ✓ | All 31 symbols captured correctly |
| `8v read <in-repo symlink>` | ◐ | Resolves to target; label shows target path, not link path |
| `8v read <out-of-repo symlink>` | ✓ | Rejected with clear "symlink escapes project directory" error |
| `8v read <directory>` | ✓ | Clear error, not silent |
| `8v read` (no args) | ✓ | Clap usage error, exit 2 |
| `8v read --help` | ✓ | All flags listed |

---

## Top 5 Agent-Friction Issues

**1. Range parsing bug (`path:abc-xyz` and `path:-N`)**
Any invalid range format silently mangles the path string instead of returning a parse error. `path:abc-xyz` becomes "not found: /path/symbols.rsbc-xyz" — the `:a` is consumed into the path. An agent that constructs a range programmatically and gets the syntax wrong receives a "file not found" error when the file exists, causing confusion and wasted retry turns.

**2. Binary file exits 0 on error**
`8v read <binary>` emits an error message but exits 0. Agents checking exit codes to decide whether to continue a batch pipeline will not detect the failure. The error message is on stdout, not stderr, so even redirect-based detection fails.

**3. Range overlap produces empty output for third entry**
`8v read a.rs a.rs:1-10 a.rs:5-15` — the third entry is silently empty. No error, no indication. An agent batching overlapping ranges to explore a function boundary gets back a ghost entry with no content and no signal about why.

**4. `kind` absent in text mode**
The symbol map text output omits the `kind` column (`fn`, `struct`, `impl`, etc.). An agent navigating by type ("find all structs") must switch to `--json` and parse a different structure. This also breaks any agent parsing text output by column count since the column count is different between text and JSON.

**5. Batch JSON shape asymmetry**
Single file → `{"Symbols":{...}}`. Batch of 1+ files → `{"Multi":{"entries":[...]}}`. An agent that sends one file gets a different top-level key than an agent that sends two. Correct handling requires: check if array length == 1 → handle as Symbols, else handle as Multi. Every agent parsing batch JSON must implement this branch or break on single-entry batches.

---

## Language-Specific Quality Ranking

| Rank | Language | Quality | Notes |
|------|----------|---------|-------|
| 1 | Rust | Best | pub/fn/struct/enum/impl/trait all captured; line numbers accurate |
| 2 | Go | Good | func + struct captured; interface missing |
| 3 | Python | Good | def + class captured; method nesting not indicated in text mode |
| 4 | TypeScript | Partial | export function captured; export const, interface sometimes captured; imports never |
| 5 | Plain text | N/A | Hint fires correctly; intentionally no symbols |

TypeScript is the weakest: `export const handler = ...` arrow functions are invisible. For a TS file that uses only arrow exports (common in React/Node), the symbol map is empty even though the file has 10+ named exports.

---

## Noise / Useless Output

- **Double header in batch symbol mode**: `=== path/to/file.rs ===` followed immediately by the file's own internal header from the symbol extractor. The `===` delimiter alone would suffice.
- **Duplicate output on repeated paths**: `8v read a.rs a.rs a.rs` returns 3 identical blocks with 3 `===` headers. No dedup, no note. Pure noise if agent deduplication logic is absent.
- **Symlink label mismatch**: When reading via a symlink, the `===` label shows the resolved target path, not the path the agent passed in. If the agent is tracking which label corresponds to which input, this breaks the mapping.

---

## Missing Info Agents Would Have to Re-fetch

- **Symbol kind** (fn vs struct vs impl vs enum): Not in text output. Agent must call `--json` to get it, then parse a different structure.
- **Symbol end line**: The map gives start line only. Agent cannot know where a function ends without reading a range and counting manually or calling `--full`.
- **Visibility modifier** (pub vs pub(crate) vs private): Not in text output or JSON. Agent must read the range to determine if a symbol is exported.
- **Enclosing scope** for methods: Python methods appear as flat entries. `greet` and `__init__` appear with no indication they belong to `User`. Agent must read the surrounding lines to infer nesting.
- **Range clamp notice**: When `:1-1000000` is requested and EOF is at line 235, no notice is returned. Agent doesn't know whether the file is 10 lines or 1000 lines from the output alone.

---

## Proposed Next Slice Candidates

These are observations only. No implementation recommended without a reviewed design.

1. **Parse error for invalid ranges**: Return a parse error (exit 1, message to stderr) instead of mangling the path. "invalid range format ':abc-xyz'" is actionable. "not found: /path/file.rsbc-xyz" is not.

2. **`kind` column in text output**: Add it as a 3rd column: `12  fn  main`. Already present in JSON. No schema change needed, just render it.

3. **Binary file exit code**: Return exit 1 when a binary file is detected. Error message already exists; the exit code is wrong.

4. **Empty entry detection in range overlap**: When a batch entry produces empty output (range overlap or other cause), emit a one-line note rather than a blank block. "no content at a.rs:5-15 (overlaps prior range)" or just an explicit empty marker.

5. **Batch JSON shape unification**: Consider a single top-level key for all reads — `{"entries":[...]}` — and let the caller iterate. This removes the single/multi branch that every JSON consumer must implement.

---

## 8v Feedback (Friction During This Audit)

### Empty file within project — cannot test without writing

**Exact command:** `mktemp /tmp/empty_test_XXXXXX.rs && 8v read /tmp/empty_test_XXXXXX.rs`

**What happened:** `8v read` rejected the path with "symlink escapes project directory". The file is not a symlink; it is outside the project root.

**What was expected:** Either (a) reads work on any absolute path the user owns, or (b) the error says "path is outside project root" not "symlink escapes".

**Friction level:** Medium. The error message conflates two different security checks. "Symlink escapes" is a symlink traversal error. A plain file outside the project root is a different constraint. The message sent me looking for a symlink that did not exist.

---

### Range with invalid format — misleading error

**Exact commands:**
```
8v read /path/symbols.rs:-10
8v read /path/symbols.rs:abc-xyz
```

**What happened:**
- `:-10` → "not found: /path/symbols.rs" (path stripped, range discarded)
- `:abc-xyz` → "not found: /path/symbols.rsbc-xyz" (`:a` consumed into path)

**What was expected:** "invalid range format" with exit 1.

**Friction level:** High for agents. A programmatic caller that sends a wrong range format gets back a "file not found" error when the file exists. Impossible to distinguish from a genuinely missing file without re-checking the filesystem.

---

### Binary file exit code inconsistency

**Exact command:** `8v read /path/to/favicon-32x32-eab170b8.png`

**What happened:** Error message on stdout, exit 0.

**What was expected:** Exit 1. Error on stderr.

**Friction level:** Medium. Exit code is the cheapest signal an agent has. Exit 0 on a failed read means the agent's error-detection logic passes this through as success.

---

### Batch range overlap — silent empty entry

**Exact command:** `8v read symbols.rs symbols.rs:1-10 symbols.rs:5-15`

**What happened:** Third entry returns empty content block with no explanation.

**What was expected:** Either an error noting the overlap, or at minimum the overlapping lines (since they exist).

**Friction level:** Medium. An agent building a sliding-window read to cover a large function body will hit this and get silent gaps.

---

### `--json` no trailing newline

**Exact command:** `8v read symbols.rs --json | wc -c`

**What happened:** Valid JSON, no trailing newline. `wc -c` showed N bytes with no `\n` at end.

**What was expected:** Trailing newline, consistent with Unix stdout conventions and `jq` compatibility.

**Friction level:** Low. Most JSON parsers handle this. Pipelines using line-by-line reads (e.g., `while IFS= read -r line`) will silently drop the last line.
