# Slice B2 — decomposition

## Why this doc exists

B2 (uniform error routing) was introduced in register v2 as a single slice that closes 11 bugs across 8+ subcommands. That's the biggest-ROI change in Phase 0 — AND it violates the stabilization loop's narrow-slice discipline (step 4: "one behavioral change per design, one design per slice"). A slice that touches 8 commands is not a slice; it's a refactor.

This doc splits B2 into four ship-independently sub-slices. Each closes a subset of the 11 bugs, each has its own failing-first tests, each has its own mutation audit. Each can be reviewed and shipped on its own cycle.

## The overarching contract

Already specified: `docs/design/error-contract.md` (with CE-2/CE-3 resolutions). B2a–B2d implement pieces of it. They do not redefine it.

## B2a — STDERR channel discipline

**What**: Every human-readable error message emits to stderr. No command prints errors to stdout. Pass-through commands (`check`, `test`, `build`) keep subprocess stderr → process stderr; subprocess stdout → process stdout. 8v's own wrapper errors emit to stderr regardless.

**Bugs closed**: BR-08 (search errors on stdout), part of BR-18 (JSON envelope comes in B2b), BR-05 (exit-code+channel together disambiguate).

**Failing-first tests** (each must fail on current binary before any code change):
- `b2a_write_error_emits_to_stderr_not_stdout`
- `b2a_search_error_emits_to_stderr_not_stdout`
- `b2a_read_error_emits_to_stderr_not_stdout`
- `b2a_check_wrapper_error_emits_to_stderr_not_stdout`
- `b2a_build_wrapper_error_emits_to_stderr_not_stdout`
- `b2a_passthrough_subprocess_stdout_not_mixed_with_errors`

**Scope**: touches all 10 subcommands' error-emit sites. Render-only. No shape changes to existing messages.

**Why narrow**: one decision ("errors to stderr") applied uniformly. Review is mechanical.

## B2b — JSON error envelope

**What**: When `--json` is set, errors emit as `{"error":"...","code":"...","path?":"...","line?":N}` to **stdout** (per error-contract §2.3), with stderr empty. Pass-through commands use the two-level schema per CE-3: `{"error","code"}` = 8v pre-run failure, `{"exit_code","tool":...,"output":...,"duration_ms":...}` = subprocess ran.

**Bugs closed**: BR-18 (JSON errors nonexistent today), parts of BR-04, BR-29, BR-33 (various per-command JSON shape gaps).

**Depends on**: B2a (stderr-empty-when-json requires stderr discipline in place).

**Failing-first tests** (each must fail on current binary before any code change):
- `b2b_write_json_error_envelope_shape_on_failure`
- `b2b_search_json_error_envelope_shape_on_failure`
- `b2b_read_json_error_envelope_shape_on_failure`
- `b2b_check_json_two_level_schema_on_subprocess_failure`
- `b2b_build_json_two_level_schema_on_subprocess_failure`
- `b2b_json_mode_stderr_empty_on_error`

**Why narrow**: one shape decision applied across commands. Shape is already specified at Level 1.

## B2c — Exit code unification

**What**: Every subcommand follows the 0/1/2 convention — 0 success, 1 runtime error (bad arg, no match, subprocess nonzero, tool missing, project detection failure), 2 clap invocation error only (malformed flag, unknown subcommand — clap's own exit path). Exit 2 is NOT for runtime conditions. Search takes the additional CE-2 convention: exit 1 + stderr empty = clean no-match; exit 1 + stderr non-empty = partial I/O failure.

**Bugs closed**: BR-05 (exit 1 overloaded), BR-19, U-2 (`upgrade` exits 0 on network failure), part of BR-02 (build --json exits 0 on failure).

**Depends on**: B2a (CE-2 discriminant requires stderr to be reliably empty on clean no-match).

**Failing-first tests** (each must fail on current binary before any code change):
- `b2c_upgrade_exits_nonzero_on_network_failure`
- `b2c_build_exits_nonzero_on_subprocess_failure`
- `b2c_search_exits_1_not_2_on_runtime_error`
- `b2c_clap_invocation_error_exits_2_not_1`
- `b2c_tool_missing_exits_1_not_2`
- `b2c_project_detection_failure_exits_1_not_2`

**Why narrow**: decisions already made in Level 1; this enforces them.

## B2d — Prefix unification

**What**: Every stderr error line starts with `error: <verb>: <subject>` where `<verb>` is the command (e.g. `error: write: file does not exist: ...`). Eliminates the six patterns measured today (`error: 8v:`, `error: Error:`, `error: error:`, `error:`, no prefix, etc.).

**Bugs closed**: parts of BR-17 (format drift), AF-3 (`error: Error:` in write), AF-5 (three-format cross-command divergence). Includes the capital-E follow-up cleanly — that slice is absorbed into this one.

**Depends on**: B2a (emits are all routed to stderr before we touch the format).

**Failing-first tests** (each must fail on current binary before any code change):
- `b2d_write_stderr_matches_error_verb_subject_format`
- `b2d_search_stderr_matches_error_verb_subject_format`
- `b2d_read_stderr_matches_error_verb_subject_format`
- `b2d_no_double_prefix_error_error_in_any_command`
- `b2d_no_capital_e_error_prefix_in_any_command`

**Why narrow**: one format decision. Applied as a final pass. Absorbs the capital-E slice — drop the separate follow-up design once B2d is approved.

## Recommended order

1. **B2a first** — lowest risk; every subsequent slice depends on stderr discipline. In particular, B2a is a prerequisite for B2c: the CE-2 discriminant ('exit 1 + stderr empty = clean no-match') requires stderr to be reliably empty on a clean no-match. Without B2a, B2c's exit-code rules cannot be correctly enforced.
2. **B2c second** — purely mechanical exit-code audit. No shape changes. Easy to review.
3. **B2b third** — introduces JSON shape. Some commands may need serde support added. Biggest individual slice.
4. **B2d last** — format sweep. Absorbs capital-E follow-up. Touches exact strings; highest test-churn.

Each must pass counterexample review (step 3 of the loop) + mutation audit (step 5.5) before ship.

## What this decomposition does NOT cover

- Subprocess-structured output preservation for `check`/`test`/`build` (covered in CE-3 resolution; B2b applies it).
- Localization (out of scope for Phase 0).
- Colored output (no decision here).
- Any behavior change beyond channel/shape/code/format. Message text content stays mostly intact; verbs and subjects get normalized.

## Gate

B2a through B2d each get their own Level 1 review + CE round before implementation. Founder picks the order (recommended above is a suggestion, not a commitment).

## Register reconciliation

After this decomposition:
- B2 is no longer a single slice; its row in register v2 §4 gets replaced by 4 rows.
- The capital-E follow-up design (`write-capital-e-prefix-superseded.md`, now in findings/) is absorbed into B2d. B2d is approved (2026-04-20). The follow-up doc is superseded; no A/B/C pick is needed.
