# QA Audit: `8v write` — 2026-04-20

Pure observation. No code changes. Binary under test: `target/debug/8v` (built 2026-04-20 11:41).
Scratch dir: `target/qa-scratch/` (inside repo; `/tmp` tests failed — see §5).

---

## §1. Form-by-form table

| # | Form | Exit 0? | Stdout | Stderr | File mutated? | Notes |
|---|------|---------|--------|--------|---------------|-------|
| T1 | `write path:N "content"` (replace single line) | yes | `path  replaced\n  - old\n  + new` | empty | yes | Clean |
| T2 | `write path:N-M "content"` (replace range) | yes | `path  replaced\n  - ...\n  + new` | empty | yes | Clean |
| T3 | `write path:N --delete` | yes | `path  deleted (N lines)\n  - ...` | empty | yes | Clean |
| T4 | `write path:N --insert "content"` | yes | `path  inserted\n  + content` | empty | yes | Inserts BEFORE line N |
| T5 | `write path --find "old" --replace "new"` (1 match) | yes | `path  replaced (1 occurrence)` | empty | yes | Clean |
| T6 | `write path --find "old" --replace "new"` (0 matches) | no | empty | `error: Error: no matches found for "..." in path. Read the file…` | no | Good recovery message |
| T7 | `write path --find "old" --replace "new"` (multiple matches, no --all) | no | empty | `error: Error: found N occurrences … use --all` | no | Clean guard |
| T8 | `write path --find "old" --replace "new" --all` | yes | `path  replaced (N occurrences)` | empty | yes | Clean |
| T9 | `write path --append "content"` | yes | `path  appended` | empty | yes | Content verbatim; no auto-newline |
| T10 | `\n` escape in content | yes | shows newline in diff | empty | yes | 8v parses `\n` → newline |
| T11 | `\t` escape in content | yes | shows tab in diff | empty | yes | 8v parses `\t` → tab |
| T12 | `\\` escape in content | yes | shows `\` in diff | empty | yes | 8v parses `\\` → backslash |
| T13 | Content with single quotes | yes | correct diff | empty | yes | Shell quoting works |
| T14 | Empty string content | no | empty | `error: content cannot be empty for replace/insert…` | no | Good guard |
| T15 | Invalid range (start > end) | no | empty | `error: Error: invalid range: start (5) > end (3)` | no | Clean |
| T16 | Invalid range (line 0) | no | empty | `error: Error: line number must be at least 1, got 0` | no | Clean |
| T17 | Range beyond EOF | no | empty | `error: Error: line 99 is out of range…` | no | Clean |
| T18 | Nonexistent file (with :N) | no | empty | `error: Error: failed to read file: not found: path` | no | Double-prefix `error: Error:` |
| T19 | Nonexistent file (--append) | no | empty | `error: file does not exist: path\n  to create…` | no | No double-prefix here; hint to use `--force` |
| T20 | Existing file without :N (create-mode conflict) | no | empty | `error: Error: file already exists: path\n  to replace entire file: add --force…` | no | Good recovery hints |
| T21 | Nonexistent file without :N (create mode) | yes | `path  created (N lines)` | empty | yes | `--force` NOT required for create on nonexistent |
| T22 | Directory as path | no | empty | `error: Error: path is a directory: path` | no | Clean |
| T23 | Binary file | no | empty | `error: Error: binary file detected: path` | no | exit=1 (unlike `read` which exits 0 on binary) |
| T24 | Read-only file | no | empty | `error: Error: permission denied: path` | no | Clean |
| T25 | Symlink target | no | empty | `error: failed to read file: symlink escapes project directory: path` | no | Symlinks blocked even within project |
| T26 | File >10MB | no | empty | `error: Error: file too large: N bytes (max 10485760)` | no | Clean |
| T27 | `--force` on existing file | yes | `path  created (N lines)` | empty | yes | Silently overwrites; "created" even though existing |
| T28 | `--force` on new file | yes | `path  created (N lines)` | empty | yes | Clean |
| T29 | `--force` combined with `--append` | no | empty | `error: Error: cannot combine --insert, --delete, --append, --find, and --force` | no | Clean guard |
| T30 | `--plain` flag | yes | same as default (no color differentiation) | empty | yes | `--plain` and default appear identical |
| T31 | `--human` flag | yes | same as default | empty | yes | All three modes (default/plain/human) identical in text |
| T32 | `--json` success (replace) | yes | `{"path":"...","operation":{"operation":"replace","old_lines":[...],"new_content":"..."}}` | empty | yes | Full diff in JSON |
| T33 | `--json` success (delete) | yes | `{"path":"...","operation":{"operation":"delete","deleted_lines":[...]}}` | empty | yes | Clean |
| T34 | `--json` success (insert) | yes | `{"path":"...","operation":{"operation":"insert","content":"..."}}` | empty | yes | Clean |
| T35 | `--json` success (append) | yes | `{"path":"...","operation":{"operation":"append"}}` | empty | yes | No content in JSON for append |
| T36 | `--json` success (find_replace) | yes | `{"path":"...","operation":{"operation":"find_replace","count":N}}` | empty | yes | No old/new text in JSON for find_replace |
| T37 | `--json` error | no | empty | `error: Error: failed to read file: not found: path` | no | Error is NOT JSON even with --json flag |
| T38 | `--find` with `\n` in pattern | no | empty | `error: Error: no matches found for "foo\nbar"…` | no | `\n` in `--find` not expanded; literal `\n` searched |
| T39 | `--find --replace ""` (empty replace) | yes | `path  replaced (1 occurrence)` | empty | yes | Delete via find/replace works |
| T40 | Insert at line 1 | yes | `path  inserted\n  + content` | empty | yes | Content inserted before first line |
| T41 | Insert at N=len+1 | yes | `path  inserted\n  + content` | empty | yes | Content appended at end |
| T42 | Delete all lines | yes | `path  deleted (N lines)\n  - ...` | empty | yes | File becomes empty (0 bytes) |
| T43 | Range replace with `\n` in content | yes | diff shows individual lines | empty | yes | Multi-line content via `\n` works in content arg |
| T44 | CRLF file line-ending preservation | yes | correct diff | empty | yes | CRLF preserved after replace |
| T45 | LF file line-ending preservation | yes | correct diff | empty | yes | LF preserved after replace |
| T46 | No trailing newline: --append | yes | `path  appended` | empty | yes | Appended content immediately follows last byte (no auto-separator) |

---

## §2. Top-5 agent-friction issues

### AF-1 (HIGH): `--force` semantics are asymmetric and undocumented for create mode

**Observed behavior:**
- `8v write path "content"` on a nonexistent file → **creates the file, exit 0**. `--force` NOT required.
- `8v write path "content"` on an existing file → exit 1, error suggests `--force`.
- `8v write path --force "content"` on an existing file → **silently overwrites**, emits `created (N lines)` even though the file was pre-existing.

**Agent friction:** An agent that reads the error hint `to replace entire file: add --force` will learn `--force` = "I intend to overwrite." But the reverse is also true: without `--force`, agents creating new files will accidentally succeed even when they think they're doing a safe "create-only" operation. The word "created" in output for an overwrite is misleading noise.

**Impact:** High. Every agent scripting file-creation workflows hits this.

---

### AF-2 (HIGH): Error messages are not JSON-formatted when `--json` is passed

**Observed behavior:**
```
$ 8v write nonexistent:1 "x" --json
# stdout: (empty)
# stderr: error: Error: failed to read file: not found: /path/...
# exit: 1
```

Errors always go to stderr as plain text, regardless of `--json`. This is inconsistent with the `--json` contract an agent expects: "all structured output via `--json`."

**Comparison:** `read` error with `--json` → `error: 8v: not found: path` (stderr, plain). Pattern is system-wide but especially painful for `write` since agents that rely on parse-and-branch on JSON output have no structured failure signal.

**Impact:** High. Agents using `--json` for reliable parsing must also handle unstructured stderr.

---

### AF-3 (MEDIUM): Double-prefix in error messages (`error: Error:`)

**Observed behavior:**
```
error: Error: failed to read file: not found: /path/...
error: Error: line N is out of range...
error: Error: binary file detected: path
error: Error: file already exists: path
```

But some errors use single prefix:
```
error: file does not exist: path    (--append on nonexistent)
error: content cannot be empty...   (empty content)
```

**Inconsistency:** Two error-prefix styles in the same command. The `error: Error:` double-prefix is noise. Agents parsing error messages to detect specific failure modes see different shapes for logically similar failures.

**Impact:** Medium. Does not block operations but makes error-type detection brittle.

---

### AF-4 (MEDIUM): `--find` does not expand `\n` in pattern (but content arg does)

**Observed behavior:**
```
$ 8v write file --find "foo\nbar" --replace "X"
error: Error: no matches found for "foo\nbar" in ...
```

The `\n` in `--find` is **not** expanded to a newline — it is searched as a literal `\n` two-character sequence. But `\n` in the content argument **is** expanded (confirmed T10). This asymmetry means multi-line find/replace is impossible and agents that try `--find "line1\nline2"` will always get a confusing "no matches found" error without any hint about why.

**Impact:** Medium. Agents attempting multi-line block replacements hit a silent limitation with misleading error.

---

### AF-5 (MEDIUM): Symlink paths rejected even within project boundary

**Observed behavior:**
```
$ 8v write symlink-inside-project:1 "x"
error: failed to read file: symlink escapes project directory: /path/to/symlink
```

Even when the symlink target resolves to a file inside the project root, the symlink itself is blocked. This is the strictest form of symlink safety — the path traversal (symlink → target) is treated as "escaping." On macOS, `/tmp` is a symlink to `/private/tmp`, making any `/tmp`-based scratch work impossible without awareness of this constraint. The error message says "escapes project directory" even when the resolved target is inside the project.

**Impact:** Medium. Agents that construct paths through symlinks (common in monorepos and macOS default `/tmp`) will see confusing failures.

---

## §3. Atomicity and durability

**Evidence collected (indirect):** No `.tmp` or `.bak` files are left after writes (T44 inspection). The binary is ~38MB and built in debug mode — no source was inspected per the audit constraint.

**What can be stated from observation:**
- Writes complete atomically from the file system perspective: no partial files observed, no temp file artifacts.
- `--delete all lines` produces a 0-byte file (not a missing file), which is correct.
- On write failure (permission denied, file too large, binary), the original file is untouched.
- Whether the implementation uses `write_all + rename` (atomic) or `truncate + write` (non-atomic) cannot be confirmed without source inspection. The behavior is consistent with atomic writes, but cannot be proven from output alone.

**Gap:** If 8v uses in-place write (open existing, write, close), a crash mid-write produces a corrupt file. If it uses temp-file + rename, it is atomic. Observation is consistent with both; source review required to confirm.

---

## §4. Line-ending and newline preservation

| Test | Behavior | Correct? |
|------|----------|----------|
| CRLF file, replace single line | `\r\n` preserved on all lines | yes |
| LF file, replace single line | `\n` preserved on all lines | yes |
| Trailing newline present, replace | trailing newline preserved | yes |
| No trailing newline, --append | appended content follows last byte with no separator (`...last_byte.appended`) | yes, matches doc: "add \n if you want trailing newline" |
| CRLF file, replaced line | replaced line gets `\r\n` (matches file's existing style) | yes |
| Mixed-ending detection | not tested (risk: if a file has both CRLF and LF, behavior undefined) | unknown |

Line-ending preservation works correctly for homogeneous files. No issues found.

---

## §5. Error-shape consistency (vs. other commands)

| Command | Error format | Error channel | Example |
|---------|-------------|---------------|---------|
| `write` (most errors) | `error: Error: <message>` | stderr | `error: Error: failed to read file: not found: path` |
| `write` (some errors) | `error: <message>` | stderr | `error: file does not exist: path` |
| `read` | `error: 8v: <message>` | stderr | `error: 8v: not found: path` |
| `search` | `error: cannot access path '...': <OS error>` | stderr | `error: cannot access path '...': No such file or directory (os error 2)` |
| `ls` | `error: cannot access path '...': <OS error>` | stderr | same as search |

**Findings:**
1. `write` has two internal error shapes (double-prefix and single-prefix) — inconsistent within the command.
2. `write` prefix `error: Error:` differs from `read` prefix `error: 8v:` — no unified error schema.
3. `search` and `ls` use raw OS error strings (`No such file or directory (os error 2)`) — Rust `std::io::Error` leak.
4. None of the commands format errors as JSON when `--json` is passed. Error channel is always plain stderr text.

---

## §6. Proposed next slice candidates

These are observations only — no designs proposed.

| Priority | Issue | Type |
|----------|-------|------|
| 1 | Unify error prefix across all commands (pick one schema) | CONTRACT |
| 2 | `--json` errors should emit structured JSON to stdout (or at minimum to stderr) | CONTRACT |
| 3 | Document `--force` semantics clearly: "create-mode" vs "overwrite-mode" distinction | DOC |
| 4 | `--find` with `\n` either expands (match content-arg behavior) or emits a clear "multi-line find not supported" error | BEHAVIOR |
| 5 | `--force` success output should say "overwritten" when file pre-existed, not "created" | RENDER |
| 6 | Symlink error message: "symlink escapes project directory" even when resolved path is inside project — misleading | CONTRACT |
| 7 | `--json` append response omits content — add `appended_content` field for agent-parseable confirmation | RENDER |
| 8 | `--json` find_replace omits old/new text — add `old_text`/`new_text` to match replace/delete fields | RENDER |

---

## §7. 8v feedback (friction observed during this audit)

**FB-1 (HIGH): Project-boundary check blocks all /tmp work on macOS**

Attempting to use `/tmp` as a scratch directory for testing failed immediately:
```
error: failed to read file: symlink escapes project directory: /tmp/8v-write-qa/t1.txt
```
On macOS, `/tmp` → symlinks to → `/private/tmp`. Both `/tmp` and `/private/tmp` were rejected. The error says "escapes project directory" which is confusing — the scratch dir was *outside* the project, which was the intent. 8v enforces writes only within the detected project root; this is a deliberate safety constraint, but the error message does not explain this contract. An agent's first attempt will always fail without this knowledge.

**Required workaround:** Create scratch dir inside the repo (`target/qa-scratch/`). Any agent that doesn't know this constraint will waste 1-2 turns diagnosing the rejection.

**Recommendation for CLAUDE.md / MCP description:** State explicitly "8v write only operates on files within the detected project root. /tmp on macOS is rejected."

---

**FB-2 (MEDIUM): `--force` semantics discoverable only through trial-and-error**

The `--force` flag description in `--help` says "Force overwrite existing file (create mode only)" but:
- Create mode works without `--force` on nonexistent files
- `--force` is required to overwrite existing files
- `--force` combined with `--append`, `--insert`, `--delete`, `--find` is rejected

An agent reading `--help` first gets correct flag list, but the mental model of "when is --force needed" requires reading the error from a failed write attempt. The help text should clarify: `--force` is needed only when replacing the entire content of an existing file.

---

**FB-3 (LOW): `--json` error schema gap causes parse failures in automated pipelines**

Agents using `--json` for reliable machine-readable output need to handle a dual-channel pattern: JSON on stdout for success, plain text on stderr for failure. This is not documented anywhere in the help text or MCP description. An agent that assumes `--json` guarantees parseable output will fail silently when errors occur.

---

**FB-4 (LOW): Success output format (`--plain`, `--human`, default) are indistinguishable**

All three output modes produce the same text. If the intent is for `--plain` to suppress the diff and `--human` to add color/icons, that distinction is not implemented. Agents receiving `--plain` output cannot rely on a minimal "just the path" response — they get the full diff regardless.
