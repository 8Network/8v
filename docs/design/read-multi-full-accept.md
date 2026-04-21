# Design: Accept repeated `--full` flags on `8v read`

## Scope
Single change: make `8v read --full --full <path>` behave identically to `8v read --full <path>`.
Delimiter format, partial failure, and `:range`+`--full` interaction are deferred.

## Problem
Clap's default `ArgAction::SetTrue` rejects a flag passed twice with a hard error:
`"the argument '--full' cannot be used multiple times"`.
Agents writing one `--full` per input path (inferring per-argument semantics from `:range` syntax) hit
this error unconditionally. 6/6 v3 benchmark runs failed at this call site.

## Current behavior
`o8v/src/commands/read.rs:23-25`:
```rust
/// Show full file content instead of symbols
#[arg(long)]
pub full: bool,
```
No `ArgAction` override, no `overrides_with_self`. Clap defaults to `SetTrue`, which disallows repeats.

## Proposed behavior
`8v read --full --full <path>` succeeds; subsequent `--full` occurrences are silent no-ops.
Behavior is otherwise identical to `8v read --full <path>`.

## Implementation approach
Add `.overrides_with_self("full")` to the existing `#[arg(long)]` annotation:

```rust
#[arg(long, overrides_with_self = "full")]
pub full: bool,
```

This is annotation-only. `pub full: bool` type is unchanged; all callers (`read_one` calls at
lines 203 and 226) require no modification. `ArgAction::Count` is NOT used — it requires changing
the field type to `u8` and updating all callers, making it a wider diff for no additional benefit.

## Test plan
Both tests live in `o8v/tests/e2e_cli.rs`. Both must **fail** on pre-fix code with the clap error
before passing after the fix.

1. `read_double_full_flag_accepted` — invoke `8v read --full --full <valid-fixture-file>`, assert
   exit code 0 and stdout contains file content (not the clap error).

2. `read_triple_full_flag_accepted` — invoke `8v read --full --full --full <valid-fixture-file>`,
   assert exit code 0 and stdout identical to single-`--full` output for the same file.

## Acceptance criteria
1. `8v read --full --full <path>` exits 0 and produces the same output as `8v read --full <path>`.
2. `8v read --full` (single) behavior is unchanged.
3. `pub full: bool` field type is unchanged; no caller diffs outside the annotation line.
4. Both tests in `o8v/tests/e2e_cli.rs` are red on pre-fix HEAD and green after the one-line fix.

## Non-risks
`overrides_with_self` is a stable clap 4 API. It does not affect how clap serializes the value —
`full` remains `true` after any number of `--full` occurrences. The change is contained to one
annotation; no logic, no render path, no storage format is touched.

## Review gate
Zero blockers from one adversarial review round before any code is written.
