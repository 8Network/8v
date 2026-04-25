# Foundation Audit & Backfill Plan

**Status:** active, 2026-04-25.  
**Trigger:** rounds 11-14 QA found 13 user-visible bugs in the installed binary that the 2,605-test suite did not catch. Audit (`/tmp/test-audit.md`) showed the deception is structural: 14 files in non-`o8v` crates named `e2e_*.rs` / `security_*.rs` test library APIs in isolation, never the binary. Bugs lived at boundaries between layers; tests are organized within layers.

**Public claim withdrawn** — README benchmark numbers pulled. No releases until this plan completes.

## Principle

Every claim must be defended at the binary boundary. A test that proves `SafeFs` rejects symlinks does not prove `8v ls --tree` rejects symlinks — those are different code paths. We test the contract the user/agent invokes, not the function the developer wrote.

## Phases

### Phase 1 — Stop the deception (mechanical, ~1 day)

Rename or relocate every misnamed file so naming reflects reality.

- Files in `o8v-fs/`, `o8v-stacks/`, `o8v-process/`, `o8v-core/`, `o8v-testkit/` named `e2e_*.rs` that do not spawn the binary → rename to `unit_*.rs` or `lib_*.rs`.
- Files in `o8v/tests/` that import library internals (e.g. `counterexamples_hook_redaction.rs`) → move to the owning crate's `src/.../mod.rs` as a `#[cfg(test)] mod tests` block, OR rename with `unit_` prefix.
- After: `e2e_*.rs` is a load-bearing convention — if the file name says e2e, the file spawns the binary.

Acceptance: `grep -rL "CARGO_BIN_EXE_8v" $(find . -name "e2e_*.rs")` returns empty.

### Phase 2 — Backfill thin/missing binary contracts (~3-4 days)

Highest-risk surfaces, in order:

1. **`upgrade`** — distribution path. Add: bad URL → exit 1 + stderr; network down → graceful; corrupt download → reject; already-current → exit 0 no-op; --json on each.
2. **Binary-level path containment** — `8v read /etc/passwd`, `8v read ../../etc/passwd`, `8v write /tmp/outside`, `8v search foo /etc` — all must error at the CLI layer with a stable message. `SafeFs` tests don't substitute.
3. **`fmt`** — read-only file, unsupported stack, json output, fmt-then-check round-trip.
4. **`build` / `test` on missing toolchain** — assert structured diagnostic, not panic.
5. **`hooks claude pre-tool-use`** — full input matrix at the binary layer (already partial; finish).

Each surface gets one new `tests/contract_<command>.rs` file. Each test asserts: exit code, stdout content (or emptiness), stderr content (or emptiness), files-on-disk side effects. No happy-path-only tests.

### Phase 3 — Convert QA findings 11-14 into binary contracts (~1 day)

Cross-check every Round 11-14 bug. Each must have a regression test at the binary boundary, not just at the function fixed.

Currently: most regression tests added during fixes spawn the binary, but a few (e.g. some search internals) only assert at the function level. Fill those gaps.

### Phase 4 — Cross-layer contract tests (~2 days)

For each shared invariant that crosses layers, add a binary-boundary test that exercises it through the actual command path. Examples:
- "8v never follows symlinks during walks" → test `init`, `ls --tree`, `search`, `check`, `build`, `fmt` each against a symlink loop fixture. (Today only `ls --tree` regression test exists post-fix.)
- "8v never writes outside the project root" → test `write`, `init`, `fmt`, hooks all against an absolute outside path.
- "Every command's --json output validates against a documented schema" → if no schema exists, write one, then test.

### Phase 5 — Re-baseline benchmarks (~1 day)

Only after Phases 1-4. Re-run the cross-agent benchmarks against the post-audit binary. Compare to historical Apr 24 numbers. Document the methodology, fixtures, and per-task variance. Re-publish numbers in README only if reproducible.

## Out of scope (for this audit)

- New features. Feature freeze remains in effect.
- Re-architecting crates. Renames and additions only.
- Rewriting unit tests that are honest (a real unit test in `src/` is fine).

## Status (2026-04-25 evening)

- **Phase 1** ✓ done. 14 misnamed test files renamed. `e2e_*.rs` is now load-bearing — convention enforced by acceptance grep. Commit `cdf...` (`test: rename misnamed test files`).
- **Phase 2** ✓ done. 5 new contract files at the binary boundary, 51 active tests + 9 ignored with FIXME tags pointing at gaps to fix in Phase 3+:
  - `contract_upgrade.rs` — 7 tests (bad URL, bogus DNS, 404, corrupt checksums, already-current, --json variants). Added one-line test affordance `8V_RELEASE_BASE_URL` env override in `upgrade.rs`.
  - `contract_path_containment.rs` — 8 active + 3 ignored. Locks read/write containment; surfaces that `search`, `ls`, `fmt` do NOT enforce containment (FIXME phase-2c — needs founder policy decision: is `8v search foo /etc` a valid use case?).
  - `contract_fmt.rs` — 11 active. Locks fmt behavior on file path, nonexistent path, empty/no-stack dir, readonly file, idempotency, fmt-then-check round-trip, invalid flag, --json shape, syntax-error file.
  - `contract_build_test_missing_tool.rs` — 5 active + 6 ignored. JSON envelope on missing tool; --timeout plumbing; missing-tool stderr does not name the tool by name (6 FIXMEs surface real gaps).
  - `contract_hooks.rs` — 20 active, 0 ignored. Full input matrix incl. malformed JSON, empty stdin, null tool_name, deeply nested, 10MB payload, non-UTF8. No hangs found.
- **Phase 3** ✓ done as audit. All 9 bugs from rounds 12-14 have BINARY_CONTRACT regression tests at the binary boundary. Phase 3 audit incorrectly flagged bug 7 (init non-TTY) as missing — it's covered by `init_without_tty_prints_error` at `bin_e2e.rs:311`. One cosmetic finding: bug 5's canonical regression-test annotation points at `append_to_file` which would not catch the original bug; the real guard is `append_to_lf_file_without_trailing_newline_uses_lf` at line 919. Cross-reference, not a coverage gap.
- **Phase 4** not started. Cross-layer invariants (no walker follows symlinks anywhere; no command writes outside project root anywhere; --json schema for every command) need writing.
- **Phase 5** not started. Re-baseline benchmarks.

## Open decisions for founder

1. **Containment policy for `search`/`ls`/`fmt`** (FIXMEs in `contract_path_containment.rs`). Current behavior: these accept any explicit path argument, even outside the project. Plausible intent: `8v search foo /etc` is a valid use case. But `8v fmt /etc` would mutate files outside the project — that's likely a bug. Need policy:
   - read/write/init/check/build/test → enforce containment (current behavior).
   - search/ls → allow any explicit path (lock current behavior with explicit policy comment).
   - fmt → enforce containment (BUG — fix in Phase 2c2).
2. **Phase 4 scope.** Walking-symlinks audit is straightforward. Writing-outside-root audit is straightforward. --json schema-per-command is a much bigger lift — needs a spec doc first.
3. **Phase 5 timing.** Benchmarks should re-baseline only after Phase 4. Need agreement on which fixtures (the historical fix-go N=6, plus what?) and whether to publicly republish numbers or hold them internal until enough rounds have stabilized.

## Tracking

This doc is the source of truth. Updated as phases complete. No release until Phase 5 lands.
