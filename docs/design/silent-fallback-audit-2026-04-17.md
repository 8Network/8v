# Silent Fallback Audit — 2026-04-17

**Scope:** All `src/` across workspace crates. Read-only audit per Rule 4 (no silent fallbacks).

## Summary

| Crate | P0 | P1 | P2 | Total |
|---|---|---|---|---|
| o8v (CLI) | 4 | 2 | 1 | 7 |
| o8v-stacks | 5 | 4 | 2 | 11 |
| o8v-check | 1 | 0 | 0 | 1 |
| o8v-project | 1 | 0 | 0 | 1 |
| o8v-process | 0 | 1 | 0 | 1 |
| o8v-core | 0 | 1 | 0 | 1 |
| **Total** | **11** | **8** | **3** | **22** |

## Systemic Finding

**Every JSON-output linter parser silently returns `Vec::new()` on deserialization failure** — rubocop, ruff, eslint, biome, tflint, hadolint, shellcheck, oxlint. A linter version bump that changes output format silently reports "no diagnostics." This is the same failure class as the TypeScript jest-runner bug fixed earlier today: *silent empty on malformed input*. It is the most dangerous class of bug a checking tool can have because it produces false-clean results.

## Top 10 by user-impact × likelihood

1. **JSON parsers — empty on deserialization failure** (8 files in o8v-stacks/parse/)
2. **Detector loop — `Err(_) => continue`** (o8v-stacks/detect.rs:105, o8v-project/lib.rs:220)
3. **Git hooks config load — `Err(_) => Config::default()`** (o8v/hooks/git.rs:170)
4. **Check parse failure — empty HashSet** (o8v/commands/check.rs:172)
5. **Dispatch — `if let Ok(cwd)` no else** (o8v/dispatch.rs:35-46)
6. **TypeScript resolution — empty return** (o8v-stacks/stacks/typescript.rs:29,41)
7. **Line parsers dropping diagnostics** (javac.rs, deno.rs, rebar_*.rs)
8. **ls I/O fallbacks** (o8v/commands/ls.rs:167,173,196)
9. **Check orchestration error swallow** (o8v-check/lib.rs:155)
10. **Cargo unknown severity → empty string** (o8v-stacks/parse/cargo.rs:60)

## Test Gap Pattern

Not a single finding has a test that exercises the failure path. The audit surfaced 22 silent-fallback sites and 22 corresponding test gaps. Per `feedback_tests_must_catch_bugs`: failing test on pre-fix code FIRST, then fix.

## Prescribed Approach

1. Pick ONE item (suggest #1 — the JSON parser pattern, since it's systemic).
2. Write a test that feeds malformed JSON, asserts the bug (silent empty) on current code.
3. Convert the `Err(_) => Vec::new()` to `Err(ParseError::MalformedOutput { tool, raw })`.
4. Verify test now expects the error.
5. Repeat pattern across all 8 JSON parsers with shared helper.
6. Move to #2.

Do not fix in parallel. One at a time, test-first, per Rule 2.

## Out of scope for this audit

- `o8v-stacks/src/resolve_tool.rs`, `stacks/rust.rs`, `o8v-core/src/project/*`, `o8v/src/commands/build.rs` — concurrent in-flight edits. Re-audit after that PR lands.
