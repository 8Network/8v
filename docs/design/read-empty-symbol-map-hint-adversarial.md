# Review R1: read-empty-symbol-map-hint

Source verified: `o8v-core/src/render/read_report.rs`

## Grounding checks

- **Emit site confirmed.** Line 80 is exactly `output.push_str("\n  (no symbols found)\n");` inside
  `ReadReport::Symbols` arm of `render_plain()`. Matches design claim.
- **`path` in scope.** Lines 73-76 destructure `path, total_lines, symbols` — `path` is bound
  before line 80. No API change needed. Confirmed.
- **`render_json()` path.** Lines 137-143: calls `serde_json::to_string(self)` directly. No text
  formatting, no separate emit site. JSON is untouched. Confirmed.
- **`symbols: []` serializes correctly.** `ReadReport::Symbols` derives `Serialize`; an empty
  `Vec<SymbolEntry>` serializes as `"symbols":[]`. JSON consumers already get the machine signal.

---

## Findings

### BLOCKER — none

### TEST GAP

**T1. Existing test `test_render_plain_symbols_empty` (line 183) will NOT fail on pre-change
code.**
> Design line 47: "change assertion to `assert!(text.contains("no symbols found — use ..."))`".

The current assertion is `assert!(text.contains("no symbols found"))` (line 193). The new
assertion substring `"no symbols found — use ..."` still contains `"no symbols found"` as a
prefix, so if a developer edits the assertion but forgets to update the code, the test still
passes on pre-change output. Worse: if the assertion is changed to the full new string before
the code change, it correctly fails — but the design's own "Review gate" note (line 68) says
"ensure updated assertion checks the full hint string" without specifying the fix. The fix is
to assert the full suffix: `"no symbols found — use \`8v read empty.txt --full\` to read as
text"` — not any shorter prefix. The design says this but does not enforce it in the test body
it actually writes.

**T2. `test_render_human_symbols` (line 310) uses `path: "lib.rs"` with `symbols: vec![]`.**
After the change, `render_plain()` will embed `"8v read lib.rs --full"` in its output.
`render_human()` delegates to `render_plain()`. The test only asserts `plain == human` — it
will still pass. But if `render_human()` ever diverges, this test is the only coverage and it
does not assert the hint text. Not a blocker; the two proposed tests cover it.

### RISK

**R1. Path with special characters embedded in backtick hint.**
The hint emits:  `` `8v read {path} --full` ``
If `path` contains a backtick, newline, or `$()` the displayed hint will be malformed or
misleading. The risk is cosmetic only (render_plain produces a plain String, no shell
execution). An agent parsing the hint as a shell command and blindly running it could be
confused by a path like `my$(rm -rf /)file.txt`. In practice, 8v's own path handling should
reject such paths before a ReadReport is constructed — but the design does not mention this
dependency. Low probability; cosmetic at worst since no shell is involved.

**R2. Long absolute paths make hint verbose but not broken.**
A path like `/Users/soheil/projects/org/some/deep/nested/file.md` produces a 60+ char hint
on the same line as the em-dash. Terminal wrapping handles it; no truncation occurs. Acceptable.

**R3. "may be prose" ambiguity for binary/empty files.**
The hint says "read as text" regardless of why symbols are empty (binary, empty file, encoding
error). "Read as text" is a slightly misleading suggestion for a zero-byte or binary file. A
binary file read with `--full` will produce garbage output. The hint is advisory and harmless,
but an agent acting on it for a binary file wastes a turn. The design treats this as a non-risk;
it is a low-probability agent confusion, not a blocker.

### NIT

**N1. JSON consumers — rule-4 ("no silent fallback") does not apply here.**
Design line 41-43 justifies omitting the hint from JSON. The rule-4 concern would only apply if
JSON output were silently suppressing an error condition. `symbols: []` is not an error — it is
a correct, complete representation. The design's reasoning is sound. No action needed.

**N2. Batch output verbosity.**
Multi-file read with several empty-symbol files produces one hint per file (via sub-report
delegation, lines 121-126). For a large batch of prose files this multiplies. The design lists
this under "Non-risks"; acceptable given that each hint is one short line.

---

## Verdict

Zero blockers. One test-gap (T1) that the design itself acknowledges but does not fully close:
the proposed assertion must use the full suffix string, not any substring that remains valid on
old code. Proceed with that fix confirmed in the test before commit.
