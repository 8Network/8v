> **Ship timing (Decision E):** This draft ships AFTER B2a (error-routing/stderr) lands. Do not merge before B2a is green.

# Slice B1 — Failure behavior draft (MCP description surface)

> **Today** (pre-B2): The binary does NOT implement the full contract described below. Key differences: (1) `--json` only emits structured errors for batch `read` — all other commands emit plain-text stderr regardless of `--json`; (2) `8v init` emits `init: failed` to stdout and the reason to stderr (BUG-4 / INCONSISTENCY-8) — stdout is NOT clean on init failure. Everything below describes the post-B2 target state.
> **After B2 ships**: the contract below is fully in effect.

> **CT-1 resolved (2026-04-20):** Canonical subprocess-capture shape: `{"exit_code":...,"tool":"...","output":"...","duration_ms":...}`. All three authoritative docs now use this shape.

**Target surface:** `o8v/src/mcp/instructions.txt`
**Bugs addressed:** BR-21 (error/exit-code contract absent from MCP description), BR-18 (partial: documents post-B2 target and explicitly flags current gap)

## Rationale

BR-21 (Axis 3 = 2.33/10 across six runs): agents cannot construct correct error-handling loops because exit-code semantics and `--json` error shape are undocumented on the MCP surface. Agents retry on every non-zero exit or treat `exit 1` as a hard stop, both of which are wrong for `search` (harvest) and `verify` (two-level schema). BR-18 documents the gap between today's binary and the post-B2 contract. Closing BR-21 first (docs) costs zero risk; it also gives agents a contract to test against when B2 ships.

**Token estimate (section only):** ~220 tokens. The MCP description is reference-manual voice; every new token displaces headroom for other commands. This section is trimmed to the minimum an agent needs to write a correct retry loop.

---

## Draft section (≤35 lines, reference-manual voice)

```
## Failure behavior

Exit codes — three values only:
  0   success; stdout has the result
  1   runtime error (bad path, parse failure, I/O, no match)
  2   invocation error (bad flags/args, clap parse failure)

Stderr carries all error text. Stdout is always clean on failure (no mixed output).
Exception: batch `read` per-entry errors appear inline under `=== label ===` headers.

`--json` error envelope (post-B2):
  {"error":"<message>","code":"<key>","path":"<path>","line":<N>}
  `path` and `line` are present only when the error has a location.
  On `--json`, stderr is empty; the envelope is on stdout.

`search` special case — harvest semantics:
  exit 0   = matches found (stderr may carry I/O warnings for unreadable files)
  exit 1 + stderr empty     = genuine no match (correct termination, stop retrying)
  exit 1 + stderr non-empty = partial I/O failure (some files unreadable)

`check`/`test`/`build`/`fmt` — two-level `--json` schema:
  Top-level key "error"     → 8v failed before running the subprocess
  Top-level key "exit_code" → subprocess ran; value is the tool's own exit code
  Discriminate on the key, not the value.

Retry guidance: retry on exit 2 only after correcting the invocation.
  exit 1 is data (wrong path, no match, tool failure) — inspect stderr, do not blindly retry.
```

---

## Counterexamples

Five cases where this prose could mislead an agent, with remediation notes.

**CE-1: Agent assumes `--json` works today.**
Draft says "post-B2" in a NOTE but the NOTE is easy to miss in dense tool context. An agent running today's binary passes `--json` to `write`, gets plain-text stderr, and fails to parse it.
_How to address:_ Move the NOTE to the top of the `--json` block, not the bottom. Make it the first line: "NOTE (pre-B2): ..."

**CE-2: Agent treats `exit 1 + stderr empty` from `search` as an I/O error.**
The prose says "genuine no match (correct termination, stop retrying)" but BUG-3 is still live: `search` swallows permission-denied and exits 1 with empty stderr. The current binary cannot distinguish "no match" from "silently failed perm check."
_How to address:_ Add a line: "BUG-3 (pre-B2): permission-denied during search silently exits 1 with empty stderr — indistinguishable from no-match. Trust only in controlled directories."

**CE-3: Agent discriminates `--json` verify schema on value, not key.**
Prose says "discriminate on the key, not the value." An agent could read `exit_code: 1` and infer "subprocess error" without realizing `exit_code: 0` is also a subprocess success and `error` key is the 8v-level failure signal.
_How to address:_ Add a concrete example pair — one `"error":...` and one `"exit_code":...` — to make the discriminant pattern explicit.

**CE-4: Agent assumes stderr is always the error source.**
The `init` command currently splits output: `init: failed` goes to stdout, reason to stderr (INCONSISTENCY-8). An agent checking only stderr for `init` failure will see an incomplete picture.
_How to address:_ Add a note in the exception list: "Exception: `init` — failure summary on stdout, reason on stderr (pre-B2 inconsistency)."

**CE-5: Agent infers that batch `read` errors on individual entries are exit 1.**
The prose says per-entry errors appear inline under `=== label ===` headers. An agent could infer that any batch `read` with a bad file exits 1. In practice the overall exit may be 0 if at least one entry succeeded (harvest behavior). This is ambiguous in the current design.
_How to address:_ State explicitly: "Batch `read` exits 0 if at least one entry succeeds; failed entries carry inline error text. Exit 1 = all entries failed."

---

## 8v feedback

Every friction point with exact command and expected vs. actual behavior.

1. **`8v read` on a `.txt` file returns a symbol map with 0 symbols, no content.**
   Command: `8v read /Users/soheilalizadeh/8/products/vast/oss/8v/o8v/src/mcp/instructions.txt`
   Expected: full text (it has no symbols — it IS prose).
   Actual: "0 symbols found. Use --full to read the full file."
   Friction: required a second call every time. `--full` is the only useful mode for `.txt` files; the tool should hint that default is useless for prose files, or auto-detect.

2. **`8v read` batch requires knowing which files need `--full` up front.**
   Command: `8v read file1.txt file2.rs --full`
   `--full` applies to ALL positional args. For mixed batches (prose + code), there is no per-file flag.
   Friction: forced separate calls for `.txt` vs `.rs` files.

3. **No friction writing the draft files.** `Write` tool worked without 8v for new-file creation (the `8v write` interface requires knowing the target line number for new files, which requires `wc -l` on a non-existent file).
   Friction: `8v write <new-path> --append` is the workaround for new files, but this requires the file to exist. For net-new files, native `Write` tool is needed or `8v write` needs a `--create` mode.
