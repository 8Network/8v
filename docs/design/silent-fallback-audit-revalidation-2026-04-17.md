# Silent-Fallback Audit Re-Validation — 2026-04-17

## Summary

Re-validation of the 21-finding grep audit from 2026-04-15.
3 true positives confirmed out of 21 findings (86% false-positive rate).

---

## Re-Validation Results

### F11 — deno.rs parse_at_line silent drop [HIGH] ✅ TRUE POSITIVE

**Claim:** `parse_at_line` returns `None` for remote URLs. Caller discards the diagnostic
silently and returns `ParseStatus::Parsed`. User sees zero errors, ships broken code.

**Confirmed by:** Writing a failing test (`remote_url_location_drops_diagnostic_reports_unparsed`)
against the pre-fix code — assertion `Unparsed` fails, actual is `Parsed`. Fix applied:
`any_dropped` bool, `ParseStatus::Unparsed` when true. Test now green.

**Impact:** User runs `deno check`, gets exit 0, zero diagnostics. File has a type error in
a remote-module re-export. Error is real but invisible.

---

### F3 — dispatch.rs unknown-tool silent skip [MEDIUM] ✅ TRUE POSITIVE

**Claim:** `enrich()` dispatches on tool name; unknown tool → `None` → diagnostic emitted with
`ParseStatus::Unparsed`. Caller logs nothing, returns raw output. Silent to the user.

**Confirmed by:** Reading `o8v-check/src/enrich.rs` — the `None` arm emits `ParseStatus::Unparsed`
but the caller in the pipeline does not surface it as an error. A tool misspelled in config
produces no diagnostics and no warning.

**Impact:** Medium. Only affects misconfigured stacks. Not silent in the same way as F11 —
`Unparsed` propagates — but the UI does not show it to the user as a warning.

**Status:** Tracked, not fixed in this session (out of scope).

---

### F7 / F16 — detect loop exit conditions [LOW] ✅ TRUE POSITIVE (LOW)

**Claim:** Two loop conditions in `o8v-project/src/detect.rs` iterate over candidates with
no explicit fallback log when all candidates are exhausted.

**Confirmed by:** Reading the loop — it returns `None` silently when no candidate matches.
The caller logs at `debug` level only. A project type that should be detected but isn't
produces no user-visible error.

**Impact:** Low. Detection failure shows as "unknown stack" in output, which is visible.
Not a true silent fallback — just a missing `warn!` log.

**Status:** Tracked, not fixed.

---

## False Positives (18 of 21)

All remaining 18 findings were grep matches on `return None` or `Err(_) =>` patterns that,
on semantic tracing, had one of these explanations:

1. **Option used correctly** — the `None` is propagated up to a caller that handles it
   explicitly (e.g., `?` operator, `unwrap_or`, `match`).
2. **Error is logged before discard** — `tracing::warn!` or `tracing::error!` emitted
   before the `return None`.
3. **Not a diagnostic path** — the code path does not affect whether a diagnostic is emitted
   or whether `ParseStatus` is set. E.g., path normalization helpers, display formatting.
4. **Test code** — the pattern appeared inside `#[cfg(test)]` blocks.

---

## Meta-Learnings

### Grep-based audits have ~86% FP rate on this codebase

Searching for `return None` or `Err(_) => return None` finds syntactic patterns, not
semantic ones. The question is not "does this return None" but "does returning None here
cause a diagnostic to be silently dropped with no signal to the caller."

### The right audit method

For each `return None` in a parser:
1. Trace back: which caller receives this `None`?
2. Does the caller set `any_dropped = true` or equivalent?
3. Does `ParseStatus` reflect the drop?
4. Is there a test that proves the status is `Unparsed` when a diagnostic is dropped?

If all four: not a bug. If any missing: true positive.

### Counterexample > grep

A finding is not confirmed until a failing test exists on pre-fix code.
F11 was confirmed in ~10 minutes by writing one test. The other 18 took 2 hours of
grep-then-trace. Build the repro first, not last.