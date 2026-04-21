# QA Sweep Round 1 — Bug Register
2026-04-20 · Sources: error-contract, command-qa-search, command-qa-ls, command-qa-read, instruction-clarity-test

---

## §1 Bug Register

| ID | Command | Summary | Silent? | Blast-radius | Type | Source |
|----|---------|---------|---------|-------------|------|--------|
| BR-01 | write | Double-prefix `error: error:` on stderr | No | Every write error; parser sees garbled output | RENDER | error-contract BUG-1 |
| BR-02 | read | Malformed range `path:abc` consumed into filename | No (misleading) | Every non-numeric range attempt | CONTRACT | error-contract BUG-2; read-qa range-parsing |
| BR-03 | search | `permission-denied` errors swallowed; exit 0 | Yes | Any restricted-dir search silently incomplete | BEHAVIOR | error-contract BUG-3; search-qa Issue 2 |
| BR-04 | search | Single-file search: `path` field is empty string in output and `--json` | No (wrong output) | Agents parsing path field get empty string | CONTRACT | search-qa Issue 1 |
| BR-05 | search | Exit code 1 overloaded: no-match AND read-error AND invalid-args all return 1 | No | Machine consumers cannot distinguish error from empty | CONTRACT | search-qa Issue 2 |
| BR-06 | search | Binary files invisible: not in `files_skipped`, never named | Yes | Mixed repos silently miss files | BEHAVIOR | search-qa Issue 3 |
| BR-07 | search | `--limit 0` silently accepted; exits 0 with "no matches found" | Yes | Agents get false no-match signal | BEHAVIOR | search-qa Issue 5 |
| BR-08 | search | Errors (invalid regex, bad -C, empty pattern) emitted to STDOUT not STDERR | No (wrong channel) | Parsers mix output with errors; `--json` returns plain text | CONTRACT | search-qa Issue 4 |
| BR-09 | ls | `--json total_files` = 500 (cap artifact) when actual = 628 | No (misleading count) | Any consumer using total_files as real count is wrong | CONTRACT | ls-qa Issue 1 |
| BR-10 | ls | `--files` footer "501 of 501" is peek-N+1 artifact; real count 628 | No (wrong number) | File count reporting wrong in text mode | CONTRACT | ls-qa Issue 2 |
| BR-11 | ls | `--stack INVALID` silently accepted; exit 0, no filter applied | Yes | Agent typos pass undetected | BEHAVIOR | ls-qa Issue 4 |
| BR-12 | ls | Conflicting flag pairs (`--tree --files`, `--json --tree`, `--loc` without `--tree`) silently resolved | Yes | Agent flag errors give no signal | BEHAVIOR | ls-qa Issue 5 |
| BR-13 | read | Range `path:-10` strips path, discards range; `path:0-10` silently corrects 0 to 1 | Yes / Misleading | Silent data loss on negative or zero-indexed ranges | BEHAVIOR | read-qa range-parsing |
| BR-14 | read | Binary file: exits 0 with error message on STDOUT (not exit 1 / STDERR) | Partially | Exit code signals success; message only on stdout | CONTRACT | read-qa binary |
| BR-15 | read | Range overlap: overlapping batch entry is silently empty | Yes | Agents receive empty content with no error or warning | BEHAVIOR | read-qa range-overlap |
| BR-16 | read | `kind` column absent in text-mode output; present only in `--json` | No (asymmetry) | Agents must use --json and parse different structure | CONTRACT | read-qa kind-column |
| BR-17 | read | Batch JSON shape asymmetry: single=`{"Symbols":{}}` vs batch=`{"Multi":{"entries":[]}}` | No (breaking) | Every batch consumer must branch on response shape | CONTRACT | read-qa batch-json |
| BR-18 | all | `--json` only emits structured errors for batch read; all others emit plain-text STDERR | No (contract gap) | Machine consumers receive unstructured errors | CONTRACT | error-contract BUG-5; search-qa Issue 4 |
| BR-19 | all | Six distinct STDERR prefix patterns; no machine-parseable taxonomy | No (fragmentation) | Parsers must handle 6 different prefix formats | RENDER | error-contract INCONSISTENCY-7 |
| BR-20 | init | Failure split across STDOUT and STDERR simultaneously | No (split output) | Consumers must read both streams to detect init failure | CONTRACT | error-contract INCONSISTENCY-8 |
| BR-21 | instruction-surfaces | Error/exit-code contract absent from MCP description and ai_section.txt | No (doc gap) | Agents cannot construct correct error-handling loops; Axis 3 = 2.33/10 | DOC | instruction-clarity P0 |
| BR-22 | write | `--find/--replace` multi-occurrence behavior undefined (zero-match documented; N>1 not) | No (doc gap) | Agents cannot predict behavior on duplicate strings | DOC | instruction-clarity P0 |
| BR-23 | search | CWD dependency undocumented: absolute paths outside git repo fail silently | Yes | Agents using absolute paths outside repo get silent failure | BEHAVIOR | search-qa noise |

---

## §2 Cross-Cutting Patterns

Each pattern requires 3+ bugs in this register.

### P-A: Silent failure
BR-03, BR-06, BR-07, BR-11, BR-12, BR-13, BR-15, BR-23 — 8 bugs

Permission errors, binary files, invalid inputs, flag conflicts, range edge cases, and path mismatches all fail without any user-visible signal. The common mechanism: the code path encounters the error condition and either drops it (search), silently corrects it (read range 0 to 1), or ignores validation (ls stack). No uniform "something was skipped" reporting surface exists.

### P-B: Exit-code overload / underspecified
BR-03 (exit 0 on perm-denied), BR-05 (exit 1 for 3 distinct conditions), BR-07 (exit 0 for invalid input), BR-14 (exit 0 for binary error) — 4 bugs

Exit codes do not distinguish success, soft error, hard error, or invalid input. Machine consumers cannot branch on exit code alone.

### P-C: STDOUT/STDERR channel violations
BR-08 (search errors on STDOUT), BR-14 (read binary error on STDOUT), BR-20 (init split across both) — 3 bugs

The invariant "errors go to STDERR" is violated in at least 3 commands. `--json` amplifies this: consumers expect a JSON stream on STDOUT and receive mixed error prose.

### P-D: `--json` contract incomplete
BR-17 (batch shape asymmetry), BR-18 (only batch read emits structured errors), BR-08 (plain text for search errors), BR-16 (kind absent in text but present in json) — 4 bugs

`--json` was designed as a machine-readable mode but the contract is only partially honored. Structured error emission is the exception (batch read) rather than the rule.

### P-E: Count / metric lying
BR-09 (total_files = 500 cap artifact), BR-10 (footer peek-N+1), BR-07 (limit 0 produces false no-match) — 3 bugs

Three separate count-reporting surfaces produce numbers that do not match reality. All three silently suppress the discrepancy.

---

## §3 Slice Map

| Bug IDs | Owning area | What a slice touches |
|---------|------------|---------------------|
| BR-01 | o8v-render, o8v-cli write path | Remove one error-prefix layer |
| BR-02, BR-13 | o8v-fs / range parser | Validate range tokens before path construction |
| BR-03, BR-06, BR-23 | o8v-process search executor | Surface skipped files/dirs via files_skipped field |
| BR-04, BR-05 | o8v-check search command | Fix path field; define distinct exit codes |
| BR-07 | o8v-check search command | Validate limit > 0 or document limit=0 semantics |
| BR-08, BR-18 | o8v-render error router | Route all errors to STDERR; emit JSON error envelope when --json |
| BR-09, BR-10 | o8v-fs ls / render | Expose real count separately from capped list; fix footer arithmetic |
| BR-11, BR-12 | o8v-check ls command | Validate --stack values; warn on conflicting flag pairs |
| BR-14 | o8v-fs / read executor | Return Err on binary; render on STDERR; exit 1 |
| BR-15 | o8v-fs / read batch | Detect overlap; return error entry instead of empty |
| BR-16, BR-17 | o8v-render read output | Add kind column to text mode; unify JSON shape |
| BR-19 | o8v-render error taxonomy | Define error type enum; map all prefixes to it |
| BR-20 | o8v-cli init command | Route all init output through single stream |
| BR-21, BR-22 | docs/ai_section.txt, MCP description | Add Failure behavior section; document exit codes; specify --find/--replace N>1 |

---

## §4 Recommended Next 3 Slices

### Slice B1 — Instruction-surface failure contract (BR-21, BR-22)
Justification: Axis 3 (Failure) scored 2.33/10 across N=6 instruction-clarity runs. All 6 agents named the same root cause: no error/exit-code contract in either surface. This is the highest-leverage doc change — it unblocks Q7/Q18/Q26-Q29 in every future benchmark run. Zero code changes required. Expected Axis 3 lift: 2.33 to 6-7; composite 5.39 to ~6.5.

### Slice B2 — Uniform error routing: STDERR + JSON envelope (BR-08, BR-18, BR-19)
Justification: Three cross-cutting patterns (P-C, P-D) trace to this single structural gap. Every command that violates STDOUT/STDERR routing also violates the `--json` structured-error contract. A single error-router in o8v-render, applied to all subcommands, eliminates 5+ bugs in one slice. Slice B1 ships first so the doc contract exists before the implementation is measured.

### Slice B3 — Silent-failure surface in search (BR-03, BR-06, BR-07, BR-23)
Justification: Pattern P-A has 8 members; search owns 4 of them. The search executor silently drops permission errors, makes binary files invisible, accepts invalid limit=0, and gives no signal for outside-repo paths. An agent running `8v search` over a codebase with any restricted files receives a result set that is silently incomplete. Fix: populate files_skipped with reason codes; return exit 1 on read errors; reject limit=0.

---

## §5 Coverage Gaps

This register does NOT cover:

1. `8v check`, `8v fmt`, `8v test`, `8v build` — none swept in round 1. Error contracts, exit codes, and --json behavior unknown.
2. `8v hooks`, `8v upgrade`, `8v mcp` — not swept. Per e2e-coverage-audit-apr17, zero E2E coverage.
3. Concurrent / parallel behavior — batch read overlap found (BR-15) with 2-file overlap only. N>2 overlaps and concurrent writers untested.
4. Windows / non-macOS paths — all QA on macOS (darwin). Path separator handling on other platforms unknown.
5. Large file behavior — symbol maps on files >10K lines not tested. MCP output cap interaction with real large codebases only partially observed.
6. `--json` schema stability — BR-17 found one shape asymmetry; full JSON schema audit across all subcommands not performed.
7. `8v run` — deferred per run_deferred.md; not in scope for this sweep.

---

## 8v Feedback

Friction observed during this synthesis session:

1. **Output cap prevents single batch read of all findings.** Six markdown files (~87K chars combined) exceeded the 55,000-char MCP cap in a single `--full` batch call. Required two separate range-batched calls after a symbol-map probe (which returned nothing — markdown has no symbols). A `--budget` flag that auto-splits would remove this round-trip.

2. **Symbol map on markdown returns empty with no hint.** `8v read file.md` returned no output. No message indicating "these files have no symbols" — agents must guess whether the file is empty, the path is wrong, or symbols are absent. The empty-hint slice (shipped in round 1) helps for single-file reads; batch mode needs the same treatment.

3. **`8v write --append` fails on non-existent files with a double-prefixed error.** During this session, `8v write <path> --append "<content>"` on a file that did not yet exist returned: `error: error: file does not exist`. The double `error:` prefix (BR-01) appeared live. The correct form (`8v write <path> "<content>"`) is not surfaced in the error message.

4. **No `8v write <path> --create` flag.** Writing to a new file requires knowing whether to use `--append` (fails if absent) vs bare write (creates if absent). The distinction is not surfaced in `--help` output shown to agents. A `--create` flag that explicitly means "new file, fail if exists" would make intent unambiguous.
