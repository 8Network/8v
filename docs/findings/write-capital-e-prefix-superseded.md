> **SUPERSEDED (2026-04-20):** B2d is approved. This doc is absorbed into B2d. No A/B/C decision needed.

# Slice follow-up: close the remaining `error: Error:` case

> BLOCKED ON FOUNDER: B2d (prefix unification) offers an absorption path for this slice — if B2d is approved, this standalone doc is superseded and the A/B/C decision below is moot. Two choices: (A) pick A, B, or C here and ship this as a standalone fix before B2 work begins; (B) defer to B2d and mark this doc superseded when B2d is approved. Pick before any implementation starts.

## Why this doc exists

Commit `b15a677` closed the lowercase `error: error: …` case for `8v write`. Empirical tests on 2026-04-20 confirm:
- `write --append` on nonexistent file: `error: file does not exist …` (single prefix ✓)
- `write :1 --insert "x" --delete`: `error: Error: cannot combine …` (mismatched-case double prefix ✗)

The outer guard at `o8v/src/main.rs:90` is `msg.starts_with("error: ")` — case-sensitive. Several sites in `o8v/src/commands/write.rs` return messages starting with capital `Error:` (verified via `8v search "\"Error:"` — 4 sites at lines 193, 245, 250, and one other). These messages still get wrapped to `error: Error: …`.

The Level 1 error-contract (docs/design/error-contract.md) specifies one canonical prefix: `error: <verb>: <subject>`. The current state violates that contract.

## Scope fence

- Single slice. Fix the remaining capital-`Error:` double-prefix cases in `8v write` only. Do not touch `o8v/src/main.rs` outer guard. Do not touch other commands' prefixes yet — the broader slice B2 (uniform error routing) will unify them.
- No new flags. No new behavior. Render-only fix.

## Options

**Option A — Lowercase at source.** Change each capital `"Error: …"` in `o8v/src/commands/write.rs` to lowercase `"error: …"`. Guard then catches it.
- Pros: matches the contract literally; no special-casing in the guard.
- Cons: touches multiple lines in `write.rs`; if a future commit adds a new capital-`Error:` string it silently regresses.

**Option B — Case-insensitive guard.** Change the guard to `msg.to_ascii_lowercase().starts_with("error: ")` in `main.rs`.
- Pros: one-line change; robust to future inner messages in any case.
- Cons: allocates a new string per error (cheap); the inner capital `Error:` survives and the surface emits `error: Error: …` (still ugly, just single-prefixed-feeling).

**Option C — Strip at inner source.** Drop the `"Error: "` prefix from each site in `write.rs` entirely. Guard in `main.rs` adds the one canonical `error:`.
- Pros: cleanest; matches contract end-to-end; no case mismatch at all.
- Cons: touches the same inner sites as A; tests asserting on exact inner text would break.

## Recommendation

**Option C.** The Level 1 contract's point is that each emit site produces the *subject* of the error, and the router produces the prefix. Duplicating the prefix at the emit site was always the bug; lowercasing it (A) patches the symptom. Guard hardening (B) hides the problem rather than fixes it.

Estimated diff: ~5-8 lines across `write.rs`, plus any test that asserts on exact text. Failing-first test: `error: cannot combine --insert, --delete …` single prefix, no trailing `Error:`.

## Counterexamples

1. **Existing tests assert on `Error:`.** Search before editing: `8v search "Error:" o8v/tests`. Update asserts as part of the slice.
2. **`Err(format!("Error: …"))` from a non-`write` module.** Only `write.rs` in scope for this slice. Leave other modules alone; B2 handles them.
3. **An `anyhow::Error` wrapping an inner message with `Error:`.** The guard sees the top-level message. If the top wraps an inner `Error:`, we'd still emit a double. Check whether `write` uses `anyhow` or returns `String`. `o8v/src/commands/write.rs:193` appears to return `String` directly — safe.
4. **Localization.** 8v is English-only today. If translations ever land, the guard's prefix-match breaks entirely. Out of scope.
5. **Mutation audit gap.** M1: revert one `Error:` → `error:` change and confirm failing-first test catches it. M2: remove the prefix entirely at the source — test must still pass (message body unchanged after the `error: ` prefix). M3: capitalize a different message to `ERROR:` — assert output matches `error: ERROR: <msg>` (one prefix only); test fails if guard emits double prefix.

## Out of scope

- `read` (`error: 8v: …`), `search`/`ls` (raw OS error strings), `check`/`test`/`fmt`/`build` exit-code + prefix unification — all handled by slice B2.
- Localization, colored output, error codes on top of the prefix.

## Gate

If B2d is approved, this doc is superseded — no A/B/C pick needed. Otherwise, pick A, B, or C before implementation starts.
