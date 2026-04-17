# `8v stats` — Design

**Status:** Draft. Rounds 1-4 applied jointly with `log-command.md`. Pending final verification round.
**Sibling:** `log-command.md` — the temporal reader. This doc is the analytical aggregator. Shares schema, argv normalization (§6.1 of log), `--strict` semantics, session boundary.
**Depends on:** commit `e3f610d` (argv threading) + `session_id` schema field (log §3).

## 1. Problem

Is `8v write --find/--replace` slow? Does `search` p95 differ between claude-code and codex? These are aggregate questions over `~/.8v/events.ndjson`. `8v log` answers "what happened in that session"; wrong tool for "what is the p95 of `write` across the last week".

## 2. Users

1. **Founder, post-hoc / during dogfood.** One-screen answer to "which commands are slow, which fail". Drives the next ergonomics fix.
2. **Benchmark pipeline.** `8v stats --json` replaces ad-hoc jq over the raw log with stable field names.
3. **Agent in-loop.** Out of scope for v1.

## 3. Surface

```
8v stats                        # per-command table, last 7 days   [DEFAULT]
8v stats <command>              # drill into one command (e.g. `8v stats write`)
8v stats --compare agent        # group by agent.name
```

Positional argument is always a **command name** (`read`, `write`, `search`, …), never a session id. That disambiguates `stats` from `log`.

### 3.1 Default — `8v stats`

```
$ 8v stats
window: 2026-04-10 to 2026-04-17  (7d)  2,341 commands across 18 sessions

cmd             n     p50    p95    p99    ok%    out/call   retries
read         1,241      2ms    18ms   94ms   100%   1.2 KB         3
write          487      4ms    12ms   40ms    92%   0.1 KB        14
search         214     38ms   180ms  420ms   100%   8.4 KB         0
check           83    410ms  1.8s    3.2s    84%   2.1 KB         2
fmt             54    280ms  820ms   920ms  100%   0.8 KB         0
ls              48      3ms    19ms   41ms   100%   0.9 KB         0
build           18      2.1s  9.8s   18.4s   94%   4.2 KB         0
test            12      3.4s  12.1s  24.8s   92%  12.8 KB         0

failure hotspots
  write --find <str> --replace <str>   31 / 142  (21.8%)  top path: handler.rs x9
  check                                13 /  83  (15.7%)  -

blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct; durations use duration_ms (monotonic, host-local).
```

Columns:
- `n` — count of `CommandCompleted` in the window.
- `p50 / p95 / p99` — percentiles of `duration_ms`. `-` when `n < 5`.
- `ok%` — `success=true` rate.
- `out/call` — mean `output_bytes` per call (thrash detection).
- `retries` — cross-session retry clusters this command contributed to (log §6 rule, aggregated across window).

Failure hotspots: top K `(command, argv-shape)` by failure count, tie-broken by rate. K = 3 default, `--top N`.

### 3.2 Drill — `8v stats <command>`

Per-command breakdown by argv-shape:

```
$ 8v stats write
write   487 calls   p50 4ms   p95 12ms   ok% 92%

argv-shape                           n     p50    p95    ok%   top path
write <path>:<line> <str>          298      3ms    8ms   100%   src/main.rs
write <path> --find <str> --re...  142      5ms   14ms    78%   handler.rs
write <path> --insert <str>         34      2ms    7ms   100%   -
write <path> --append <str>         13      3ms   10ms   100%   -

failure hotspots (within write)
  write <path> --find <str> --re...  31 / 142 (21.8%)  top path: handler.rs x9
```

Argv-shapes with `n = 1` rolled into an `other` row to avoid one-hit-wonders.

### 3.3 Compare — `--compare agent`

One dimension only in v1: **`agent`**. Cross-agent comparison is the benchmark pipeline's use case (user #2). Other dimensions (`project`, `caller`, `stack`) **cut from v1** — no named user.

```
$ 8v stats --compare agent
window: 2026-04-10 to 2026-04-17  (7d)

agent                    n      p50    p95    ok%    retries/1K
claude-code 2.1.112  1,612       3ms    24ms   96%         8.1
codex       0.121.0    729       5ms    51ms   89%        42.3
(no agent / CLI)        -        -       -       -           -
```

`(no agent / CLI)` row shown only if any CLI events match the filter window; `-` when empty to make the absence legible (rather than silently dropped).

### 3.4 JSON — `--json`

Stable field names for the benchmark pipeline:

```
$ 8v stats --json | jq '.rows[] | select(.command=="write")'
{
  "command": "write",
  "n": 487,
  "duration_ms": { "p50": 4, "p95": 12, "p99": 40 },
  "ok_rate": 0.92,
  "output_bytes_per_call_mean": 128,
  "retry_cluster_count": 14,
  "failure_hotspots": [
    { "argv_shape": "write <path> --find <str> --replace <str>",
      "n": 142, "failures": 31, "top_path": "handler.rs", "top_path_count": 9 }
  ]
}
```

**JSON field names are a stable contract** once the benchmark pipeline consumes them. Renaming a field = breaking change that requires bumping a version tag.

## 4. Signals — how each is computed

All from `CommandStarted` + `CommandCompleted` joined on `run_id`. Argv normalization spec shared with `log-command.md` §6.1 (per-event `project_path`, basename fallback warned once per session, path separators normalized, etc.). Single-pass reader (log §6.2).

| signal | computation |
|---|---|
| `n` per command / argv-shape | count matching `CommandCompleted` rows |
| `p50 / p95 / p99` | percentile of `duration_ms` via **fixed log-histogram**: 60 log-spaced buckets over 1ms–1000s. Exact-within-bucket, no streaming digest needed. Deterministic across runs given identical input. Renders `-` when `n < 5`. |
| `ok%` | `success=true` / total |
| `out/call` | mean `output_bytes` |
| `retries` | cross-session retry clusters (log §6 rule). `--retry-window` default 30s, **applied per-day** — retries never span midnight even if within window. |
| failure hotspots | top K `(command, argv-shape)` by failure count, tie-broken by rate. K from `--top` (default 3). |

## 5. Flags

**Common filters** (verbatim with `log` §5):

`--json`, `--since <dur>` (default **7d**), `--until`, `--on <date>`, `--project <path>`, `--caller cli|mcp`, `--agent <name>`, `--strict`.

**Stats-specific:**

| flag | behavior |
|---|---|
| `--compare agent` | group by `agent_info.name` (only dimension in v1) |
| `--top N` | failure-hotspots row count (default 3) |
| `--percentiles p50,p95,p99` | override computed percentiles |
| `--min-n N` | hide rows with fewer than N samples (default 1) |

## 6. Edge cases

Shared with `log-command.md` §6.3: empty sessions, DST, orphan/duplicate events, malformed lines, clock skew, legacy events, filter-then-limit ordering, `--strict`.

**Stats-specific:**

| case | rule |
|---|---|
| Command with `n < 5` | percentile columns `-`; row still shown unless `--min-n` hides it |
| Argv-shape seen once (`n=1`) | rolled into `other` row in drill view |
| Identical samples (all same duration) | p50 = p95 = that bucket — correct, flagged only by visual column equality |
| `--compare agent` with session-less events | grouped under `(no agent / CLI)` row if non-empty, else omitted |
| Empty window (no events match filters) | print `no matching events` to **stderr**, exit **2** (distinct from exit 0 with zero rows — no silent-empty-result anti-pattern) |
| Legacy events (no `session_id`) | included in aggregates (they still have `duration_ms` and `success`); `retries` cannot cluster them (no session scope), so they contribute `0` to retry counts and this is noted in blind spots |

## 7. Blind spots (footer of every human view)

Enumerated explicitly (no blanket "inherits") — four items apply to `stats`:

- Native tool calls outside 8v — invisible.
- Write "success" ≠ code-correct.
- Clock skew: NFS-shared log from multiple hosts → `timestamp_ms` non-monotonic. Durations ok (`duration_ms` is monotonic), window-bucketing (`--since`, `--on`) wrong for skewed hosts.
- Malformed lines — skipped (or `--strict`).

`session_id` is-per-process blind spot from `log` §7 does **not** apply to `stats` (stats is non-session).

## 8. Out of scope (v1)

- Time-series graphs / sparklines.
- Alerting / thresholds.
- Persistent stat snapshots (every invocation recomputes — cheap for <100K events).
- Cost modeling (token × price).
- Cross-window diff (this week vs last week) — use two invocations + jq.
- `--compare project | caller | stack` — **cut**. No named user. Revisit after v1 lands if asked.

## 9. Files touched

- `o8v/src/commands/stats.rs` — new subcommand module.
- `o8v-core/src/render/stats_report.rs` — report types + Renderable impls (stable JSON field names).
- `o8v/src/event_reader.rs` — shares the single-pass aggregator state with `log`. `stats` projects different views over the same state. **If reviewer finds duplicated parsing / normalization between `log` and `stats`, that blocks merge.**
- `o8v/src/commands/mod.rs` — dispatch arm.
- Tests: `o8v/tests/e2e_stats.rs`; unit tests for histogram bucket math.

No schema changes beyond what `log` requires (`session_id` + `#[serde(default)]`).

## 10. Dispositions (all prior open questions resolved)

| # | Question | Disposition |
|---|---|---|
| 1 | Percentile algorithm | **Fixed log-histogram**, 60 buckets over 1ms–1000s. Deterministic, simple, no crate dependency. |
| 2 | `stack` dimension | **Cut from v1.** Would require either read-time project-resolution (fragile for deleted projects) or emit-time schema field (extra schema change). No named user. |
| 3 | Default window | **7 days.** Configurable via `--since`. |
| 4 | `--compare` dimensions | **`agent` only.** `project | caller | stack` cut. |
| 5 | JSON field stability | **Stable contract** once benchmark pipeline consumes it. Field renames require version bump. |
| 6 | Empty-window behavior | Stderr `no matching events`, exit 2. |
| 7 | Legacy events in aggregates | Included (they have `duration_ms`/`success`); excluded from retry clustering; noted in blind spots. |

## 11. E2E test plan

Minimum E2E set, each must fail on pre-fix code:

1. **`e2e_stats::default_table_renders`** — seeded fixture with ≥ 3 commands, assert table rows contain expected `n`, `p95`, `ok%`.
2. **`e2e_stats::drill_argv_shape_breakdown`** — seeded `write --find/--replace` failures, assert `8v stats write` shows the argv-shape row with `ok% < 100`.
3. **`e2e_stats::compare_agent_separates_rows`** — fixture with mixed `agent_info.name`, assert `--compare agent` yields one row per distinct agent.
4. **`e2e_stats::n_lt_5_percentiles_dashed`** — fixture with 3 calls of a command, assert p50/p95/p99 render `-`.
5. **`e2e_stats::empty_window_exits_2`** — `--since 0s` or future `--on`, assert exit 2 and stderr `no matching events`.
6. **`e2e_stats::json_field_contract`** — `--json` output, assert presence of documented fields (`command`, `n`, `duration_ms.p50/p95/p99`, `ok_rate`, `retry_cluster_count`, `failure_hotspots[]`).
7. **`e2e_stats::malformed_line_skipped`** — corrupt NDJSON line in fixture, default exit 0, `--strict` exits non-zero.
8. **`e2e_stats::legacy_events_aggregated`** — pre-`session_id` lines in fixture, assert their durations contribute to percentiles but they don't form retry clusters.

Unit coverage: log-histogram bucket math, percentile extraction from histogram, argv-shape extraction for drill view.
