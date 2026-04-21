# QA Findings: `8v ls` — 2026-04-20

Binary: `target/debug/8v` (commit 2681102, built 2026-04-20)
Working directory for all tests: `/Users/soheilalizadeh/8/products/vast/oss/8v`
Method: run every documented form, record exit code + output, classify verdict

---

## 1. Form-by-Form Table

| Form | Exit | Verdict | Notes |
|------|------|---------|-------|
| `8v ls` | 0 | ✓ | Summary view. Shows project count, file count, stack. |
| `8v ls <path>` | 0 | ✓ | Scoped summary. Works for existing dirs. |
| `8v ls <nonexistent>` | 1 | ✓ | Prints error, exits 1. |
| `8v ls --tree` | 0 | ✓ | Tree view. Files + stack annotations. |
| `8v ls --tree --depth 1` | 0 | ✓ | Depth filter works correctly with `--tree`. Footer shows "N files filtered". |
| `8v ls --depth 1` (no `--tree`) | 0 | ✗ | Shows only summary + "596 files filtered" footer. No files listed. Depth filter applied invisibly. |
| `8v ls --loc` (no `--tree`) | 0 | ◐ | `--loc` silently ignored in default view. Summary shows no LOC data. No warning. |
| `8v ls --tree --loc` | 0 | ✓ | LOC counts appended to each file. Works as documented. |
| `8v ls --files` | 0 | ✓ | Flat file list. Default cap 500. Footer: "Showing 500 of 501 files". |
| `8v ls --files --limit 1000` | 0 | ✓ | Shows all 628 files. Correct. |
| `8v ls --files --limit 10` | 0 | ✓ | Shows first 10 files. Footer: "Showing 10 of 11 files". Truncation signal present. |
| `8v ls --match "*.rs"` | 0 | ✓ | Filters to Rust files. Summary counts reflect filter. |
| `8v ls --match "foo***bar"` | 0 | ✗ | Invalid glob accepted silently. Treated as no-match. Exit 0 with empty results. No parse error. |
| `8v ls --stack rust` | 0 | ◐ | Accepts known stack. Does NOT filter files. Only changes the displayed stack label. |
| `8v ls --stack INVALID` | 0 | ✗ | Accepts unknown stack silently. No validation error. No visible effect. |
| `8v ls --tree --files` | 0 | ✗ | `--files` silently ignored when `--tree` is passed. Tree view renders instead. No warning. |
| `8v ls --json` | 0 | ◐ | JSON output works. But `total_files` is wrong when truncated (see Issue 2). |
| `8v ls --json --tree` | 0 | ◐ | `--tree` silently ignored in JSON mode. Output is flat regardless. No warning. |
| `8v ls --plain` | 0 | ✓ | Strips color/styling. View mode unchanged. Correct. |
| `8v ls --plain --tree` | 0 | ✓ | Plain tree. Color stripped. Correct. |
| `8v ls --meta` | 0 | ◐ | Listed in `--help`. No visible effect in default summary view. |
| `8v ls <subdir>` | 0 | ◐ | Stack detection shows `unknown` for subdirs inside a Rust workspace. |
| `8v ls --tree` (root vs subdir) | 0 | ◐ | Root renders as `/  [rust]`. Subdir renders as `./  [unknown]`. Label inconsistency. |

---

## 2. Top 5 Issues — Agent Parsing Friction

### Issue 1: `--json total_files` lies when truncated

`8v ls --json` hard-caps output at 500 files. When the project has more than 500 files, `total_files` in the JSON reports 500 (the cap), not the actual count.

Observed:
```
"total_files": 500
"truncated": true
```

Actual file count: 628 (confirmed via `--files --limit 1000`).

Agent impact: An agent reading `total_files` to decide how many files exist will undercount by 20%+. If the agent uses this to drive iteration, it stops early. The `truncated: true` flag is present but an agent would need to know to distrust `total_files` when it's set — that's an undocumented invariant.

### Issue 2: `--files` footer is a peek-n+1 artifact, not real total

`8v ls --files` (default cap 500) shows: `Showing 500 of 501 files`.

The "501" is not the real file count. It is the result of a peek-n+1 check: the code checks if one more file exists beyond the cap to signal truncation. The real total is 628.

Agent impact: An agent trying to understand scale from the footer will see "501" and conclude the project has ~500 files. It has 628. The footer actively misleads on total size.

### Issue 3: `--depth N` without `--tree` applies filter silently

`8v ls --depth 3` shows:
```
1 project detected
596 files filtered
```

No files are listed. The word "filtered" is ambiguous — agents often interpret "N files filtered" as "N files matched the filter" rather than "N files were excluded." In this case, it means the opposite: depth filtering excluded those files, but no view is rendered that would show what remains. The output surface is a summary header with a confusing counter.

Agent impact: An agent trying to limit scope with `--depth 3` and omitting `--tree` gets an empty response and may retry with different commands, producing unnecessary tool calls.

### Issue 4: `--stack` does not filter files

The `--help` entry for `--stack` says "Show only projects of this stack". The actual behavior is: accept the flag, change the stack label in the summary, show all files unchanged.

`8v ls --stack python` in a Rust-only workspace still shows 628 Rust files with the label changed to `python`.

`8v ls --stack INVALID_STACK` is accepted without error.

Agent impact: An agent scoping a search to a specific language stack will use `--stack typescript` expecting filtered output. It will receive everything. The false positive rate is 100%.

### Issue 5: Conflicting flag pairs give no signal

Three flag conflicts are silently resolved by ignoring one side:
- `--tree --files`: `--files` is dropped, tree renders
- `--json --tree`: `--tree` is dropped, flat JSON renders
- `--loc` (no `--tree`): `--loc` is dropped, summary renders

In each case: no warning, no error, no indication in output. An agent that used the wrong combination gets data in an unexpected shape with no diagnostic. It cannot distinguish "flag was silently ignored" from "flag does not apply here."

---

## 3. Missing Features (Freeze Queue — No Implementation)

These go into the backlog. Nothing here is a Phase 0 build target.

1. **`--stack` as a real filter.** Filter files to only those belonging to projects of the given stack. Right now it only changes the label. The help text already describes the intended behavior — the implementation is wrong.

2. **`--depth N` without `--tree`.** Either error ("--depth requires --tree") or activate `--tree` automatically. The current "N files filtered" summary with no file list is a dead end.

3. **`--loc` without `--tree`.** Add LOC totals to the default summary view. Or error with "use --tree --loc for per-file counts."

4. **`--json total_files` accuracy.** Report the real count, not the cap. If the real count is expensive to compute, add a `total_files_exact: false` flag.

5. **`--stack` validation.** Reject unknown stack names with exit 1 and a message listing valid values.

6. **`--meta` documentation.** Either implement visible behavior or remove from `--help`.

7. **Glob pattern validation.** Reject malformed glob patterns (e.g., `foo***bar`) at parse time with exit 2 and an error message.

8. **Hidden file indication.** When hidden files are excluded, show a count or a note in the summary ("N hidden files excluded").

---

## 4. Noise and Redundancy

These are candidates for a doc-slice or a tiny render fix in Phase 0 hardening:

**Noise 1: `--files` footer "501" misrepresents scale.**
A one-line render fix: change the peek-n+1 display from `"N of N+1"` to `"N of (N+ more)"` or simply `"first N (more exist)"`. Does not require changing the underlying detection logic.

**Noise 2: Root vs subdir stack label.**
Root: `/  [rust]`. Subdir: `./  [unknown]`. The inconsistency is cosmetic but agents see two different format patterns and may fail to parse the stack name consistently. A doc-slice is sufficient until `--stack` is fixed.

**Noise 3: `--depth` footer "N files filtered" = excluded, not matched.**
The word "filtered" conventionally means "what passed through." Here it means "what was excluded." Rename to "files excluded by depth" or "N files beyond depth N". One-line message change.

**Noise 4: `--tree` without `--depth` has no footer.**
`--tree` shows files but no summary line. `--tree --depth 1` shows "596 files filtered." An agent reading `--tree` output cannot tell if it saw everything or if there is a default depth limit. Add a consistent footer to `--tree` output regardless of `--depth`.

**Noise 5: `--json --tree` drop is silent.**
The least disruptive fix: add `"tree_ignored": true` to JSON output when `--tree` is passed in JSON mode. One field, visible, agents can detect.

---

## 5. Proposed Next Test Slice for `ls`

If a follow-up QA slice is warranted, these are the highest-value probes not covered today:

1. **`--match` + `--stack` combined.** Do they compose? Does one take precedence? Run `8v ls --match "*.ts" --stack rust` and inspect which filter wins.

2. **`--loc` + `--files` combined.** Does `--files --loc` produce LOC in the flat list? Or is `--loc` silently ignored as it is in the default view?

3. **`--depth` edge cases.** `--depth 0`, `--depth 999`, `--depth -1`. Does negative depth error or silently clamp?

4. **Large repo accuracy.** Run `8v ls --json` on a repo with exactly 501 files and verify `total_files` vs actual count vs truncated field. This builds a repro for Issue 1.

5. **`--tree` with `--limit`.** Does `--limit` apply to `--tree` output? Is it documented?

6. **Permission-denied directory.** `8v ls /root` or a chmod-000 directory. Does it error cleanly or silently exclude?

---

## 8v Feedback

**Friction logged from this QA session:**

1. **`8v ls --json` total_files is wrong when truncated.** I had to run `--files --limit 1000` to get the real count. The JSON field should either be accurate or flagged as an estimate.

2. **`--depth` without `--tree` produces an unusable output surface.** The summary counter with no file list is a dead end. Wasted a probe to discover this was the behavior.

3. **`--stack` validation gap cost extra probes.** I expected an error on `--stack INVALID_STACK`. Got exit 0. Had to test several stack names to understand the behavior model.

4. **Flag conflict resolution is invisible.** Running `--tree --files` should take 1 probe. Instead it took 3 (initial + realize --files was silently dropped + verify). A warning on stderr like `note: --files ignored with --tree` would collapse this to 1.

5. **`--files` "501" footer misleads on repo scale.** I spent 2 extra probes reconciling 501 (footer) vs 500 (JSON total_files) vs 628 (actual). These three numbers should agree.
