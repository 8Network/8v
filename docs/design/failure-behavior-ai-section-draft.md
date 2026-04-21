> **Ship timing (Decision E):** This draft ships AFTER B2a (error-routing/stderr) lands. Do not merge before B2a is green.

# Slice B1 — Failure behavior draft (ai_section.txt / CLAUDE.md surface)

> **Today** (pre-B2): The binary does NOT implement the full contract described below. Key differences: (1) `--json` only emits structured errors for batch `read` — all other commands emit plain-text stderr regardless of `--json`; (2) `8v init` emits `init: failed` to stdout and the reason to stderr (BUG-4 / INCONSISTENCY-8) — stdout is NOT clean on init failure. Everything below describes the post-B2 target state.
> **After B2 ships**: the contract below is fully in effect.

> **CT-1 resolved (2026-04-20):** Canonical subprocess-capture shape: `{"exit_code":...,"tool":"...","output":"...","duration_ms":...}`. All three authoritative docs now use this shape.

**Target surface:** `o8v/src/init/ai_section.txt` (injected into CLAUDE.md on `8v init`)
**Bugs addressed:** BR-21 (error/exit-code contract absent from ai_section.txt), BR-18 (partial: documents post-B2 target and explicitly flags current gap)

## Rationale

BR-21 (Axis 3 = 2.33/10 across six runs): agents consuming CLAUDE.md have no contract for what non-zero exits mean or how to parse `--json` failures. Without this, every `exit 1` is treated identically — agents either retry blindly or abort when they should inspect. BR-18 creates a tension: today's binary does not fully implement the post-B2 contract. This draft documents the target state and explicitly flags the gap so the founder can decide whether to ship this prose before B2 code lands. Closing the doc gap first is zero-risk and gives agents a contract to validate against when B2 ships.

**Token estimate (section only):** ~270 tokens. The `ai_section.txt` surface uses tutorial/directive voice with bullet lists; agents scan bullets faster than dense prose. This estimate includes three short example blocks.

---

## Draft section (≤35 lines, directive tutorial voice)

```
## Failure behavior

**Exit codes — memorize three values:**
- `0` — success; stdout has the result
- `1` — runtime error: bad path, I/O failure, no match, tool error
- `2` — invocation error: bad flags or args (fix the command, not the target)

**Stderr carries all errors. Stdout is always clean on failure.**
Exception: `8v read` batch mode — per-entry errors appear inline in stdout under `=== label ===` headers, not on stderr.

**`--json` error envelope (post-B2 contract):**
```json
{"error":"file not found","code":"NOT_FOUND","path":"src/main.rs"}
```
`path` and `line` are present only when the error has a location. On `--json`, stderr is empty.

**`8v search` — do not treat "no match" as an error:**
- `exit 0` — matches found (stderr may carry warnings for unreadable files — normal)
- `exit 1` + empty stderr — no match found; stop retrying, adjust the pattern
- `exit 1` + non-empty stderr — partial I/O failure; some files were unreadable

**`8v check` / `test` / `build` / `fmt` — two-level `--json` schema:**
Discriminate by top-level key:
- `"error"` key → 8v failed before running the subprocess (wrong path, bad config)
- `"exit_code"` key → subprocess ran; inspect `output` for the tool's own diagnostics

**Retry rules:**
- `exit 2`: fix the invocation, then retry
- `exit 1`: read stderr and act on the cause — do not retry the same command unchanged
- Never retry `search exit 1` with empty stderr — it is a definitive no-match signal
```

---

## Counterexamples

Five cases where this prose could mislead an agent, with remediation notes.

**CE-1: Agent retries `search exit 1 + empty stderr` assuming a transient error.**
The note says "stop retrying, adjust the pattern" but agents in retry loops may not distinguish this from a transient `exit 1` from `check`. Pattern: any `exit 1` triggers retry logic.
_How to address:_ Add a concrete example: `8v search "TODO" . → exit 1, stderr empty → no TODOs found, done.` The example makes the stop condition concrete.

**CE-2: Agent passes `--json` to `write` today and tries to parse stderr as JSON.**
The NOTE is present but could be read as "only batch read emits JSON" without grasping that ALL other commands emit plain-text. An agent running pre-B2 binary on `write --json` will get plain-text stderr and either crash its JSON parser or discard the error.
_How to address:_ Add a one-line mechanical rule: "Pre-B2: if command is not `read` (batch), treat all error output as plain text, even with `--json`."

**CE-3: Agent reads `"exit_code": 0` from `check --json` and concludes success.**
Two-level schema: `exit_code: 0` means the subprocess exited cleanly, but 8v itself may still be reporting a pre-run failure via the `"error"` key. An agent checking `exit_code` first could miss the outer `"error"`.
_How to address:_ State check order explicitly: "Check for `"error"` key first. Only inspect `"exit_code"` if `"error"` is absent."

**CE-4: Agent uses `init` and checks only stderr for failure.**
`init` currently writes `init: failed` to stdout and the reason to stderr (INCONSISTENCY-8 in the measurement doc). An agent checking only stderr for init failures will see a reason but miss the stdout failure marker, or vice versa.
_How to address:_ Add: "Exception: `init` — check both stdout and stderr for failure signals (pre-B2 inconsistency)."

**CE-5: Agent infers `exit 1 + non-empty stderr` from `search` always means "retry."**
The prose says this is a partial I/O failure. An agent may interpret "partial failure" as "retry the whole search" rather than "some files were unreadable; the results you got are valid but incomplete."
_How to address:_ Clarify: "Partial I/O failure: matches already returned are valid. Only the unreadable files were skipped. Inspect stderr to see which files were skipped — do not discard results."

---

## 8v feedback

Every friction point with exact command and expected vs. actual behavior.

1. **`8v read` on `.txt` files returns useless symbol map.**
   Command: `8v read /Users/soheilalizadeh/8/products/vast/oss/8v/o8v/src/init/ai_section.txt`
   Expected: file content (it is prose with no symbols).
   Actual: empty symbol map, prompt to use `--full`.
   Friction: mandatory two-call pattern for every prose file — discovery call reveals nothing, then `--full` call gets the content. Wasted round-trip on every `.txt`, `.md`, and similar file.

2. **No `8v write` path for net-new files.**
   To create a new file using 8v idioms, `8v write <path> --append "<content>"` requires the file to already exist. For the two draft files in this task, native `Write` tool was needed instead.
   Friction: breaks the "use 8v for all file operations" principle for file creation. `8v write <path> --create "<content>"` or treating `--append` on a non-existent path as create would close this.

3. **`8v read` batch `--full` is all-or-nothing.**
   Command: `8v read file.txt src/main.rs --full`
   `--full` applies to every positional arg. For a mixed batch (prose + code), it is impossible to get symbols for `.rs` and full text for `.txt` in a single call.
   Friction: forces two separate calls when reading heterogeneous file sets.
