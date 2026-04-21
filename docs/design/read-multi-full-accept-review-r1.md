# Review R1: read-multi-full-accept.md

Reviewer: adversarial round 1 — 2026-04-19

---

## Blockers

None.

---

## Test gaps

**TG-1 — Test 2 (triple) cannot fail-then-pass in isolation from test 1.**
Design says both tests must be "red on pre-fix HEAD." With `SetTrue` (current), the
*second* `--full` causes an error, so both double and triple invocations fail. Both
turn green together after the fix. The independent failure evidence is real, but the
doc's "red-then-green" framing implies each test proves something distinct. Triple
adds no incremental proof: if `overrides_with_self` is applied, triple passes for the
same structural reason as double. A stronger test would assert `stdout` is
*byte-for-byte identical* to single-`--full` output (not merely "contains file
content") — that would catch a hypothetical future renderer treating count > 1
differently. The doc's test 2 asserts `stdout identical to single---full output` —
that wording is present but only in prose, not as a concrete assertion pattern.
Ensure the test actually diffs the two outputs, not just checks a substring.

---

## Risks

**R-1 — `overrides_with_self` changes clap's generated help text shape slightly.**
Clap 4 appends `[may be specified multiple times]` (or similar) to the help line for
args that carry `overrides_with_self`. The doc says "behavior is otherwise identical"
but does not address `--help` / `-h` output or `--json` schema output. This is not a
blocker — the semantic behavior is correct — but `8v read --help` will look
marginally different. No test today asserts help text, so no regression risk, but the
doc's "Non-risks" section should acknowledge it.

---

## Nits

**N-1** — §Implementation: "This is annotation-only" is accurate but line 14 of the
doc cites `o8v/src/commands/read.rs:23-25` as current code. The actual `#[arg(long)]`
and `pub full: bool` are at lines 24-25 (the doc-comment is line 23). Minor off-by-one
in the citation; does not affect the design.

**N-2** — Acceptance criterion 3 says "no caller diffs outside the annotation line."
There are two call sites (`read.rs:203` and `read.rs:226`) that both pass `args.full`.
Both are untouched by the change — criterion 3 is correct — but naming them explicitly
would make verification faster for the implementer.

---

## Summary

| Category   | Count |
|------------|-------|
| Blockers   | 0     |
| Test gaps  | 1     |
| Risks      | 1     |
| Nits       | 2     |

**Recommendation: proceed to implementation.**

The one test gap (TG-1) is a hardening note, not a stopper. Test 2 should use a
byte-level diff against single-`--full` output, not a substring check — make that
explicit in the test body. R-1 (help-text cosmetic change) warrants one sentence in
§Non-risks; no code change required.
