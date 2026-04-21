# Test Audit — 2026-04-18

Scope: entire 8v workspace test suite. Audit depth: Phase A–F.
Companion: `test-patterns-and-mistakes-2026-04-18.md` (patterns only, no per-file catalog).

---

## Phase A — Test Catalog

| File | Test | Claimed Coverage | Real Assertion | Layer | Survives refactor? | Boundary? | Fixture |
|---|---|---|---|---|---|---|---|
| dispatch.rs | build_context_has_event_bus | EventBus wired | not None | unit | yes | no | inline |
| dispatch.rs | dispatch_emits_lifecycle_events | Started+Completed emitted | event name strings only | unit | fragile | no | RecordingSubscriber |
| dispatch.rs | dispatch_emits_completed_on_panic | RAII on panic | event name "CommandCompleted" | unit | fragile | no | RecordingSubscriber |
| dispatch.rs | dispatch_emits_completed_on_failure | RAII on error | event name only | unit | fragile | no | RecordingSubscriber |
| sliding_window.rs | sliding_window_splits_long_timeline | splits long runs | correct split at 1_000ms | unit | yes | NO — 1_000ms only | inline |
| sliding_window.rs | sliding_window_gap_breaks_run | gap > window breaks | 5 events, 5_000ms window | unit | yes | NO — no 30_000ms | inline |
| sliding_window.rs | single_event_does_not_cluster | no false cluster | len==1 | unit | yes | trivial | inline |
| sliding_window.rs | reversed_timestamps_break_run | reversed ts | break detected | unit | yes | no | inline |
| sliding_window.rs | reversed_timestamp_in_sliding_windows_breaks_run_not_overflows | no overflow | no panic | unit | yes | no | inline |
| redact.rs | redacts_openai_api_key | sk- redaction | pattern matched | unit | yes | happy path | inline |
| redact.rs | redacts_jwt | JWT redaction | pattern matched | unit | yes | happy path | inline |
| redact.rs | redacts_url_with_credentials | URL redaction | pattern matched | unit | yes | happy path | inline |
| counterexamples_hook_redaction.rs | api_key_uppercase_prefix_not_redacted_bug | SK- uppercase | FAILS (bug) | unit | n/a | boundary | inline |
| counterexamples_hook_redaction.rs | api_key_underscore_separator_not_redacted_bug | sk_ underscore | FAILS (bug) | unit | n/a | boundary | inline |
| counterexamples_hook_redaction.rs | jwt_with_base64_padding_not_redacted_bug | JWT padding | FAILS (bug) | unit | n/a | boundary | inline |
| counterexamples_hook_redaction.rs | url_percent_encoded_colon_not_redacted_bug | %3A in URL | FAILS (bug) | unit | n/a | boundary | inline |
| counterexamples_basic_ops.rs | gap5_retry_cluster_detected_for_rapid_repeated_commands | retry detection | cluster detected at 700ms | integration | yes | NO — no 29_000ms | pair_with_output_bytes |
| counterexamples_basic_ops.rs | output_bytes_zero_not_counted_bug (MISSING) | output_bytes=0 pipeline | NOT PRESENT | — | n/a | critical gap | — |
| e2e_stats_contract.rs | contract_4_output_bytes_per_call_mean_is_number | opc computed | opc >= 0.0 | E2E | fragile | NO — passes at 0 | fresh() hardcoded 512 |
| counterexamples_stats_v2.rs | various aggregation tests | aggregation correctness | mean/p95/count | integration | yes | partial | ndjson_pair() hardcoded 512 |
| e2e_hook.rs | hook_redacts_sk_lowercase | hook E2E sk- | redacted in log | E2E | yes | lowercase only | TempDir+_8V_HOME |
| e2e_*.rs (search) | (none) | search E2E | NOT PRESENT | — | n/a | missing | — |

---

## Phase B — Contract-to-Coverage Maps

### B1: output_bytes pipeline
Contract: `CommandCompleted.output_bytes` = actual bytes written by command.

| Stage | Covered? | Evidence |
|---|---|---|
| dispatch.rs emits correct value | NO | line 57 passes literal `0` |
| RecordingSubscriber reads payload | NO | dispatch.rs:256-277 records event name only |
| fresh() fixture reflects reality | NO | e2e_stats_contract.rs:79 hardcodes 512 |
| contract_4 catches zero | NO | line 347-372 asserts `>= 0.0`; 0.0 passes |
| Any round-trip test | NO | none exist |

All five pipeline stages uncovered. Bug ships invisibly.

### B2: Retry-cluster consecutive-gap algorithm
Contract: events with gap > threshold start a new cluster.

| Test scenario | Covered? | Evidence |
|---|---|---|
| Gap at 30_000ms boundary | NO | all tests use 1_000ms or 5_000ms |
| Three events at 0/29_000/58_000ms | NO | not present anywhere |
| Consecutive-gap vs total-span indistinguishable | YES (bug) | gap5 at 700ms spacing cannot distinguish |

### B3: Hook redaction patterns
Contract: all credential patterns redacted before log write.

| Pattern | happy-path test | boundary test | E2E |
|---|---|---|---|
| sk- lowercase | yes (redact.rs) | yes (e2e_hook.rs) | yes |
| SK- uppercase | no | FAILS (counterexample) | no |
| sk_ underscore | no | FAILS (counterexample) | no |
| JWT with padding | no | FAILS (counterexample) | no |
| %3A percent-encoded | no | FAILS (counterexample) | no |

4 of 5 boundary patterns have no passing tests.

### B4: CommandGuard RAII guarantees
Contract: CommandCompleted always emitted, payload fields correct.

| Guarantee | Covered? | Evidence |
|---|---|---|
| Completed on success | event name | dispatch.rs:287-314 |
| Completed on panic | event name | dispatch.rs:315-352 |
| Completed on error | event name | dispatch.rs:353-401 |
| output_bytes in payload | NO | RecordingSubscriber never deserializes |
| success=false on error | NO | never asserted |
| duration_ms > 0 | NO | never asserted |

### B5: E2E subcommand coverage
| Subcommand | Dedicated E2E file | Status |
|---|---|---|
| read | e2e_read.rs | covered |
| write | e2e_write.rs | covered |
| check | e2e_check.rs | covered |
| build | e2e_build.rs | covered |
| ls | e2e_ls.rs | covered |
| stats | e2e_stats_contract.rs | partial (payload blind) |
| search | none | ZERO E2E |
| hooks | e2e_hook.rs | smoke only (1 pattern) |
| upgrade | none | ZERO E2E |
| mcp | none | ZERO E2E |
| fmt | e2e_fmt.rs | covered |
| init | e2e_init.rs | covered |

3 subcommands have zero E2E coverage; 2 more have smoke-only.

---

## Phase C — Structural Critique

**C1: Fixture infrastructure prevents bug detection.**
`fresh()` (e2e_stats_contract.rs:44-91), `make_completed()` (counterexamples_stats_v2.rs:47-51), and `ndjson_pair()` (counterexamples_stats_v2.rs:73-111) all hardcode `output_bytes: 512`. A system-wide defect (dispatch.rs:57 emits 0) is invisible to every test in the suite. The fixture shape matches what the code *should* produce, not what it *does* produce.

**C2: Contract tests assert properties, not contracts.**
`contract_4_output_bytes_per_call_mean_is_number` (e2e_stats_contract.rs:347-372) asserts `opc >= 0.0`. This is a type-level assertion, not a contract. The correct contract is `opc > 0.0` (when output was produced). The current assertion is satisfied by the bug value.

**C3: Algorithm tests are altitude-mismatched.**
All 5 sliding_window tests use 1_000ms or 5_000ms thresholds. The production retry-cluster window is 30_000ms. No test presents events near the production boundary. The consecutive-gap algorithm (lines 28-36) cannot be distinguished from a total-span algorithm using any existing test.

**C4: Counterexample suite is structurally correct but operationally noisy.**
`counterexamples_basic_ops.rs` uses the correct parameterized `pair_with_output_bytes()` helper. `counterexamples_stats_v2.rs` has the right structure. However, 4 permanently-failing tests in `counterexamples_hook_redaction.rs` are not marked `#[ignore]`, causing `cargo test --workspace` to always report 4 failures. This trains developers to accept a non-green baseline and dilutes signal.

**C5: Dispatch layer tests are too shallow.**
`RecordingSubscriber` (dispatch.rs:256-277) records only the `"event"` string key. It never deserializes the event payload. Three correctness-critical fields (`output_bytes`, `success`, `duration_ms`) are entirely unverified at the dispatch layer.

**C6: Test isolation is excellent.**
All E2E tests uniformly use `TempDir` + `_8V_HOME` env override. No cross-test state observed. This is a genuine strength.

**C7: Regression test discipline is inconsistent.**
Regressions exist for orphan runs (F5), run_id collision, and some aggregator edge cases. No regression test exists for `output_bytes=0` (the actively shipped bug) or for the consecutive-gap boundary condition (the known behavioral bug). Both bugs were known before this audit.

---

## Phase D — Root Causes

### D1: Fixture pollution (highest leverage)
**Evidence**: `fresh()` hardcodes 512 at e2e_stats_contract.rs:79; `make_completed()` hardcodes 512 at counterexamples_stats_v2.rs:47-51; `ndjson_pair()` hardcodes 512 at counterexamples_stats_v2.rs:101. Three files, 50+ tests. All fixtures agree on the same wrong value. The dispatch bug (line 57 emits 0) is structurally invisible to the entire suite.

**Impact**: Any bug in the output_bytes pipeline — emission, storage, aggregation, rendering — is undetectable until it reaches production.

### D2: Tests at wrong altitude (algorithm coverage)
**Evidence**: `sliding_window.rs` tests use 1_000ms and 5_000ms windows (lines 70-152). Production uses 30_000ms. `gap5` in counterexamples_basic_ops.rs uses 700ms spacing. No test probes the 29_000ms consecutive-gap boundary.

**Impact**: The algorithm could be replaced with total-span semantics and all tests would still pass. The behavioral bug (events at 0/29s/58s clustering as one) is undetectable.

### D3: Permanent failure baseline (signal dilution)
**Evidence**: 4 `_bug`-suffix tests in counterexamples_hook_redaction.rs always fail (lines 53, 70, 109, 196). Not marked `#[ignore]`. `cargo test --workspace` = 359 pass + 4 fail.

**Impact**: Developers learn to mentally subtract 4 from the failure count. When a new failure appears, it may be dismissed as "probably another known one." Green baseline = unambiguous signal. This repo has no green baseline.

---

## Phase E — Bounded Recommendations

### Category 1: Fix false-green tests (do first)

**E1** — Write a round-trip regression for output_bytes.
Add a test that runs the real CLI dispatch path and reads back the stored event, asserting `output_bytes > 0`. This is the only test that would have caught dispatch.rs:57.

**E2** — Parameterize `fresh()` to accept output_bytes.
Change signature to `fresh(output_bytes: u64)`. Update all callers. This unblocks E1 and prevents future fixture pollution.

**E3** — Add `gap7` boundary test to counterexamples_basic_ops.rs.
Three events at timestamps 0ms / 29_000ms / 58_000ms. With consecutive-gap algorithm they form one cluster; with total-span they form two. This is the only test that distinguishes the two algorithms.

**E4** — Add a 30_000ms window test to sliding_window.rs.
Mirror the production configuration. The existing tests at 1_000ms do not protect the production behavior.

### Category 2: Reduce permanent noise

**E5** — Mark 4 hook redaction bug tests `#[ignore]`.
`counterexamples_hook_redaction.rs` lines 53, 70, 109, 196. Add `#[ignore = "known bug: pattern not yet implemented"]`. Restore green baseline.

**E6** — Extend dispatch tests to deserialize payload.
Replace `RecordingSubscriber` string-only recording with a subscriber that deserializes to `serde_json::Value`. Assert `output_bytes`, `success`, and `duration_ms` fields in all four dispatch tests.

### Category 3: Fill coverage gaps

**E7** — Add `e2e_search.rs`.
Minimum: one test for a matching search, one for zero results, one for extension filter. Search has zero E2E coverage.

**E8** — Add payload assertions to `e2e_stats_contract.rs`.
`contract_4` must assert `opc > 0.0`, not `opc >= 0.0`. Document the distinction.

**E9** — Add `e2e_mcp.rs` smoke test.
Even a single "mcp registers without error" test catches startup regressions. Currently zero MCP E2E coverage.

**E10** — Add CommandGuard drop test with payload verification.
One test that panics inside dispatch and then reads back the stored event from the NDJSON file, asserting `success=false` and `duration_ms > 0`. The current test only checks event names.

---

## Phase F — Audit Summary

**Three bugs that tests cannot currently catch:**

1. `output_bytes=0` (dispatch.rs:57) — invisible because all fixtures hardcode 512 and contract_4 asserts `>= 0.0`.
2. Retry-cluster consecutive-gap boundary (sliding_window.rs:28-36) — invisible because no test uses 30_000ms window or 29_000ms gaps.
3. Hook redaction for 4 patterns — documented in failing counterexamples but never fixed; causes non-green baseline.

**One structural strength:**
Test isolation (TempDir + `_8V_HOME`) is uniform and correct across all E2E tests.

**Highest-leverage action:**
E2 (parameterize `fresh()`) + E1 (round-trip regression) in one PR. This alone would have caught the shipped bug before it reached production and costs ~2 hours.

**Lowest-risk action to improve baseline:**
E5 (`#[ignore]` on 4 failing tests). Zero behavior change, unambiguous green baseline restored, <10 minutes.

---

*Audit produced: 2026-04-18. Author: audit agent. Do not edit without re-running the catalog from source.*
