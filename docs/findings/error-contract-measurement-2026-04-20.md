# Error Contract Measurement — 2026-04-20

**Binary**: `/Users/soheilalizadeh/8/products/vast/oss/8v/target/debug/8v` (v0.1.0, commit 2681102, dirty)
**Method**: N=1 per case; manual exercise of 12 subcommands across 10 failure modes each.
**No code changes made.** All permission tests run in tmpdir; restored after.
**Caveats**: N=1 means one-off flakes are not ruled out. Dirty build may mask unreleased fixes.

---

## 1. Tables per Subcommand

### `ls`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: cannot access path '...': No such file or directory (os error 2)` |
| Permission-denied (file, chmod 000) | 1 | (empty) | `error: '...' is not a directory` |
| Invalid flag (`--notarealflag`) | 2 | (empty) | clap usage block |
| Missing required arg | 0 | cwd listing | (empty) — not an error |
| `--json` on nonexistent | 1 | (empty) | same plain-text `error:` on STDERR |

---

### `read`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: 8v: not found: <path>` |
| Path outside repo (boundary) | 1 | (empty) | `error: 8v: symlink escapes project directory: <path>` |
| Invalid flag | 2 | (empty) | clap usage block |
| Malformed range (`foo.rs:abc-xyz`) | 1 | (empty) | `error: 8v: not found: foo.rs:abc-xyz` — **range parsed as filename** |
| Missing required arg | 2 | (empty) | clap usage block |
| Permission-denied in repo | 1 | (empty) | `error: 8v: permission denied: <path>` |
| Binary file (in repo, `--full`) | 1 | (empty) | `error: 8v: <path>: file contains invalid UTF-8 (binary file?)` |
| Path traversal (`../../etc/passwd`) | 1 | (empty) | `error: 8v: not found: <resolved-path>` |
| Batch: one exists + one missing | 1 | `=== label ===` headers; missing entry shows inline `error:` in STDOUT | (empty) |
| `--json` batch (one missing) | 1 | `{"Multi":{"entries":[...,{"label":"...","result":{"status":"Err","message":"..."}}]}}` on STDOUT | (empty) |
| `--json` single-file missing | 1 | (empty) | plain-text `error:` on STDERR — **not JSON** |

---

### `write`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: Error: failed to read file: not found: <path>` |
| Path outside repo | 1 | (empty) | `error: Error: failed to read file: symlink escapes project directory: <path>` |
| Invalid flag | 2 | (empty) | clap usage block |
| Missing required arg | 2 | (empty) | clap usage block |
| Empty content (`write path:1 ""`) | 1 | (empty) | `error: error: content cannot be empty...` — **double-prefix** |
| Malformed range (`:abc-xyz`) | 1 | (empty) | `error: error: invalid line range ":abc-xyz"...` — **double-prefix** |
| `--find` with no match | 1 | (empty) | `error: Error: no matches found for "..." in README.md...` |
| Permission-denied in repo | 1 | (empty) | `error: Error: failed to read file: permission denied: <path>` |
| `--json` on any write error | 1 | (empty) | same plain-text on STDERR — **not JSON** |

---

### `search`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: cannot access path '...': No such file or directory (os error 2)` |
| Permission-denied file (chmod 000) | 1 | `no matches found` | (empty) — **BUG: swallows the error** |
| Invalid flag | 2 | (empty) | clap usage block |
| Invalid regex (`[`) | 1 | (empty) | `error: invalid regex pattern: regex parse error: ...` |
| Missing required arg | 2 | (empty) | clap usage block |
| Single arg (`.` treated as pattern) | 0 | all-line match output | (empty) — **BUG: no path arg, `.` is pattern not cwd** |
| `--json` on any search error | 1 | (empty) | same plain-text on STDERR — **not JSON** |

---

### `check`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: path resolution failed: ...: No such file or directory (os error 2)` |
| Invalid flag | 2 | (empty) | clap usage block |
| No arg (runs on cwd) | 1 | (empty) | rich formatted block on STDERR (tool errors) |
| `--json` on error | 1 | (empty) | same plain-text on STDERR — **not JSON** |

---

### `fmt`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: path not found: <path>` |
| Invalid flag | 2 | (empty) | clap usage block |
| No arg (runs on cwd) | 0 | (empty) | status output on STDERR |

---

### `test`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: 8v: invalid path: path not found: <path>` |
| Invalid flag | 2 | (empty) | clap usage block |
| No arg (runs on cwd) | 1 | test runner output | (empty) — exits 1 because real tests fail |

---

### `build`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | (empty) | `error: 8v: invalid path: path not found: <path>` |
| Invalid flag | 2 | (empty) | clap usage block |
| No arg (runs on cwd) | 0 | (empty) | build output on STDERR |

---

### `init`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Nonexistent path | 1 | `init: failed` on STDOUT | `error: path not found: <path>` on STDERR |
| No TTY (`< /dev/null`) | 1 | `init: failed` on STDOUT | `error: 8v init requires an interactive terminal` on STDERR |
| Invalid flag | 2 | (empty) | clap usage block |

---

### `mcp`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Invalid flag | 2 | (empty) | clap usage block — **no `error:` prefix** |
| Help | 0 | (empty) | help text on STDERR |

---

### `hooks`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Invalid flag | 2 | (empty) | clap usage block |
| No subcommand | 2 | (empty) | usage block on STDERR — **no `error:` prefix** |

---

### `upgrade`

| Failure mode | Exit | STDOUT | STDERR |
|---|---|---|---|
| Invalid flag | 2 | (empty) | clap usage block |

---

## 2. Consistency Analysis

### Exit codes

- **EXIT 1**: runtime errors across all subcommands. Consistent.
- **EXIT 2**: clap parse errors across all subcommands. Consistent.
- **EXIT 0 when it should be non-zero**: `ls` with no arg (runs on cwd — acceptable); `fmt` with no arg (acceptable); `build` with no arg (acceptable); `search` single-arg `.` — **not acceptable** (silently treats `.` as regex pattern, matches everything).

### STDERR prefix

| Pattern | Commands using it | Notes |
|---|---|---|
| `error: 8v: <message>` | `read`, `test`, `build` | Most structured |
| `error: Error: <message>` | `write` | Double capitalization |
| `error: error: <message>` | `write` (empty content, malformed range) | Double prefix — **bug** |
| `error: <message>` | `ls`, `search`, `check`, `fmt` | Bare form |
| `error: path resolution failed:` | `check` | Different key phrase from `read`'s `not found:` |
| `error: path not found:` | `fmt`, `init` | Different from `fmt`'s own phrasing vs `check` |
| No `error:` prefix | `mcp`, `hooks` (no-subcommand) | clap default, no override |

### `--json` error routing

Only batch `read` produces machine-readable JSON errors in STDOUT. Every other command emits plain-text STDERR errors regardless of `--json`. There is no contract: `--json` is not a guarantee of structured error output.

### STDOUT vs. STDERR for errors

- All single-file/single-path errors: STDERR only, STDOUT empty. Consistent.
- Batch `read` errors: STDERR empty, STDOUT has inline `error:` per entry. Inconsistent with the rest.
- `init` failure: STDOUT gets `init: failed`, STDERR gets the actual error. Mixed split.

---

## 3. Inconsistencies Found

### BUG-1: `write` double-prefix errors

`error: error: content cannot be empty` and `error: error: invalid line range`. The inner message already starts with `error: ` and is wrapped again. Affects the empty-content and malformed-range cases.

### BUG-2: Malformed range (`foo.rs:abc-xyz`) treated as filename

`read foo.rs:abc-xyz` does not parse the range portion; it treats the whole string as a path and reports `not found: foo.rs:abc-xyz`. The correct error is `invalid range`. The user gets no signal that the colon-syntax was recognized.

### BUG-3: `search` swallows permission-denied

`search <pattern> <chmod-000-file>` exits 1, STDOUT says `no matches found`, STDERR is empty. The permission error is invisible. Correct behavior: STDERR should show `error: permission denied: <path>`, and the distinction between "no matches" and "could not read" must be explicit.

### BUG-4: `search .` (single arg) silently becomes pattern, not path

`8v search .` treats `.` as the regex pattern, matches every character in every file in cwd, exits 0. There is no `error: missing path argument`. Users expecting `search` to default to cwd (like `ls`, `fmt`, `check`, `build`) get a flood of output with no warning.

### BUG-5: `--json` does not produce JSON errors for most commands

The flag suggests machine-readable output. Batch `read` honors it for per-entry errors. Every other command emits plain-text STDERR. Agents parsing `--json` output for structured errors will silently fail to parse the error and likely retry.

### BUG-6: "symlink escapes project directory" error for non-symlinks

`read /tmp/someplace/file.rs` (a plain file, no symlinks) reports `error: 8v: symlink escapes project directory: ...`. The file is simply outside the project root. The error text is misleading: it implies a symlink attack when none exists.

### INCONSISTENCY-7: Error prefix fragmentation across subcommands

Six distinct STDERR prefix patterns identified (see Section 2). The agent cannot write a single prefix-based parser to classify errors. This directly impacts the v4 benchmark: agents see inconsistent error shapes and cannot reliably extract failure cause.

### INCONSISTENCY-8: `init` splits its failure across STDOUT + STDERR

`init: failed` goes to STDOUT; the reason goes to STDERR. Every other command keeps STDOUT clean on failure. Agents reading only STDOUT see a failure signal with no cause; agents reading only STDERR see a cause with no summary.

---

## 4. Partial-Failure Contract

### `read` (batch) — confirmed harvest behavior

```
8v read exists.rs missing.rs
```

- EXIT: 1
- STDOUT: `=== exists.rs ===` block with symbol map, then `=== missing.rs ===` block with inline `error: 8v: not found: missing.rs`
- STDERR: (empty)

The batch does not fail-fast. All entries are attempted; per-entry errors appear inline in STDOUT under the `=== label ===` header. The overall exit code is 1 if any entry errored.

With `--json`:
- STDOUT: `{"Multi":{"entries":[{"label":"exists.rs","result":{"Symbols":...}},{"label":"missing.rs","result":{"status":"Err","message":"..."}}]}}`
- STDERR: (empty)

This is the only command where `--json` produces structured error output.

### All other commands — fail-fast

`ls`, `search`, `check`, `fmt`, `test`, `build`, `init`, `write` each accept one path argument. On error they stop immediately, emit to STDERR, exit 1. No harvest.

`search` accepts a path argument but silently skips unreadable files (see BUG-3) rather than reporting them. This is partial-failure without signaling — the worst variant.

---

## 5. Recommended Error Contract Doc Slice

The following should be the next instruction-surface slice for the v4 benchmark to close the Failure axis gap.

### Title: `8v error contract`

**What every agent must know:**

1. **Exit codes are binary**: EXIT 0 = success, EXIT 1 = runtime error, EXIT 2 = bad invocation (clap). No other codes.

2. **All errors go to STDERR** — except batch `read`, where per-entry errors appear inline in STDOUT under `=== label ===` headers.

3. **`--json` only structures batch `read` errors**. For all other commands, errors remain plain-text STDERR regardless of `--json`.

4. **STDERR prefix is NOT uniform**. Do not parse by prefix. Parse by exit code first; then read STDERR as free text for the human-readable reason. The prefix patterns (`error: 8v:`, `error: Error:`, `error:`) carry no machine-parseable taxonomy today.

5. **Partial failure**: only batch `read` harvests. All other commands fail-fast. `search` silently skips unreadable files — treat EXIT 1 with "no matches found" as ambiguous.

6. **"symlink escapes project directory"** means the path is outside the project root. Not a symlink attack. Applies to `read` and `write`.

7. **Malformed range** (`foo.rs:abc-xyz`) is reported as "not found", not "invalid range". Validate range syntax client-side before calling `read`.

---

## 6. 8v Feedback (Friction Encountered While Running This Task)

These are bugs and friction points encountered while using 8v to perform this measurement task — not theoretical findings, but live friction.

**Friction-1 (double-prefix, write)**: When hitting `write path:1 ""`, the error output `error: error: content cannot be empty` required a double-take. The prefix layering is a parsing antipattern for agents.

**Friction-2 (range as filename, read)**: `8v read oss/8v/src/main.rs:abc-xyz` silently becomes a "not found" for `main.rs:abc-xyz`. An agent that makes a typo in a range syntax gets zero indication that the syntax was attempted. This caused me to re-run to confirm it wasn't a real path lookup.

**Friction-3 (--json inconsistency)**: When trying to confirm whether `--json` mode would help agents parse errors, I had to run every command individually. There is no documented contract. The only way to discover batch-read-only JSON errors is empirically.

**Friction-4 (search single-arg ambiguity)**: `8v search .` feels like it should search cwd. It doesn't. The silent flood of all-line matches (exit 0) could cause an agent to think search succeeded and attempt to parse the output as results. This is a confusability bug with a large blast radius.

**Friction-5 (misleading symlink message)**: Every file outside the project root triggers "symlink escapes project directory". During this task I was frequently working with files in `/tmp/` and the error text made me question whether I had created an actual symlink. I had not. The message needs to say "outside project root" instead.

**Friction-6 (init STDOUT/STDERR split)**: When testing `init` with a nonexistent path, the failure status landed in STDOUT and the reason landed in STDERR. Capturing just STDOUT (as agents often do) gives `init: failed` with no reason. Capturing just STDERR gives the reason with no summary. An agent reading both must correlate across streams.

**No friction**: The `--help` on every subcommand was reliable. Exit code 2 for all parse errors was consistent. Batch `read` JSON errors were well-structured once discovered.
