# `8v stats` — Design

**Status:** Draft. Round 1 + 2 in progress (the two rounds happen jointly with `log-command.md` since both share schema + normalization). No code until 3 review rounds come back empty and the feature freeze (2026-04-14) lifts.
**Sibling doc:** `log-command.md` — the temporal reader. This doc is the analytical aggregator. Same data source, different verb.
**Depends on:** commit `e3f610d` (argv threading) + `session_id` schema field (from `log-command.md` §3).

## 1. Problem

Is `8v write --find/--replace` slow? Is `search` faster on claude-code than on codex? Does `check` regress after a release? These are aggregate questions over `~/.8v/events.ndjson`. `8v log` answers "what happened in that session"; it is the wrong tool for "what is the p95 of `write` across the last week".

## 2. Users

1. **Founder, post-hoc / during dogfood.** Wants a one-screen answer to "which commands are slow and which fail". Drives the next ergonomics fix.
2. **Benchmark pipeline.** `8v stats --json` feeds the benchmark reports; today the pipeline has to jq the raw log. Replacing that with a single-call JSON gives stable field names.
3. **Agent in-loop.** Out of scope for v1. Same reason as `log`.

## 3. Surface

Single subcommand, no nesting:

```
8v stats                        # per-command table, last 7 days (DEFAULT)
8v stats read                   # drill into one command
8v stats --compare agent        # group by agent.name
8v stats --compare project      # group by project_path
```

Progressive default: give the minimum useful answer (per-command table) without arguments. Positional argument is a **command name** (`read`, `write`, `search`, …) — never a session id. That's what disambiguates `stats` from `log`.

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
...

failure hotspots
  write --find/--replace         31 / 487   (6.4%)   top path: handler.rs  x9
  check                          13 /  83  (15.7%)   top stack: typescript

blind spots: duration trusts duration_ms; clock skew not corrected; n<5 → stats shown as `-`.
```

Columns:
- `n` — count of `CommandCompleted` for that command in the window.
- `p50/p95/p99` — percentiles of `duration_ms`. `-` when `n < 5`.
- `ok%` — `success=true` rate.
- `out/call` — mean `output_bytes` per call (useful for thrash detection).
- `retries` — count of cross-session retry clusters this command contributed to (see §4).

The **failure hotspots** block surfaces the top 3 (command, argv-shape) pairs by failure rate, with the most-frequent path if paths are present in argv. That's the direct hand-off to the ergonomics backlog.

### 3.2 Drill — `8v stats <command>`

Per-command breakdown by argv-shape. This answers "is `write --find/--replace` slow, or is all `write` slow":

```
$ 8v stats write
write   487 calls   p50 4ms   p95 12ms   ok% 92%

argv-shape                            n     p50    p95    ok%   top path
write <path>:<line> <str>           298      3ms    8ms   100%   src/main.rs
write <path> --find <str> --re...   142      5ms   14ms    78%   handler.rs
write <path> --insert <str>          34      2ms    7ms   100%   -
write <path> --append <str>          13      3ms   10ms   100%   -

failure hotspots (within write)
  write <path> --find <str> --re...  31 / 142 (21.8%)  top path: handler.rs x9
```

### 3.3 Compare — `--compare <dimension>`

Grouping axis for cross-agent or cross-project comparison:

```
$ 8v stats --compare agent
window: 2026-04-10 to 2026-04-17  (7d)

agent                    n      p50    p95    ok%    retries/1K
claude-code 2.1.112  1,612       3ms    24ms   96%         8.1
codex       0.121.0    729       5ms    51ms   89%        42.3
cli (no agent)          -        -       -       -           -
```

Valid dimensions: `agent`, `project`, `caller`, `stack` (when project_path resolves to a detected stack).

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
    { "argv_shape": "write <path> --find <str> --re...",
      "n": 142, "failures": 31, "top_path": "handler.rs", "top_path_count": 9 }
  ]
}
```

## 4. Signals — how each is computed

All from `CommandStarted` + `CommandCompleted` joined on `run_id`. Argv normalization spec is shared with `8v log` — see `log-command.md` §6, "Argv normalization". Single-pass over `events.ndjson` (same §6.1 constraint).

| signal | computation |
|---|---|
| `n` per command / argv-shape | count `CommandCompleted` rows matching the key |
| `p50 / p95 / p99` | percentile of `duration_ms` values, computed with a streaming-friendly digest (t-digest or fixed-bucket histogram — implementation detail, document in code); `-` when `n < 5` |
| `ok%` | `success=true` / total |
| `out/call` | mean `output_bytes` |
| retries | cross-session retry clusters (same rule as `log` §6: `(command, normalized-argv)` with `≥ 2` occurrences in `--retry-window`; here aggregated across **all** sessions in the window) |
| failure hotspots | top K `(command, argv-shape)` pairs ranked by failure count, tie-broken by failure rate; K = 3 in default view, configurable via `--top` |

## 5. Flags

Shared with `8v log` (defined in `log-command.md` §5):

| flag | behavior |
|---|---|
| `--json` | structured output |
| `--since <dur>` | relative window (default **7d**) |
| `--until <dur\|time>` | close the window (reproducibility) |
| `--on <date>` | calendar day, local time |
| `--project <path>` | filter by `project_path` |
| `--caller cli\|mcp` | filter |
| `--agent <name>` | filter (implies `--caller mcp`) |
| `--strict` | hard-fail on malformed lines (default: skip with stderr warning) |

Stats-specific:

| flag | behavior |
|---|---|
| `--compare <dim>` | group by dimension: `agent`, `project`, `caller`, `stack` |
| `--top N` | number of rows in failure-hotspots block (default 3) |
| `--percentiles p50,p95,p99` | override which percentiles to compute |
| `--min-n N` | hide rows with fewer than N samples (default 1) |

## 6. Edge cases

Shared rules with `log-command.md` §6.2 (empty sessions, DST, duplicate/orphan events, malformed lines, clock skew, file rotation, argv normalization, filter-then-limit).

Stats-specific:

| case | rule |
|---|---|
| Command with `n < 5` | all percentile columns render `-`; row still shown unless `--min-n` hides it |
| Argv-shape seen only once | rolled into an "other" row in drill view to avoid one-hit-wonders dominating |
| Percentile on identical samples | p50 = p95 = that value; correct, not a bug — flagged visually only by column equality |
| `--compare <dim>` where a session has no value (e.g. CLI row with no agent) | grouped under the dimension's "none" bucket, shown as `(no agent)` / `(no project)` |
| Empty window (no events match filters) | print `no matching events` to stderr, exit 2 (distinct from exit 0 with zero rows) |

## 7. Blind spots

Inherits `log-command.md` §7 (native tool calls invisible, write-success ≠ code-correct, multi-host clock skew, malformed lines). No stats-specific additions.

## 8. Out of scope (v1)

- Time-series graphs / sparklines (terminal rendering cost, low value).
- Alerting / thresholds.
- Persistent stat snapshots (every invocation recomputes from the log — cheap for <100K events).
- Cost modeling (token price × token count). Separate feature if wanted.
- Comparison across time windows (this week vs last week) — possible via two invocations + diff, not built in.

## 9. Files touched when implementation begins

- `o8v/src/commands/stats.rs` — new subcommand.
- `o8v-core/src/render/stats_report.rs` — report types + Renderable impls.
- `o8v/src/event_reader.rs` — shares the single-pass aggregator with `log` (see `log-command.md` §9); `stats` just exposes a different projection of the same state.
- `o8v/src/commands/mod.rs` — dispatch arm.
- Tests: `o8v/tests/e2e_stats.rs`, unit tests.

**Zero duplication with `log`:** the reader is one piece of code; `log` and `stats` are two views over its state. If a reviewer finds duplicated event-parsing or argv-normalization code between the two, that blocks merge.

## 10. Open questions

1. **Percentile algorithm.** t-digest (accurate, ~1 KB state) vs fixed log-bucket histogram (exact within bucket, trivial to implement). Recommend fixed histogram for v1 — durations span ~6 orders of magnitude (1ms–1000s), 60 log-buckets is enough. ← founder confirm or defer to implementation.
2. **`stack` dimension in `--compare`.** Requires resolving `project_path` → stack via `o8v-project`. Do we do it at read time (may fail for deleted projects) or stamp `stack` on `CommandStarted` at emit time (another schema field, cleaner)? Recommend: **emit-time**, which is a second schema change we'd bundle with `session_id`. ← founder decide.
3. **Default window.** 7 days is a guess. If agents run heavily, 7 days is too much history; if lightly, too little. Recommend keep 7d; user overrides with `--since`. ← founder confirm.
