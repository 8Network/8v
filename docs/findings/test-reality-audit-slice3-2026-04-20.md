# Test Reality Audit — Slice 3: Empty Symbol Map Hint
**Date:** 2026-04-20
**File under test:** `o8v-core/src/render/read_report.rs`
**Feature:** Empty-symbol-map hint in `render_plain()` — path-substituted, em-dash wording, plain-only

---

## Verdict

**Slice 3 tests had one critical gap (M4) that let a real bug slip past all four tests.**

The trailing `\n` on the hint line could be silently deleted and every test would still pass. This is a direct consequence of all four tests using `contains()` assertions only — substring presence is necessary but not sufficient to verify output structure.

Two gap-closing tests were added. All 13 tests now pass. The audit is complete.

---

## Production Code Under Test

Location: `o8v-core/src/render/read_report.rs`, inside `impl Renderable for ReadReport`, arm `Symbols { path, total_lines, symbols }`:

```rust
if symbols.is_empty() {
    output.push_str(&format!(
        "\n  (no symbols found \u{2014} use `8v read {path} --full` to read as text)\n"
    ));
}
```

This replaced the HEAD version which had a static, path-less, em-dash-less message:

```rust
if symbols.is_empty() {
    output.push_str("\n  (no symbols found)\n");
}
```

The production upgrade was part of the preceding session's work and is intentional. The audit treated the current working-tree version as the code under test.

---

## Mutations Tested (M1–M6)

### M1 — Remove path substitution (literal `{path}` string)
```rust
// mutant
output.push_str(&format!(
    "\n  (no symbols found \u{2014} use `8v read {path} --full` to read as text)\n"
));
// changed to static string without interpolation — literal "{path}" in output
```

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | FAIL — asserts `8v read empty.txt --full` |
| `test_render_plain_symbols_empty_hint_text` | FAIL — asserts `8v read lib.rs --full` |
| `test_render_plain_symbols_empty_hint_path_substitution` | FAIL — asserts path-specific strings |
| `test_render_json_symbols_empty_unchanged` | PASS — JSON test doesn't care about plain output |

**Detected: YES (3/4 tests fail)**

---

### M2 — Typo in hint wording ("symols" instead of "symbols")
```rust
// mutant
"\n  (no symols found \u{2014} use `8v read {path} --full` to read as text)\n"
```

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | FAIL — asserts `no symbols found \u{2014}...` |
| `test_render_plain_symbols_empty_hint_text` | FAIL — asserts `no symbols found \u{2014}...` |
| `test_render_plain_symbols_empty_hint_path_substitution` | PASS — only checks path strings, not wording |
| `test_render_json_symbols_empty_unchanged` | PASS — JSON test doesn't inspect plain output |

**Detected: YES (2/4 tests fail)**
**Gap identified:** `test_render_plain_symbols_empty_hint_path_substitution` doesn't assert hint wording.

---

### M3 — Em-dash replaced with hyphen
```rust
// mutant
"\n  (no symbols found - use `8v read {path} --full` to read as text)\n"
```

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | FAIL — asserts `\u{2014}` |
| `test_render_plain_symbols_empty_hint_text` | FAIL — asserts `\u{2014}` |
| `test_render_plain_symbols_empty_hint_path_substitution` | PASS — only checks path strings, not em-dash |
| `test_render_json_symbols_empty_unchanged` | PASS — JSON test asserts `\u{2014}` absent, hyphen still absent in JSON |

**Detected: YES (2/4 tests fail)**
**Gap confirmed:** same as M2 — path_substitution test doesn't cover wording or em-dash.

---

### M4 — Drop trailing newline from hint
```rust
// mutant
"\n  (no symbols found \u{2014} use `8v read {path} --full` to read as text)"
// trailing \n removed
```

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | PASS — uses `contains()`, doesn't check end |
| `test_render_plain_symbols_empty_hint_text` | PASS — uses `contains()`, doesn't check end |
| `test_render_plain_symbols_empty_hint_path_substitution` | PASS — uses `contains()`, doesn't check end |
| `test_render_json_symbols_empty_unchanged` | PASS — JSON test doesn't check plain output structure |

**Detected: NO — SLIPS PAST ALL FOUR TESTS**
**Root cause:** Every slice-3 test uses `text.contains(...)`. Substring presence is necessary but not sufficient. Missing trailing `\n` means the next caller that writes to a terminal or pipes the output sees a malformed line — no tests caught this.

---

### M5 — Remove entire hint (static fallback, no em-dash, no path)
```rust
// mutant
output.push_str("\n  (no symbols found)\n");
```

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | FAIL — asserts em-dash and `--full` |
| `test_render_plain_symbols_empty_hint_text` | FAIL — asserts em-dash and `--full` |
| `test_render_plain_symbols_empty_hint_path_substitution` | FAIL — asserts `8v read src/alpha.rs --full` |
| `test_render_json_symbols_empty_unchanged` | PASS — JSON test doesn't inspect plain text |

**Detected: YES (3/4 tests fail)**

---

### M6 — Inject hint into `render_json()` too
The `render_json()` implementation serializes `self` with `serde_json::to_string`. The mutation would inject the hint text into the JSON output (by prepending or appending it to the `json` string).

| Test | Result |
|---|---|
| `test_render_plain_symbols_empty` | PASS — only checks plain output |
| `test_render_plain_symbols_empty_hint_text` | PASS — only checks plain output |
| `test_render_plain_symbols_empty_hint_path_substitution` | PASS — only checks plain output |
| `test_render_json_symbols_empty_unchanged` | FAIL — asserts `--full` absent and `\u{2014}` absent in JSON |

**Detected: YES (1/4 tests fail)**

---

## Mutation Summary Table

| ID | Mutation | Tests failing | Detected? |
|---|---|---|---|
| M1 | Remove path substitution | 3/4 | YES |
| M2 | Typo "symols" | 2/4 | YES |
| M3 | Em-dash → hyphen | 2/4 | YES |
| M4 | Drop trailing `\n` | **0/4** | **NO** |
| M5 | Remove entire hint (static fallback) | 3/4 | YES |
| M6 | Inject hint into JSON output | 1/4 | YES |

**Weakest mutation: M4.** Detection rate: 5/6 mutations caught = 83%.

---

## Gap Analysis

### Gap 1 — M4: Trailing newline not verified (all 4 tests)
**Root cause:** All four Slice 3 tests use only `contains()`. `contains()` verifies that a substring appears somewhere in the output. It says nothing about what comes after the substring — a missing trailing `\n` is invisible.

**Impact:** A caller rendering output to a terminal would see the hint line immediately joined to the next line of output, no blank line separation. This is a visual correctness bug.

**Fix:** Added `test_render_plain_symbols_empty_hint_ends_with_newline`, which:
1. Asserts `text.ends_with('\n')` — whole plain output must end with newline
2. Locates the hint substring via `text.find("no symbols found")`
3. Asserts the slice after the hint contains `'\n'` — the hint block itself must have a trailing newline

### Gap 2 — M2/M3: `test_render_plain_symbols_empty_hint_path_substitution` doesn't assert hint wording or em-dash
**Root cause:** This test was written to verify path embedding and cross-path isolation only. It deliberately avoided asserting the full hint string, leaving em-dash and "no symbols found" wording uncovered.

**Impact:** A mutation that silently changes the canonical hint wording or replaces the em-dash with a hyphen goes undetected by this test. M2 and M3 both slip past it.

**Fix:** Added `test_render_plain_symbols_empty_hint_path_substitution_full_text`, which asserts the full canonical hint string including em-dash, path, and `--full` flag for a third distinct path (`src/gamma.rs`).

---

## Gap-Closing Tests Added

Both tests are in `o8v-core/src/render/read_report.rs` inside the existing `#[cfg(test)]` module, after the four original Slice 3 tests. No existing tests were modified.

**`test_render_plain_symbols_empty_hint_ends_with_newline`** — catches M4
- Path: `gap.rs`
- Asserts `text.ends_with('\n')`
- Asserts substring after `"no symbols found"` contains `'\n'`

**`test_render_plain_symbols_empty_hint_path_substitution_full_text`** — catches M2/M3 gap
- Path: `src/gamma.rs`
- Asserts full canonical hint: `"no symbols found \u{2014} use \`8v read src/gamma.rs --full\` to read as text"`

### Final test count

```
test render::read_report::tests::test_render_human_symbols ... ok
test render::read_report::tests::test_render_json_full ... ok
test render::read_report::tests::test_render_json_range ... ok
test render::read_report::tests::test_render_json_symbols ... ok
test render::read_report::tests::test_render_json_symbols_empty_unchanged ... ok
test render::read_report::tests::test_render_plain_full ... ok
test render::read_report::tests::test_render_plain_range ... ok
test render::read_report::tests::test_render_plain_symbols ... ok
test render::read_report::tests::test_render_plain_symbols_empty ... ok
test render::read_report::tests::test_render_plain_symbols_empty_hint_ends_with_newline ... ok
test render::read_report::tests::test_render_plain_symbols_empty_hint_path_substitution ... ok
test render::read_report::tests::test_render_plain_symbols_empty_hint_path_substitution_full_text ... ok
test render::read_report::tests::test_render_plain_symbols_empty_hint_text ... ok

test result: ok. 13 passed; 0 failed
```

Post-gap-close detection rate: **6/6 mutations caught = 100%**.

---

## Git Diff Status

`git diff o8v-core/src/render/read_report.rs` shows intentional changes only — no mutation artifacts:

1. Production code: static `"(no symbols found)"` replaced with path-interpolated, em-dash hint
2. Existing test `test_render_plain_symbols` upgraded: added negative assertion that hint does NOT appear when symbols are present
3. Existing test `test_render_plain_symbols_empty` upgraded: asserts full hint string with em-dash and `--full` (was bare `"no symbols found"`)
4. Four new Slice 3 tests added
5. Two gap-closing tests added

All six mutations have been fully reverted. Working tree contains only the intended production improvements and new tests.

---

## 8v Feedback

Friction observed during this audit session:

1. **Linter auto-modification breaks the edit flow.** After applying M3 (em-dash → hyphen) and running `cargo test`, the formatter auto-modified the file. The next `8v write --find/--replace` call failed with "File has been modified since read." The fix was a re-read before the edit. This friction cost one extra round-trip. Expected behavior: `8v write` should detect and report the modification (which it does), but a lighter-weight "re-read + retry" path would reduce friction.

2. **`8v write --find "<old>" --replace "<new>"` is the right tool for mutation testing.** The find-replace interface is clean and precise. No issues with the basic write flow.

3. **`8v read <path>:<start>-<end>` is the correct tool for confirming mutation state.** After each `--find/--replace`, reading back the mutated lines confirms the edit landed correctly before running cargo. This two-step (write then spot-read) is the right pattern and worked without friction.

4. **`8v` has no equivalent to `git diff` for state verification.** Verifying that all mutations had been reverted required falling back to Bash (`git diff`). An `8v status` or `8v diff` command showing working-tree changes relative to HEAD would keep the flow entirely within 8v.
