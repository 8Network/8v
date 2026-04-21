# Design: 8v Error/Exit-Code Contract (Level 1)

**Date**: 2026-04-20
**Status**: Draft — awaiting counterexample review
**Slice**: B1 (instruction-surface, error/failure axis only)
**References**:
- `docs/findings/error-contract-measurement-2026-04-20.md` — measured behavior across 12 subcommands
- `docs/findings/instruction-clarity-test-2026-04-20.md` — v4 benchmark; Failure axis 2.33/10

---

## 1. The Problem

The Failure axis of the v4 instruction-clarity benchmark scored 2.33/10 across all six evaluator runs (N=3 Opus, N=3 Sonnet). This is not a scoring anomaly. All six independent evaluators named the same root cause: there is no documented error contract. From the v4 findings:

> "Five of my thirteen gap callouts (Q7, Q18, Q26, Q27, Q28, Q29) are all the same root cause: no documented error model." — opus-run2, Q22

Agents that cannot reason about failure paths retry on ambiguity, parse errors with ad-hoc heuristics, and silently lose the distinction between "no results" and "could not read." The measurement doc confirmed these ambiguities exist in the live binary, not just in the documentation.

This slice defines the contract that will be documented in the instruction surfaces. It does not define implementation work beyond the four confirmed bugs already in the codebase.

---

## 2. The Contract

### 2.1 Exit Codes

Three codes. No others.

| Code | Meaning |
|------|---------|
| 0 | Success — the command completed its intended work |
| 1 | Runtime error — a path, permission, regex, or content condition failed |
| 2 | Invocation error — invalid flag, missing required argument (clap parse failure) |

**Why this set:** The current binary already uses exactly this split consistently (measurement doc Section 2, Exit codes row). Exit 0 = success, exit 1 = runtime, exit 2 = clap — this is empirically true for all 12 subcommands. The contract documents reality. No behavior change needed for exit codes.

**Why not more codes (e.g., 3 = partial failure, 4 = permission):** Agents branch on exit code. A larger set increases branching complexity without a measured agent need. The v4 benchmark evaluated models on what they expected the codes to be — all six runs assumed a small set. Granular codes would require updated instruction text and additional evaluator training. The cost exceeds the benefit at this stage.

### 2.2 Error Channel (stderr vs. stdout)

**Canonical rule**: On any error, the human-readable message goes to stderr. Stdout is reserved for successful output.

**One exception — batch `read`**: When a batch `read` call includes multiple paths, per-entry errors appear inline in stdout under the `=== label ===` header alongside successful entries. The overall exit is still 1 if any entry failed. This is the designed harvest behavior: agents can consume what succeeded without a second call. The `--json` form emits `{"status":"Err","message":"..."}` per entry in the batch envelope, which is already structured.

**`init` is out of contract**: `init` currently emits `init: failed` to stdout and the reason to stderr. This violates the canonical rule. It is a known bug (BUG-listed below) and will be fixed as part of this slice.

**Why this split**: All six v4 evaluators assumed errors would be on stderr. Every model-level recommendation (Q22, Q24, Q22 variants) specified "errors to stderr." The measurement confirmed this is already the behavior for 11 of 12 subcommands. The one exception (`init`) is a bug, not a design choice.

### 2.3 stderr Prefix Convention

**One canonical prefix**: `error: <message>`

No secondary prefix. No tool name in the prefix. No capitalization variants.

The measurement found six distinct prefix patterns currently in use:

| Current pattern | Commands | Decision |
|-----------------|----------|---------|
| `error: 8v: <message>` | `read`, `test`, `build` | Reduce to `error: <message>` |
| `error: Error: <message>` | `write` | Fix to `error: <message>` |
| `error: error: <message>` | `write` (double-prefix) | Fix to `error: <message>` |
| `error: <message>` | `ls`, `search`, `check`, `fmt` | Keep as-is |
| `error: path resolution failed:` | `check` | Normalize message text |
| No `error:` prefix | `mcp`, `hooks` | Add prefix |

**Why single prefix**: The measurement doc (Section 2, STDERR prefix) confirmed that agents cannot write a single prefix-based parser today. Six patterns means six cases to handle. The v4 evaluators (Q22, Q24 consensus) requested a single predictable error format so agents can filter stderr by prefix. Stripping the tool name (`8v:`) from the prefix makes the pattern simpler and identical to the de-facto convention in Unix tooling — it adds no information an agent needs (the agent already knows it called `8v`).

**What the message text after the prefix looks like**: The canonical form is `error: <verb>: <subject>`. Examples: `error: not found: <path>`, `error: permission denied: <path>`, `error: invalid range: <spec>`. The verb provides the category; the subject provides the location. This is already the pattern used by `read` and `write` for file-system errors and is the model for the rest.

### 2.4 `--json` Error Shape

**Single schema, all commands**:

```
{
  "error": "<human-readable message>",
  "code": "<short machine-readable key>"
}
```

Optional fields present when applicable:

```
{
  "error": "<message>",
  "code": "<key>",
  "path": "<affected path>",   // when the error is path-specific
  "line": <number>             // when the error is line-specific (write, read range)
}
```

**Where it goes**: stdout (not stderr), alongside exit code 1. When `--json` is passed and an error occurs, stdout contains the JSON object above and stderr is empty.

**Why stdout for JSON errors**: Agents parsing `--json` output read stdout. Mixing the JSON success path (stdout) with a plain-text error path (stderr) requires the agent to poll two streams and merge. The v4 measurement confirmed this is exactly what breaks today: "Agents parsing `--json` output for structured errors will silently fail to parse the error and likely retry" (measurement doc, BUG-5). Every v4 evaluator's recommendation assumed JSON errors would appear on stdout.

**Why `code` field**: A machine-readable error key allows agents to branch on error type without parsing natural-language message text. The current behavior forces agents to substring-match message text, which breaks on any wording change. The `code` field is stable; message text can evolve. The minimal initial set of codes: `not_found`, `permission_denied`, `outside_project`, `invalid_range`, `invalid_regex`, `content_empty`, `no_match` (for `--find`), `invocation_error`.

**Batch `read` --json**: Batch `read` already produces structured per-entry errors in the `Multi` envelope. The per-entry error shape should conform to the same schema: `{"status":"Err","error":"<message>","code":"<key>"}`. This is a shape normalization, not a behavior change.

**Exception — `check`, `fmt`, `test`, `build`**: These commands delegate to external tools (cargo, go, etc.). Their `--json` output captures `{"exit_code":...,"tool":"...","output":"...","duration_ms":...}`. They do not produce the `{"error":...}` envelope because their failures are captured in the subprocess output. This is by design and should be documented as the explicit exception.

**Why not a `path` and `line` at the top level always**: Optional fields only when meaningful. A permission-denied error on a path is naturally path-scoped. An invalid-regex error has no path. Mandatory empty fields would be noise in the token stream, contrary to the progressive principle.

### 2.5 Partial-Failure Behavior

**Harvest (do not fail-fast)**: batch `read` only. Attempt all inputs; emit per-entry results and errors inline; exit 1 if any entry failed.

**Fail-fast (stop on first error)**: all other commands. On error, emit to stderr (or JSON to stdout with `--json`), exit 1.

**`search` special case**: `search` currently silently swallows permission-denied errors and emits `no matches found` (BUG-3 in measurement doc). The correct behavior is fail-fast: emit `error: permission denied: <path>` to stderr and exit 1. "No matches" and "could not read" are different outcomes and must not be conflated.

**Why only `read` harvests**: `read` is the primary discovery command. Agents routinely issue multi-file reads to understand a codebase. A single missing file should not abort the entire read. For `write`, `check`, `fmt`, `test`, `build`, partial success leaves the project in an unknown state — aborting and reporting the first failure is the safe default.

**Why this must be documented explicitly**: The measurement found that `search`'s silent swallowing is the most dangerous partial-failure variant — it exits 1, which signals an error, but the stdout output says `no matches found`, which reads like success. The v4 evaluators could not determine whether "search returned no results" and "search could not read a file" were the same exit path (they were not, but the documentation gave no signal).

---

## 3. Why Each Decision

Every decision above is tied to a specific measurement or run citation. Summary table:

| Decision | Evidence |
|----------|---------|
| Three exit codes | Measurement doc §2: already consistent across 12 commands; no action needed on codes |
| Errors to stderr | All 6 v4 evaluators assumed stderr; 11/12 commands already do this |
| Single `error:` prefix | Measurement doc INCONSISTENCY-7: 6 patterns found; agents cannot parse by prefix |
| JSON errors to stdout | Measurement doc BUG-5: "Agents parsing `--json` will silently fail to parse the error" |
| `code` field | v4 Q22 (all 6 runs): agents need a machine-readable key, not just message text |
| Harvest for batch `read` only | Measurement doc §4: only command where harvest is designed and documented |
| `search` fail-fast on permission | Measurement doc BUG-3: silent swallow is the worst partial-failure variant |

---

## 4. What Does NOT Change

This slice is bounded. The following are explicitly out of scope:

- **Command behavior**: No command changes its operational behavior. Exit codes, success paths, output format for successful runs — all unchanged.
- **`write --find/--replace` multi-occurrence behavior**: Documenting the actual behavior (replace all vs. error on ambiguity) is a separate slice. The v4 benchmark flagged this as a secondary P1 item. It requires a behavior audit first.
- **Instruction surface rewording**: The error contract section will be added to both surfaces (`ai_section.txt`, `instructions.txt`) as a new block. Existing content is not reworded as part of this slice.
- **CLI flag additions**: No new flags. The `--json` error shape is a behavioral normalization of existing `--json` flag behavior.
- **Regex dialect documentation** (`search` `(regex)` clarification): Separate slice; see v4 P1 findings.
- **Surface 1 ↔ Surface 2 wording drift**: Separate slice; see v4 P1 findings.
- **`check`/`fmt`/`test`/`build` success output format**: Flagged in v4 as a gap on the Output axis, not the Failure axis. Separate slice.
- **`read` empty symbol map hint**: Already has an accepted design doc; do not bundle.

---

## 5. Known Bugs to Fix as Part of This Slice

These are bugs confirmed in the measurement doc. They must be fixed for the contract to be true. They are listed here as bugs, not designed — implementation belongs to the implementation design (Level 2).

**BUG-1**: `write` double-prefix errors — `error: error: content cannot be empty` and `error: error: invalid line range`. The inner message already contains `error:` and is wrapped again. (Measurement doc BUG-1)

**BUG-2**: Malformed range (`foo.rs:abc-xyz`) treated as filename — `read` reports `not found: foo.rs:abc-xyz` instead of `invalid range: foo.rs:abc-xyz`. (Measurement doc BUG-2)

**BUG-3**: `search` swallows permission-denied — exits 1 with `no matches found` on stdout and nothing on stderr. Should emit `error: permission denied: <path>` to stderr. (Measurement doc BUG-3)

**BUG-4**: `init` splits failure across stdout (`init: failed`) and stderr (the reason). Should emit the reason to stderr only; `init: failed` on stdout should be removed or moved to stderr. (Measurement doc INCONSISTENCY-8)

These four bugs have a combined surface area narrow enough to fix without risk to unrelated paths. They do not require design; they require matching the output to the contract stated in §2.

---

## 6. Counterexamples and Edge Cases

The following scenarios could falsify or stress the proposed contract. Each must be addressed before implementation proceeds.

**CE-1: Batch `read` partial-success with `--json` — what is the top-level exit code?**
If 3 of 5 files succeed, the current contract says exit 1 because at least one failed. An agent checking only the exit code cannot determine whether to retry all 5 or only the failed 3. The JSON envelope contains per-entry status, but the agent must parse JSON to find which entries failed. If the agent retries the whole batch after a partial failure, it wastes calls on already-successful entries. The contract should explicitly state: exit 1 + per-entry `{"status":"Err"}` means retry only the failed entries. If the documentation does not say this, agents will over-retry.

**CE-2: `search` permission-denied on a directory subtree — fail-fast or partial harvest?**
The proposed contract makes `search` fail-fast on permission-denied. But `search` typically operates on a directory. If one subdirectory is unreadable but 10 others are readable, fail-fast means the agent gets no results at all. The alternative (skip unreadable, emit warning, continue) is partial harvest. The current behavior silently harvests (worst case). The proposed behavior fails fast (potentially throws away useful results). Neither is clearly right. The contract must pick one and justify it, or introduce an explicit `--allow-partial` flag — but new flags are out of scope for this slice. This is an unresolved tension. **RESOLVED: see §7 Resolutions — CE-2.**

**CE-3: `--json` error schema conflicts with existing `check`/`test`/`build` JSON shape.**
The proposed `{"error":"...","code":"..."}` envelope conflicts with the subprocess-capture shape `{"exit_code":...,"stdout":"...","stderr":"..."}`. If an agent treats all `8v --json` errors as the `{"error":...}` schema, it will fail to parse subprocess errors from verify commands. The contract must explicitly document the two schemas and which commands use which. If undocumented, this is a new source of agent confusion — potentially replacing the current problem with a different one. **RESOLVED: see §7 Resolutions — CE-3.**

**CE-4: `--json` on `write` — success has no stdout output today.**
`write` emits nothing to stdout on success. With `--json`, success should presumably emit `{"ok":true}` or similar to stdout so agents can confirm completion without relying on exit code alone. If the contract only defines the error JSON shape and leaves success undefined, agents may interpret empty stdout + exit 0 as an error (degenerate case) or as success (correct). This ambiguity does not exist today because `--json` is not honored at all for `write` errors. Defining the error schema without defining the success schema creates a half-documented surface.

**CE-5: `error: <message>` prefix collision with clap errors.**
clap generates its own error text (exit 2). clap messages do not have the `error:` prefix added by 8v — they have clap's own formatting. If an agent filters stderr for the `error:` prefix to find 8v errors, clap errors (exit 2) may or may not match, depending on clap's output format. If clap also outputs `error: ...`, the agent cannot distinguish invocation errors from runtime errors by prefix alone — it must use exit code 2 vs. 1 as the primary discriminant. The contract must state this explicitly: prefix is only reliable for exit 1 errors. Exit 2 errors use clap's format, which may or may not carry the same prefix.

**CE-6: External-tool error text from `check`/`fmt`/`test`/`build` appears on stderr.**
These commands run cargo, go, etc. and stream their output to stderr. The `error:` prefix in their output comes from the external tool (e.g., `error[E0308]: mismatched types` from rustc), not from 8v. An agent scanning all stderr for `error:` prefixes after running `8v check .` will capture compiler errors, which are not 8v errors and do not conform to the `error: <verb>: <subject>` shape. The contract must clarify that the `error:` prefix convention applies only to 8v's own diagnostic messages, not to passthrough subprocess output.

**CE-7: Multiple concurrent `8v` processes writing to the same event store.**
Not directly an error-contract question, but it affects exit code reliability: if two agents run `8v write` concurrently on the same file, one may exit 1 (collision detected) or exit 0 (last writer wins). The error contract says exit 1 = runtime error, but concurrent-write collisions are not currently detected. An agent relying on exit 0 to mean "my write succeeded" may be wrong if another agent also succeeded. This is deferred as theoretical (per `docs/memory/write_command_hardened.md`) but should be noted as a known gap in the atomicity guarantee.

---

## 7. Resolutions

### CE-2: `search` permission-denied — harvest or fail-fast?

**Pick: harvest with visible warnings.**

Search continues past unreadable files. For every skipped file, 8v emits `error: permission denied: <path>` to stderr (one line per file, never silent). Exit codes stay within the existing three-code contract:

- Exit 0: matches found. Stderr may contain per-file I/O warnings — the agent knows results may be incomplete.
- Exit 1: no matches found, or all files were unreadable. Stderr distinguishes the two sub-cases: empty stderr means the pattern genuinely was not found; non-empty stderr means at least one read failure occurred.

Exit 2 remains clap invocation errors only. No new exit codes are introduced.

**Why not fail-fast (option b):** A directory walk where one subdirectory is unreadable is a routine condition on shared filesystems. Aborting and returning no results is UX-destroying and inconsistent with the ripgrep reference behavior (which continues past unreadable files). The measurement doc Section 4 confirmed this is exactly the case that matters: `8v search "hello" .` with a chmod-000 file already implicitly continues — the current bug is only that the warning is invisible, not that the traversal aborts.

**Why not option (c) — `--allow-partial` flag:** §4 of this document explicitly prohibits new flags for this slice. Ruled out.

**Why stderr non-emptiness is the discriminant:** The command-qa-search findings (Issue 2) state: "There is no way to distinguish [no match vs. read error] from exit code alone. STDERR is always empty." The fix is to make STDERR non-empty when a read failure occurs, not to add new exit codes. An agent can check: `exit 1 + stderr empty` = genuine no match; `exit 1 + stderr non-empty` = partial I/O failure, results may be incomplete.

**What could still be wrong:** A directory with all files unreadable and zero matches returns exit 1 with non-empty stderr. An agent that checks only exit code cannot tell whether any matching work was done. This is an acceptable residual ambiguity: the agent can inspect stderr to determine the cause. The contract must document this explicitly in the instruction surfaces.

---

### CE-3: Two `--json` error schemas for different command classes

**Pick: two-level schema, already partially described in §2.4.**

The two schemas are not in conflict — they cover different failure points in the execution pipeline:

- **8v-side failure** (project not found, tool not installed, path invalid, permission denied before the subprocess runs): `{"error":"<message>","code":"<key>"}` on stdout, exit 1. The agent sees `"error"` as the top-level key.
- **Subprocess-capture** (the subprocess ran; the tool reported failure in its own output): `{"exit_code":<n>,"tool":"...","output":"...","duration_ms":<ms>}` on stdout. The agent sees `"exit_code"` as the top-level key.

**Disambiguation rule for agents:** Check the top-level key. `"error"` present → 8v could not run the subprocess (pre-run failure). `"exit_code"` present → the subprocess ran; inspect `exit_code` and `output` for the tool's own error.

**Why not option (a) — pass-through:** Pass-through leaves the "8v could not start the tool" case unhandled. If cargo is not installed and the agent runs `8v check . --json`, the current binary emits plain-text to stderr and nothing to stdout — neither schema is honored. The two-level design is the minimum change that closes this gap.

**Why not option (b) — always-wrap:** Wrapping the subprocess-capture shape inside `{"error":...,"tool_output":{...}}` forces double-parsing. The agent must unwrap the outer envelope to read `tool_output`, then parse the inner `exit_code`/`stderr` fields. No gain over two-level. The added nesting increases token cost in the agent's response-parsing path.

**Why §2.4 is almost the resolution already:** §2.4 states: "They do not produce the `{"error":...}` envelope because their failures are captured in the subprocess output. This is by design and should be documented as the explicit exception." The CE-3 resolution formalizes what §2.4 describes: it names the two schemas, identifies the top-level key as the discriminant, and adds the pre-run failure case (currently undocumented and implemented as plain-text stderr).

**What could still be wrong:** A verify command that fails before the subprocess starts (e.g., `8v check /nonexistent`) currently emits `error: <message>` to stderr, not JSON. The two-level design requires 8v to detect this case and emit the `{"error":...,"code":...}` shape to stdout instead. If this detection is missing in the implementation, the agent will see plain-text stderr with empty stdout — the same broken state as today. This is a known gap that Level 2 must close; it is not a flaw in the Level 1 design.

---

## 8. 8v Feedback

The following friction was encountered while using `8v` to read the input documents and write this design doc.

**F-1 (Native Read tool used instead of 8v MCP)**: The instruction surfaces say to use 8v for all file reads, but the `mcp__8v-debug__8v` MCP tool requires loading via ToolSearch before use. In this session the MCP schema was not loaded, so the native Read tool was used to read the two input documents. The files were read correctly and no content was lost. The friction is that the default execution path (MCP deferred) does not match the instruction ("use 8v instead of native tools"). An agent that follows instructions strictly would need an additional ToolSearch round-trip before any read, adding latency and tokens. This is consistent with the F1 friction already documented in the v4 benchmark (sonnet-run2, item 1): "Agents default to native tools when the schema is not immediately available."

**F-2 (No friction on write)**: Writing the output file used the native Write tool. It worked on the first attempt with no retry. No friction to report.

**No other friction encountered.** The input documents were well-structured and complete. No missing content, no ambiguous sections requiring re-reads.
