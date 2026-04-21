# Slice C1 — init/hooks correctness (Level 1)

## Why this slice exists

The round 1 QA register names five bugs across `8v init` and `8v hooks` that share one failure mode: the command reports success while doing nothing or the wrong thing. Source: `docs/findings/command-qa-init-hooks-upgrade-mcp-2026-04-20.md` (I-1, I-2, I-3, H-1, H-3). H-1 is security-adjacent: `hooks claude pre-tool-use` exits 0 with "passed" on empty/malformed stdin, producing a false-allow signal when the gate is meant to be protective.

Falls under pattern family P-A "Silent success when should be failure" per register v2 §3.

## Scope

- `8v init` output truthfulness: do not claim hook installation when no files were created (I-1).
- `8v init` re-run message references correct filename (I-2, cosmetic).
- `hooks` input validation: empty or malformed stdin on `claude pre-tool-use` must not emit `passed`. Fail-closed (exit 1, "block") is the only valid outcome — exit 2 is reserved for clap invocation errors (§2.1 of the error contract).
- `hooks` Co-Authored-By stripping: broaden to cover other AI attribution patterns (H-3, bounded — see counterexamples).

Out of scope:
- C2 (`upgrade`) and C3 (`write` semantics).
- The broader error-contract (B1/B2/B3 own those).
- Any new commands or flags.

## What changes, in one sentence per bug

- **I-1**: `init` must detect the "no-git, no files created" path and emit failure (exit 1) with a message that names what was not created. Today it emits success.
- **I-2**: re-run skip message names the correct file (CLAUDE.md not AGENTS.md when CLAUDE.md was the one found).
- **I-3**: hooks call `8v` bare — add PATH validation (detect `8v` is resolvable before installing) or invoke via absolute path discovered at install time.
- **H-1**: `hooks claude pre-tool-use` must reject empty or malformed stdin. Fail-closed default: treat all invalid input (empty or malformed) as "block" (exit 1). Empty stdin is a runtime condition at the gate boundary, not a programming error; exit 2 is reserved exclusively for clap parse failures.
- **H-3**: widen Co-Authored-By stripping to cover: `Co-authored-by: Claude`, `Co-authored-by: AI`, `Generated-by:`, `AI-Assistant:`. Bounded — see counterexamples.

## Why each change

Each traces to one register row and a reproducing command recorded in `command-qa-init-hooks-upgrade-mcp-2026-04-20.md`. No speculative decisions.

## Counterexamples

1. **H-1 fail-closed breaks legitimate allow paths.** If an upstream client sends whitespace-only lines, do those become a block? Define: trim + empty string → exit 1 (block — fail-closed); non-empty but unparseable → exit 1 (block). Both are runtime conditions; neither maps to exit 2.
2. **H-3 over-zealous stripping.** A commit message that legitimately mentions Claude (e.g. "Fix a bug found while using Claude Code") should not be mangled. Strip must target Co-Authored-By footer pattern only, not inline mentions.
3. **I-1 non-git-dir false negative.** A user intentionally running `8v init` outside a repo to see what it would do. Message must be clear this is a "no-op, no git repo" outcome, not a crash.
4. **I-3 PATH validation race.** PATH at install time may differ from PATH at hook fire time. Prefer absolute path discovered at install.
5. **Localization.** Fixed English strings. Out of scope — noted for later.

## Failing-first acceptance tests (before implementation)

- `hooks_claude_pre_tool_use_empty_stdin_exits_1_not_0`
- `hooks_claude_pre_tool_use_malformed_json_exits_1_not_0`
- `init_non_git_dir_exits_1_with_explicit_message`
- `init_rerun_skip_message_names_actual_file_created`
- `hooks_coauthor_strips_generated_by_and_ai_assistant`
- `init_installed_hook_uses_absolute_8v_path_not_bare_name`

Each must fail on current binary (measured 2026-04-20) before any code change.

## Gate

No implementation starts until founder reviews this Level 1 design and counterexamples come back empty. Level 2 (implementation design) is a separate doc.

## Out of scope (explicitly)

- Renaming any command.
- Changing stdin protocol (e.g. switching JSON → TOML) — protocol stays.
- Adding new hook event types.
- Cross-shell portability (this affects only Claude Code invocations).
- BR-28 remains orphaned; no standalone slice proposed yet; park for Phase 0 round 2.
