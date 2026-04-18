# log/stats POC Learnings

**Date:** 2026-04-17
**Status:** POC complete, feature design (Level 1) validated, implementation design (Level 2) pending.

## Why this doc exists
The existing log/stats code is a POC, not production. It shipped without a Level 2 implementation design. An adversarial review found 4 critical + 9 serious bugs, all traceable to missing Level 2 concerns. This doc captures what the POC taught us, so the rewrite is informed.

## What the POC proved works (Level 1 validated)
- Feature shape in `docs/design/log_stats_design.md` §3 is correct: per-command aggregates, argv-shape drill, retry clusters, failure clusters, --compare agent split, percentile histograms, --since/--until windows.
- Shared single-pass aggregator feeding both `log` and `stats` is the right topology.
- ULID-based `session_id` stamped at process start, one session per CLI process / per MCP transport, is the right identity model.
- argv normalization per design §6.1 (relativize inside project, `<abs>` outside, `<tmp>`, `<str>` for quoted, basename fallback) captures real agent behavior.
- Fixed log-spaced histogram (60 buckets, 1ms–1000s) with n<5 → None sentinel is adequate for p50/p95/p99.
- Real dogfood run (30 commands) confirmed the signal shape surfaces real failures (3 write failures grouped by argv shape correctly).

## What the POC got wrong (structural, not bugs)

### 1. Ownership is split, not layered
- Percentiles computed in both `stats_histogram.rs` (via log buckets) AND `log/drill.rs` (via sort-and-index). Two algorithms on the same data → log and stats disagree.
- Warnings collected in `aggregator` then silently discarded in `log/search.rs` (`_warnings: Vec<String>`).
- Empty-window signal leaks from `stats` into `commands/mod.rs` via `eprintln!` — bypassing the warnings channel.
- No single owner for "event → session → stats." Each consumer rebuilds what it needs.

### 2. Primitives where types belong
- `session_id: String` — any String compiles, including empty; aggregator maps empty → `"ses_legacy"` at the wrong layer.
- `argv_shape: String` — no distinction from raw argv.
- Timestamps as raw `i64` / `u64` that cast across signed/unsigned boundaries: `(last - first) as u64` at `aggregator.rs:432, :472` silently wraps to u64::MAX on reversed clocks, killing cluster detection.
- `--compare: Option<String>` matched against `"agent"` — typos silently fall through to default.
- `percentile(p: f64)` — unvalidated, p=1.5 or NaN produces nonsensical output.

### 3. Render pipeline bypassed by format!
- `stats_report.rs:148` uses `r#"{{"label":{label:?}..."#` — Rust `{:?}` Debug is NOT a JSON escaper; labels with `\n`, `\`, or `"` produce malformed JSON.
- `log/drill.rs` does `format!("{:?}", caller).to_lowercase()` — Debug as stable display.
- The "no bypass architecture" rule is violated *inside* the render layer itself.

### 4. Silent fallbacks instead of errors
- `canonicalize(path).unwrap_or_else(|_| PathBuf::from(path))` at `aggregator.rs:72-75` — unresolvable path silently becomes raw path.
- `now_ms.saturating_sub(since_ms)` at `stats.rs:78` — future `since` values hidden.
- `(last - first) as u64` — reversed timestamps wrap silently.
- Empty `session_id` → `ses_legacy` mapping — no warning when it happens.
- Duplicate `CommandCompleted` for same run_id silently dropped — design required warning.

### 5. Design §3.4 JSON contract drifted during implementation
- Field names diverged: `label` (impl) vs `command` (spec); flat `p50_ms/p95_ms/p99_ms` (impl) vs nested `duration_ms:{p50,p95,p99}` (spec); `ok_pct` vs `ok_rate`; `out_per_call` vs `output_bytes_per_call_mean`; `retries` vs `retry_cluster_count`.
- Missing entire `failure_hotspots` array.
- Missing `window` / `blind_spots` / `warnings` envelope.
- Benchmark consumer (stable contract user) is broken, and nobody noticed because e2e tests assert impl field names.

### 6. Algorithm bug: retry window is total-span, not sliding window
- 3 occurrences at t=0, t=20s, t=90s span 90s > 30s window → no cluster detected, even though t=0/t=20s is a valid pair within the 30s window.
- Must be a two-pointer sliding window.

### 7. CLI session boundary = process lifetime
- Shell-loop invocations never form retry clusters — each shell-out is its own session.
- Two valid responses: (a) accept by design, MCP is the target user; (b) add cross-session retry view as a second aggregation pass.
- Not a bug, but must be acknowledged in Level 1 scope.

## Adversarial review summary (for reference)
- 4 CRITICAL findings: C1 reversed-timestamp cast (aggregator.rs:432,472), C2 JSON Debug-as-escape (stats_report.rs:148), C3 pre-epoch fmt_timestamp garbage (log_report.rs:54-92), C4 future-since saturating_sub (stats.rs:78).
- 9 SERIOUS findings: stringly-typed --compare, duplicate percentile algorithm in drill, hardcoded MIN_SAMPLES=5, silent warning discard in search, canonicalize silent fallback, unvalidated percentile, missing counterexample tests for reversed timestamps / invalid --compare / label-with-newlines-JSON.
- Verdict: do not merge.

## What Level 2 must answer
1. What is the type system boundary? (newtypes over primitives — SessionId, ArgvShape, TimestampMs, CommandName, CompareMode)
2. Who owns aggregation? (single producer; log and stats are read-only views)
3. How do warnings flow? (typed Warning enum carried through every layer, never dropped)
4. What is the single serialization path? (serde derive + renames matching §3.4 byte-for-byte; zero format! for JSON)
5. What is the test strategy? (golden JSON from design §3.4; counterexample tests that fail on current code before passing)
6. How is the sliding-window algorithm implemented? (two-pointer over sorted timestamps)
7. What is the error policy? (no silent fallback — every `unwrap_or`, `as u64`, `saturating_sub` becomes an explicit Warning or Error)

## POC disposition
- Keep the code running until Level 2 rewrite replaces it layer by layer.
- No new features on POC code. Freeze.
- Every layer replaced → delete the corresponding POC code, no parallel paths.
