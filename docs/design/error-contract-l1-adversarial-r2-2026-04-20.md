# Adversarial Review — Round 2
**Date:** 2026-04-20  
**Scope:** 7 Level-1 design drafts revised after round-1 (19 blockers)  
**Reviewer:** Adversarial read-only pass  
**Verdict:** 0 READY / 7 REVISE (3 docs unblocked pending 3 founder decisions)

---

## §1 — Round-1 Blocker Verdicts (19 total)

### slice-b2-decomposition.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| B2-D-1 | B2a→B2c dependency unproven — no explanation why exit codes need stderr channel first | **FIXED** — line 86 explains CE-2 discriminant: `exit 1 + stderr empty` vs `exit 1 + stderr non-empty` requires stderr channel before exit code contract can be set |
| B2-D-2 | Capital-E handoff undocumented — doc silent on what happens if B2d not approved | **PARTIALLY FIXED** — both handoff paths now documented (absorb if B2d approved; pick A/B/C from standalone doc if not); actual founder decision still pending |
| B2-D-3 | No named failing tests per sub-slice | **FIXED** — 6+6+6+5 named tests across B2a–B2d |

### slice-b2a-counterexamples.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| B2A-1 | A6 framed as L1 gate answer — "surveyable at L2" is not a contract decision | **PARTIALLY FIXED** — reclassified to "pre-L2 entry condition"; A2 still BLOCKED ON FOUNDER (option A=exclude JSON paths or B=ship B2a+B2b together); doc's own verdict (line 84): "Not yet ready for Level 2" |
| B2A-2 | A2 (JSON paths on stderr) unresolved — two conflicting behaviors, no decision | **PARTIALLY FIXED** — options A/B named explicitly; remains BLOCKED ON FOUNDER; A2 is the most critical open tension per the doc itself |
| B2A-3 | A7/A8 appear in gate without counterexample bodies | **FIXED** — both have "Pre-L2 required" survey notes clarifying they are entry conditions, not L1 answers |

### slice-b3-search-silent-failure.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| B3-1 | BR-39 conflict — design closes it but register §4 note says explicitly out of scope | **PARTIALLY FIXED** — B3 scope is now clear (BR-39 out of scope in this doc, line 82); register v2 §4 row still shows B3 closing 3 bugs while note excludes BR-39; count/scope mismatch unreconciled |
| B3-2 | No backward-compat statement for new `files_skipped_by_reason` JSON field | **FIXED** — additive field noted at line 16; no current consumers confirmed (2026-04-20 survey) |

### slice-c1-init-hooks-correctness.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| C1-1 | H-1 fix assigned exit 2 to runtime condition (malformed stdin) — violates error-contract §2.1 | **FIXED** — explicitly exit 1 at line 13; cites §2.1 at line 26 ("exit 2 is reserved exclusively for clap parse failures") |
| C1-2 | No test for I-3 (PATH anchor) fix | **FIXED** — `init_installed_hook_uses_absolute_8v_path_not_bare_name` added at line 49 |
| C1-3 | BR-28 orphaned — doc claimed to close it but register lists it as unowned | **NOT FIXED** — BR-28 silently removed from the doc with no explanation; register v2 §5 still lists BR-28 as orphaned with "park for Phase 0 round 2" note; ownership gap unresolved |

### slice-c2-upgrade-contract.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| C2-1 | JSON shape deferred — `status` field shape unspecified, L2 cannot start | **PARTIALLY FIXED** — two options named at line 27 (A: `status` enum; B: `upgraded: false + current: true`); remains BLOCKED ON FOUNDER |
| C2-2 | No test for "field absent/null when network error" behavior | **FIXED** — `upgrade_json_field_absent_when_network_error` added at line 52 |
| C2-3 | Offline exit code unspecified — design silent on `upgrade` when network is unreachable | **FIXED** — DNS failure → non-zero covered at line 16; counterexample 4 defaults to exit 1 for offline cache scenario |

### slice-c3-write-semantics.md

| # | Round-1 Blocker | Verdict |
|---|---|---|
| C3-1 | Escape depth analysis wrong — two-layer shell+8v escape treated as one | **FIXED** — counterexample 1 now correctly traces: `\\n` → shell delivers `\n` → 8v produces newline; `\\\\n` → shell delivers `\\n` → 8v produces literal `\n` |
| C3-2 | AF-1 test presupposes L2 branch decision (behavior change vs help-text only) | **FIXED** — fix explicitly scoped to help-text correction only; no behavior change, no L2 branch needed |
| C3-3 | BR-38 orphaned — doc claimed to close it but register lists it under C3 | **NOT FIXED** — BR-38 silently removed from the doc; register v2 §5 lists BR-38 under C3 with "symlink error text wrong for non-symlink"; C3 still claims to close BR-38 in register §4 but the design doc no longer mentions it; ownership undocumented |

### write-capital-e-prefix-superseded.md (findings/)

| # | Round-1 Blocker | Verdict |
|---|---|---|
| CAP-1 | No A/B/C decision recorded — doc presents options but makes no recommendation | **PARTIALLY FIXED** — Option C (strip at inner source) stated as recommendation at line ~40; but founder decision on absorb-into-B2d vs ship-standalone still pending; cannot implement either path without this gate |
| CAP-2 | B2d absorption handoff undocumented — doc silent on what supersedes it | **FIXED** — gate at line 55: "If B2d is approved, this doc is superseded"; cross-referenced in slice-b2-decomposition §Register reconciliation |

---

## §2 — Summary

| Verdict | Count |
|---|---|
| FIXED | 11 |
| PARTIALLY FIXED | 6 |
| NOT FIXED | 2 |
| **Total** | **19** |

---

## §3 — New Blockers (introduced or missed)

**NEW-1 (slice-b3, line 17): `--limit 0` exit-code violation**  
The draft specifies `--limit 0` is rejected at parse time with exit 2. Error-contract §2.1 reserves exit 2 exclusively for clap parse failures. If this validation is a post-parse custom check (not a native clap `value_parser` range constraint), it must emit exit 1. The draft does not specify which mechanism is used. This must be resolved in L1 before L2 can implement it.

**NEW-2 (slice-b2a, line 84): Doc self-declares not ready**  
The doc's own Verdict section explicitly states "Not yet ready for Level 2. A2 is the most important open tension." A doc that fails its own gate cannot advance. A2 (BLOCKED ON FOUNDER: option A or B for JSON paths on stderr) must be resolved before this doc can be marked READY. This is not a classification dispute — the doc authored the verdict itself.

**NEW-3 (cross-doc: b2a ↔ b2-decomposition): A2 option B invalidates recommended slice order**  
If the founder picks A2 option B (ship B2a + B2b together), the recommended execution order in slice-b2-decomposition §4 (B2a → B2c → B2b → B2d) becomes invalid: B2b can no longer trail B2a. This cross-document dependency is unresolved. The slice order must be reconsidered after A2 is decided.

---

## §4 — Founder Decisions Blocking Progress

Three decisions gate all remaining PARTIALLY FIXED items:

1. **A2 (B2a):** JSON paths on stderr — option A (exclude them) or option B (ship B2a+B2b together). Resolves NEW-2 and NEW-3. Unblocks b2a-counterexamples and the slice order in b2-decomposition.
2. **Capital-E / B2d gate:** Absorb write-capital-e-prefix-superseded (findings/) into B2d (doc superseded) or ship standalone (pick A/B/C). Resolves CAP-1 and B2-D-2. Documented in both docs, waiting for gate decision.
3. **C2 JSON shape:** Option A (`status` enum) or option B (`upgraded: false + current: true`). Resolves C2-1. Unblocks slice-c2 Level 2 start.

---

## §5 — git status --short (snapshot)

```
 M AGENTS.md
 M CLAUDE.md
 M o8v-core/src/render/read_report.rs
 M o8v-testkit/Cargo.toml
 M o8v-testkit/src/benchmark/claude.rs
 M o8v-testkit/src/benchmark/experiment.rs
 M o8v-testkit/src/benchmark/mod.rs
 M o8v-testkit/src/benchmark/pipeline.rs
 M o8v-testkit/src/benchmark/report.rs
 M o8v-testkit/src/benchmark/store.rs
 M o8v-testkit/src/benchmark/types.rs
 M o8v/src/aggregator/sliding_window.rs
 M o8v/src/commands/read.rs
 M o8v/src/init/ai_section.txt
 M o8v/src/mcp/handler.rs
 M o8v/src/mcp/instructions.txt
 M o8v/tests/agent_benchmark.rs
 M o8v/tests/counterexamples_hook_redaction.rs
 M o8v/tests/e2e_cli.rs
 M o8v/tests/e2e_stats.rs
 M o8v/tests/e2e_stats_contract.rs
 M o8v/tests/e2e_stats_session.rs
 M o8v/tests/mcp_e2e.rs
 M o8v/tests/regression_orphan_session_filter.rs
?? docs/design/review-round2-2026-04-20.md
[... + 70 untracked docs/findings/design files, omitted for brevity]
```
