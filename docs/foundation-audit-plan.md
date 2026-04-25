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

## Tracking

This doc is the source of truth. Updated as phases complete. No release until Phase 5 lands.
