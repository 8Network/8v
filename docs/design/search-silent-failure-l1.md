# Slice B3 — search silent-failure (Level 1)

## Why this slice exists

`8v search` silently loses information that the agent needs. Four bugs in `docs/findings/command-qa-search-2026-04-20.md` share the same failure mode: the agent believes "no matches found" when the reality is "couldn't read some files" or "zero was passed for a limit". Pattern family P-A (Silent success when should be failure) per register v2.

Bugs in scope:
- **BR-03**: chmod-000 files are silently skipped. Exit 1, "no matches found", stderr empty. Indistinguishable from genuine no-match.
- **BR-06**: binary files (NUL bytes) silently skipped without incrementing the visible `files_skipped` counter.
- **BR-07**: `--limit 0` is accepted silently and returns "no matches found". Agents cannot distinguish "limit exhausted" from "zero matches".
- **BR-23**: single-file search returns empty path in both text and JSON output. When the input resolves to exactly one file and it has matches, the matched line appears without an identifier.

## Scope

- Every filesystem read error during traversal emits a per-file `error: search: <reason>: <path>` to stderr. Harvest-and-warn, per the CE-2 resolution in the error-contract Level 1.
- Every silently-skipped binary/non-UTF-8 file is added to the `files_skipped` counter AND to a new `files_skipped_by_reason` map in `--json`, keyed by `permission_denied` / `binary` / `not_utf8`. This field is additive: existing consumers that do not read it are unaffected. Consumers MUST ignore unknown fields to remain forward-compatible. No current consumers of `files_skipped_by_reason` exist today (confirmed 2026-04-20 survey).
- `--limit 0` is rejected at parse time via clap's native value validator (`value_parser = clap::value_parser!(u32).range(1..)` on the `limit` field in `SearchArgs`). Because clap rejects the value before the command runs, exit 2 is correct per error-contract §2.1. The rejection emits a one-line hint ("--limit must be ≥ 1").
- Single-file search emits the input path on every match line, same as multi-file. Both text and `--json` output paths populated.

Out of scope:
- Changing ripgrep's context-direction labels (`>`/`<`) — separate issue, not a silent failure.
- Adding `--count`, `--max-per-file`, or any new limit flags.
- Behavior on symlinks (separate concern).
- Error routing for commands other than search (B2 territory).

## What changes

- Traversal code emits per-file errors to stderr when `std::fs` returns an error. Already follows harvest behavior per `files_skipped` existence; needs stderr warning.
- Exit code split per CE-2 resolution of error-contract:
  - `exit 0` = ≥1 match, no I/O errors
  - `exit 1 + stderr empty` = 0 matches, no I/O errors
  - `exit 1 + stderr non-empty` = partial I/O failure (0 or ≥1 matches)
- `--limit 0` added to clap validator.

## Why each change

- BR-03: measurement confirms permission errors never reach stderr. Adding per-file stderr lines is the minimum change that makes partial failures observable.
- BR-06: `files_skipped` exists but is neither populated for binary files nor broken out by reason. The register v2 cross-check shows agents treat `files_skipped > 0` as "something's up" — right now they never get the signal.
- BR-07: `--limit 0` returning success is an exit-code lie. Reject at parse.
- BR-23: fixing the path field is a one-render-site change; same rendering path used by multi-file search needs to be the default.

## Counterexamples

1. **A repo with a symlink cycle causing a chain of errors.** Output floods stderr. Mitigate by deduplicating identical error messages per-path during a single invocation (Level 2 detail).
2. **`files_skipped_by_reason` inflates JSON size on large scans.** Cap the map size or use a small reason-code enum. Level 2.
3. **A file that is both binary and unreadable (permission-denied on a PNG).** Which reason wins? Permission-denied — it's the first error encountered.
4. **CE-2 discriminant fragility.** The whole contract's "exit 1 + stderr empty = clean no-match" breaks if ANY future code path writes to stderr on a clean no-match. Add a regression test that pipes stderr to a check on every `exit 1 + 0 matches` path.
5. **Existing tests assert on current behavior.** Expect to update tests that assume `exit 1` = "no match" without checking stderr.

## Failing-first acceptance tests

- `search_emits_stderr_warning_on_permission_denied`
- `search_stderr_empty_on_clean_no_match`
- `search_files_skipped_by_reason_populated_for_binary`
- `search_files_skipped_by_reason_populated_for_permission_denied`
- `search_limit_zero_exits_2_with_hint`
- `search_single_file_emits_path_on_every_match`

Each must fail on current binary before any code change.

## Mutation audit plan (step 5.5 of the loop)

- M1: revert the stderr emit path for permission-denied — `search_emits_stderr_warning_on_permission_denied` must fail.
- M2: set `files_skipped_by_reason` always to `{}` — two tests must fail.
- M3: make `--limit 0` accepted (return Ok) — test must fail.
- M4: emit match line without path — `search_single_file_emits_path_on_every_match` must fail.
- M5: write to stderr on clean no-match — `search_stderr_empty_on_clean_no_match` must fail. This is the CE-4 discriminant regression test.

If any mutation fails to break a test, the test is happy-path theater and needs widening.

## Gate

No implementation until founder reviews Level 1 + CE round returns empty. Level 2 (implementation design) is a separate doc specifying the error-reason enum, dedup strategy, and exact render-site edits.

## Out of scope (explicitly)

- Context labels `>`/`<` inversion.
- Regex-vs-literal heuristic.
- Binary detection algorithm change.
- `--files` footer format cleanup.
- Compact mode toggle.
- BR-39 (`8v search .` treats `.` as regex not path) — separate behavioral/parsing change; tracked for Phase 0 round 2.
