# log/stats — Implementation Design (Level 2)

**Date:** 2026-04-17
**Precedence:** supersedes the current POC implementation. Level 1 (feature design) unchanged — see `log_stats_design.md`. Learnings this design responds to: `log-stats-design-learnings.md`.

**Principle in one line:** every value that can be wrong at runtime must be wrong at compile time, or must become a typed `Warning` that travels to the user. Nothing silent. Nothing duplicated. One path per concern.

---

## 1. Module map (after rewrite)

```
o8v-core/src/events/lifecycle.rs      — CommandStarted / CommandCompleted (types only)
o8v-core/src/types/                   — NEW: typed boundary (SessionId, ArgvShape, TimestampMs, CommandName, Warning)
o8v-core/src/render/log_report.rs     — pure data model; derives Serialize
o8v-core/src/render/stats_report.rs   — pure data model; derives Serialize; matches §3.4 byte-for-byte

o8v/src/aggregator/                   — NEW folder:
  mod.rs                              — public API: aggregate_events(&[Event], Config) -> Aggregate
  argv_shape.rs                       — normalization (§6.1); returns ArgvShape
  histogram.rs                        — single percentile implementation (used by log AND stats)
  clusters.rs                         — two-pointer sliding window over sorted timestamps
  warnings.rs                         — Warning sink (Vec<Warning>, never dropped)

o8v/src/commands/stats.rs             — read-only view over Aggregate → StatsReport
o8v/src/commands/log/mod.rs           — read-only view over Aggregate → LogReport
o8v/src/commands/log/drill.rs         — uses aggregator histograms; no percentile code of its own
o8v/src/commands/log/search.rs        — carries warnings through
```

Every module has one responsibility. `drill.rs` and `stats.rs` consume `Aggregate`; neither computes stats of its own. Any future consumer does the same.

---

## 2. Typed boundary (Layer 1)

All primitives that carry invariants become newtypes. Construction enforces invariants; consumers cannot bypass.

```rust
// o8v-core/src/types/session_id.rs
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);                          // format: "ses_<ULID>"
impl SessionId {
    pub fn new() -> Self { /* generates ses_<ULID> */ }
    pub fn try_from_raw(s: String) -> Result<Self, InvalidSessionId> { /* validates prefix+ulid */ }
}

// o8v-core/src/types/timestamp.rs
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMs(i64);                           // always i64; never u64
impl TimestampMs {
    pub fn now() -> Self { /* SystemTime, panics on pre-epoch */ }
    pub fn checked_sub(self, rhs: Self) -> Option<DurationMs> { ... }   // None if negative
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct DurationMs(u64);                            // constructed only via checked_sub / from_positive_i64

// o8v-core/src/types/argv_shape.rs
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArgvShape(String);                          // output of normalize(argv)

// o8v-core/src/types/command_name.rs
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum CommandName { Read, Write, Search, Check, Fmt, Ls, Build, Test, Init, Hooks, Upgrade, Mcp, Log, Stats }
// Display for stable string form; NEVER Debug-as-display.

// o8v/src/commands/stats.rs
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CompareMode { Agent }                         // typo becomes clap parse error, not silent default
```

**What this kills by construction:**
- C1 (reversed timestamp `as u64`): `TimestampMs::checked_sub → Option<DurationMs>`. Cast is gone. Call sites must handle `None` (→ Warning).
- S1 (stringly-typed `--compare`): clap ValueEnum rejects typos at parse time.
- S6 (unvalidated percentile): `Percentile(f64)` newtype with `new(p: f64) -> Result<Self, OutOfRange>` in histogram.rs.
- Empty-`SessionId`-mapped-to-`ses_legacy`: `SessionId::try_from_raw` rejects empty; caller must emit a `Warning::EmptySessionId` and either skip or bucket into a typed sentinel (see §4).

---

## 3. Single aggregation owner (Layer 2)

```rust
// o8v/src/aggregator/mod.rs
pub struct Config {
    pub retry_window: DurationMs,
    pub strict: bool,
}

pub struct Aggregate {
    pub sessions: Vec<SessionAggregate>,
    pub by_command: HashMap<CommandName, CommandStats>,       // global, across sessions
    pub by_shape:   HashMap<(CommandName, ArgvShape), CommandStats>,
    pub by_agent:   HashMap<Option<AgentId>, CommandStats>,
    pub warnings:   Vec<Warning>,
}

pub struct SessionAggregate {
    pub session_id: SessionId,
    pub commands: Vec<CommandRecord>,
    pub retry_clusters: Vec<Cluster>,                          // sliding-window
    pub failure_clusters: Vec<Cluster>,
}

pub struct CommandStats {
    pub n: u64,
    pub ok: u64,
    pub histogram: Histogram,                                  // ONE histogram impl
    pub output_bytes_total: u64,
}

pub fn aggregate_events(events: &[Event], cfg: &Config) -> Aggregate { /* single pass */ }
```

**Rules:**
- `stats::run()` and `log::*::run()` call `aggregate_events` once, then project into their report.
- No percentile code outside `aggregator::histogram`. `drill.rs` reads `CommandStats::histogram.percentile(p)`.
- `Aggregate::warnings` is the only warning collector. Every layer appends, none drops.

---

## 4. Typed Warning flow (Layer 3)

```rust
// o8v-core/src/types/warning.rs
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    CanonicalizeFailed { path: String, reason: String },
    DuplicateCompleted { run_id: String },
    ReversedTimestamps { session: SessionId, earlier: TimestampMs, later: TimestampMs },
    EmptySessionId { at: TimestampMs },
    FutureSince { since_ms: i64, now_ms: i64 },
    MalformedEventLine { line_no: u64, reason: String },
    OrphanCompleted { run_id: String },
    OrphanStarted { run_id: String },
    NormalizerBasenameFallback { path: String, reason: String },
    PercentileOutOfRange { p: f64 },
}
```

**Flow:**
1. Aggregator collects `Warning`s into `Aggregate::warnings`.
2. Reports embed them: every top-level report has `warnings: Vec<Warning>`.
3. `render_json` serializes the enum with its tag — stable contract.
4. `render_human` prints `⚠ warning: <kind>: <summary>` lines *after* the main table.
5. Dispatch layer never touches warnings. `eprintln!` forbidden outside the renderer.

**Empty-window signal:** becomes `Aggregate { sessions: [], warnings: [] }` + renderer emits `no matching events in window <w>` and returns exit code 2. No leak to dispatch.

---

## 5. Single JSON serialization path (Layer 4)

```rust
// o8v-core/src/render/stats_report.rs

/// Report mode. Serialises as `"table"`, `"drill"`, or `"by_agent"`.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Table,
    Drill,
    ByAgent,
}

/// Which field the `label` column in each `StatsRow` contains.
/// Serialises as `"command"`, `"argv_shape"`, or `"agent"`.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKey {
    Command,
    ArgvShape,
    Agent,
}

/// JSON envelope. `shape` is present only when `kind = Drill`.
/// `warnings` and `failure_hotspots` are always present (may be empty arrays).
///
/// Example (table mode):
/// { "kind":"table", "label_key":"command", "rows":[...],
///   "warnings":[], "failure_hotspots":[...] }
#[derive(Serialize)]
pub struct StatsReport {
    pub kind: ReportKind,                     // table | drill | by_agent
    pub label_key: LabelKey,                  // command | argv_shape | agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,                // Some(_) only when kind = drill
    pub rows: Vec<StatsRow>,
    pub warnings: Vec<Warning>,
    pub failure_hotspots: Vec<FailureHotspot>,
}

/// One row in the stats table. `label` is polymorphic; its meaning is
/// determined by `StatsReport::label_key`. Serialised field names
/// match the JSON wire contract (§3.4 + §10).
#[derive(Serialize)]
pub struct StatsRow {
    pub label: String,                                // polymorphic; meaning determined by label_key
    pub n: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<DurationStats>,           // None when n < 5
    pub ok_rate: Option<f64>,                         // 0.0..=1.0, None when no completed records
    pub output_bytes_per_call_mean: Option<f64>,      // None when no completed records
    pub retry_cluster_count: u64,
}

/// Latency percentiles in milliseconds. Present only when n ≥ MIN_SAMPLES_FOR_PERCENTILE.
#[derive(Serialize)]
pub struct DurationStats {
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
}

/// A (command, argv_shape) pair that accumulated repeated failures — the top
/// path-argument and its occurrence count surface the most impactful target.
#[derive(Serialize)]
pub struct FailureHotspot {
    pub command: String,
    pub argv_shape: String,
    pub count: u64,                  // total failures in window, cross-session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_path: Option<String>,    // most frequent first-path-like argv token
    pub top_path_count: u64,         // occurrences of top_path (0 when top_path is None)
}
```

**Rules:**
- JSON output is exactly `serde_json::to_string_pretty(&report)`. Zero `format!`.
- Golden JSON fixtures live in `o8v/tests/fixtures/stats_report_*.json`, extracted from design §3.4 verbatim.
- Every renamed field has a doc comment citing the design section.
- `render_plain` and `render_human` are separate fns that take the same typed report — no JSON involved.

---

## 6. Sliding-window cluster algorithm (Layer 5)

```rust
// o8v/src/aggregator/clusters.rs
pub fn detect_clusters<P>(
    events: &[(TimestampMs, ArgvShape, CommandName)],
    window: DurationMs,
    predicate: P,
) -> Vec<Cluster>
where P: Fn(&(TimestampMs, ArgvShape, CommandName)) -> bool
{
    // 1. Group by (CommandName, ArgvShape).
    // 2. For each group, sort by TimestampMs ASC.
    // 3. Two pointers l, r: advance r; while events[r].ts - events[l].ts > window, advance l.
    // 4. Every time (r - l + 1) >= 2 AND predicate, record a cluster starting at l (dedup overlapping).
}
```

**Regression fixture:** t=0, 20s, 90s with 30s window → cluster at [0, 20s] detected; [20s, 90s] not; [0, 90s] not. Unit test asserts both positive and negative.

---

## 7. Error policy (what replaces silent fallbacks)

| Current silent fallback                               | Replacement                                            |
|-------------------------------------------------------|--------------------------------------------------------|
| `canonicalize(p).unwrap_or(p)`                        | `Warning::CanonicalizeFailed` + keep raw path in shape |
| `(last - first) as u64`                               | `TimestampMs::checked_sub` → `None` → `Warning::ReversedTimestamps` + skip cluster |
| `now.saturating_sub(since)`                           | `since > now` → `CommandError::Execution("future since: ...")` before aggregation |
| empty `session_id` → `"ses_legacy"`                   | `Warning::EmptySessionId` + bucket into `SessionId::sentinel_legacy()` (explicit typed sentinel) |
| duplicate `CommandCompleted` dropped silently         | `Warning::DuplicateCompleted { run_id }` + keep first |
| `{label:?}` as JSON escape                            | `#[derive(Serialize)]` only |
| `format!("{:?}", caller)`                             | `Caller: Display` impl, hand-written stable form |
| `_warnings: Vec<String>` in `search.rs`               | `SearchResults { warnings: Vec<Warning> }` |
| hardcoded `MIN_SAMPLES = 5`                           | `pub const MIN_SAMPLES_FOR_PERCENTILE: u64 = 5;` exported from `histogram` |
| `fmt_timestamp(negative)` → garbage date              | `fmt_timestamp` takes `TimestampMs`; negative returns `"(invalid timestamp)"` sentinel |

Invariant for PR review: grep the diff for `unwrap_or`, `as u64`, `as i64`, `saturating_`. Every remaining occurrence must be justified by a comment citing the reason.

---

## 8. Test strategy

### 8.1 Golden JSON contract tests
- `o8v/tests/fixtures/stats_report_v1.json` — exact shape from design §3.4, minimal non-empty population.
- Test: build an Aggregate programmatically, render JSON, assert bytes-equal to golden (after pretty normalization).
- Any drift from §3.4 → test fails.

### 8.2 Counterexample tests (fail-first)
Each one written as a failing test against current POC code BEFORE the fix. Captured as regression.
1. `reversed_timestamps_still_cluster` — within-window pair detected even when events arrive out-of-order.
2. `label_with_newlines_roundtrips_json` — `serde_json::from_str` succeeds on the output.
3. `invalid_compare_rejected` — `--compare bogus` exits non-zero with clap error.
4. `percentile_out_of_range_errors` — `Percentile::new(1.5)` returns Err.
5. `negative_timestamp_renders_sentinel` — `fmt_timestamp(TimestampMs(-1))` = `"(invalid timestamp)"`.
6. `future_since_errors` — `--since 99999d` returns `CommandError::Execution`.
7. `duplicate_completed_emits_warning` — two `CommandCompleted` for same run_id → one `Warning::DuplicateCompleted` in report.
8. `canonicalize_failure_emits_warning` — unresolvable path → `Warning::CanonicalizeFailed`.
9. `sliding_window_triple_gap` — events at t=0,20s,90s with 30s window detect [0,20s] only.
10. `empty_session_id_emits_warning` — legacy events → `Warning::EmptySessionId` + routed to sentinel bucket.

### 8.3 Existing e2e scenarios (rewrite)
- Rewrite `e2e_stats.rs` and `e2e_log.rs` assertions against design §3.4 field names. Current tests asserting `label`, `p50_ms`, `ok_pct` are lying and pass anyway — delete and rewrite.
- Add missing scenarios listed in tasks: `session_id_stamped_cli`, `session_id_stamped_mcp`, `malformed_line_skipped`, `drill_shows_per_cmd_p95`, `retry_cluster_detected`, `orphan_started_counted`, `argv_present_regression`.

### 8.4 Gates
- `cargo test --workspace` green.
- `cargo clippy --workspace -- -D warnings` green.
- `8v check .` green (rule 10).
- Golden JSON test green.
- Every counterexample test passes.

---

## 9. Execution order (layer by layer, no parallel code)

1. Task #2 — types module. Replace primitive fields everywhere. Compile green.
2. Task #3 — aggregator folder. Move histogram + clusters there. Delete drill.rs percentile. Compile green. Existing tests still pass (with renames).
3. Task #4 — Warning enum. Wire through every layer. Remove eprintln! from dispatch. Compile green.
4. Task #5 — stats_report.rs + log_report.rs rewrite with serde derives. Delete `format!` JSON code. Golden JSON test goes green.
5. Task #6 — sliding-window clusters. Sliding-window unit test goes green.
6. Task #7 — e2e rewrite against §3.4.
7. Task #8 — counterexample tests.
8. Task #9 — final gates.

Each task ends with `cargo test --workspace && cargo clippy --workspace -- -D warnings` green before the next begins.

---

## 10. Self-review corrections (2026-04-17, pre-execution)

Against `stats-command.md` §3.4 and §4:

1. **Retry clusters are cross-session per spec §4** ("cross-session retry clusters", "applied per-day — retries never span midnight"). My §3 put `retry_clusters` under `SessionAggregate`. Correction: clusters live on `Aggregate`, grouped by `(day, CommandName, ArgvShape)`. `SessionAggregate` keeps only per-session `commands`.
2. **`failure_hotspots` needs `top_path` / `top_path_count`** per §3.4 example. `CommandStats` must track a `PathFrequency` map over the first path-like argv token (bounded cardinality).
3. **JSON envelope** — `{ kind, label_key, shape?, rows, warnings, failure_hotspots }`. `warnings` is justified by silent-fallback elimination. `label_key` values: table→`"command"`, drill→`"argv_shape"`, by_agent→`"agent"`.
4. **No legacy sentinel** — founder confirmed no backcompat. Remove `SessionId::sentinel_legacy`. Empty/malformed `session_id` → `Warning::EmptySessionId` + drop event from aggregation. `CommandName::try_from_raw` likewise rejects unknown.
5. **Sliding-window definition (unambiguous):** for each `(day, CommandName, ArgvShape)` group, sort events ASC by ts; emit *maximal non-overlapping* windows of width ≤ `retry_window` whose count ≥ 2. Two-pointer `l`, `r`: advance `r`; when `ts[r] - ts[l] > window`, emit cluster if `r - l ≥ 2`, then set `l = r`.

These corrections are binding; §§3–6 above are read through this §10.

6. **2026-04-18**: Finalized `StatsReport` shape. Removed invented `window` and `blind_spots`. Added explicit `FailureHotspot` definition. `StatsReport` now uses typed `ReportKind` and `LabelKey` enums (inline, defined in §5) instead of `&'static str`. `StatsRow` field names finalized: `label` (polymorphic), nested `duration_ms: Option<DurationStats>`, `ok_rate: Option<f64>`, `output_bytes_per_call_mean: Option<f64>`, `retry_cluster_count: u64`.

## 11. Non-goals (explicitly out of scope for v2)
- Cross-session retry detection (memory'd as candidate for v3 — see `log_stats_first_diagnosis.md`).
- Persistence compaction / rotation of `~/.8v/events.ndjson`.
- New CLI flags beyond what Level 1 already specified.
- Performance work beyond "single-pass aggregator."
