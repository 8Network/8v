# Design: Actionable hint when symbol map is empty

## Scope
Plain-text message only. One line change in `render_plain()`. No auto-fallback, no parser
changes, no new flags, no `--json` shape changes.

## Problem
`8v read <path>` on a prose file returns `(no symbols found)` with no guidance. Agent guesses
the next command, adds a retry turn, pays extra tokens. Evidence: v3 benchmark P1 (Opus
O1/O2/O3, 3/6 runs) + dogfood Section 11: "A message of 'no symbols found; use --full to
read as text' would eliminate the second call."

## Current behavior
```
empty.txt (10 lines)

  (no symbols found)
```

## Proposed behavior
```
empty.txt (10 lines)

  (no symbols found — use `8v read empty.txt --full` to read as text)
```

## Implementation
One line in `o8v-core/src/render/read_report.rs:80`, inside `render_plain()`:

```rust
// before
output.push_str("\n  (no symbols found)\n");
// after
output.push_str(&format!(
    "\n  (no symbols found — use `8v read {path} --full` to read as text)\n"
));
```

`path` is already bound in the match arm — no API changes, no new parameters.

`--json` path (`render_json()`) calls `serde_json::to_string(self)` directly — no separate
emit site, no hint in JSON output. JSON is intentionally unchanged; `symbols: []` is the
machine signal.

## Test plan (failing-first)
1. **Update** `test_render_plain_symbols_empty`: change assertion to
   `assert!(text.contains("no symbols found — use `8v read empty.txt --full`"))`.
   Fails on old code; passes after change.
2. **New** `test_render_plain_symbols_empty_path_hint`: `path: "src/lib.rs"`, `symbols: vec![]`,
   assert `text.contains("8v read src/lib.rs --full")`. Confirms path substitution.
   Fails on old code; passes after change.

## Acceptance criteria
- Both tests fail on pre-change code (verified before commit).
- Both tests pass after change.
- `8v check .` passes on workspace.
- `render_json()` output unchanged.
- No new flags, no new struct fields, no auto-fallback logic.

## Non-risks
- JSON consumers: unaffected — `render_json()` is a different code path.
- Batch (`ReadReport::Multi`): delegates to sub-reports' `render_plain()`; hint appears
  automatically for any empty-symbol sub-entry.

## Review gate
- Does any path cause agent to silently fall back to `--full` content? No — advisory text only.
- Can test 1 pass on pre-change code? No — old assertion checks "no symbols found" (substring
  still present), so updated assertion must check the new suffix. Ensure updated assertion
  checks the full hint string, not just the old substring.
