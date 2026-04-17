# `8v log` — Design

**Status:** Draft. Rounds 1-4 applied. Pending final verification round. No code until verification round returns empty and the feature freeze (2026-04-14) lifts.
**Supersedes:** earlier "agent feedback harness" framing. This doc is the temporal reader only. Analytical aggregates live in `stats-command.md`.
**Sibling:** `stats-command.md` (shares schema, argv normalization, `--strict` semantics).
**Depends on:** commit `e3f610d` (argv threading, 2026-04-17).

## 1. Problem

`~/.8v/events.ndjson` has every 8v command call with timestamps, argv, success, duration, project, and agent. There is no tool to read it. "What did the agent do in that session?" and "where does `write --find/--replace` fail?" require jq and grep. The raw log is the source of truth; what's missing is a reader.

## 2. Users

1. **Founder, post-hoc.** Reviewing an agent session (minutes to hours later). Decides which 8v ergonomics to fix next. First-class user.
2. **CI / scripting.** `8v log --json` consumed by scripts or benchmarks. Same data, different renderer.
3. **Agent in-loop self-correction.** Out of scope for v1. Explicitly deferred.

## 3. Session boundary

Every event gets a `session_id: String` field on both `CommandStarted` and `CommandCompleted`.

- **MCP:** one session id per `initialize` call; threaded via `CommandContext`; stamped on every event until transport closes. Laptop sleep/wake does **not** rotate the id — the transport hasn't closed.
- **CLI:** one process = one session. Generated in `main.rs` before dispatch.
- **Format:** ULID with literal `ses_` prefix (e.g. `ses_01HW4K…`). 26 chars, lexicographically sortable by creation time. Prefix disambiguates positional args from subcommand names (§4).

**Backward compatibility (critical):** `session_id` is declared with `#[serde(default)]` on both event types. Legacy lines in `~/.8v/events.ndjson` (written before this change) deserialize with `session_id = ""`. All such lines are grouped into a single pseudo-session with id `ses_legacy` and the label "(legacy, pre-session-id events)" in the sessions list. Existing `event_reader.rs` tests must continue to pass — the fixture at `event_reader.rs:169` has no `session_id` and must still deserialize.

## 4. Surface

```
8v log                       # sessions list (last 10, newest first)  [DEFAULT]
8v log last                  # drill into the most recent session
8v log show <ses_id>         # drill into a specific session
8v log search <query>        # cross-session search
```

(`-f` / live-tail cut from v1: daemon-adjacent complexity, no named user. If a CI user emerges later, revisit.)

Positional session ids must start with `ses_`. Anything else is either a reserved subcommand (`last`, `show`, `search`) or an error. Prefix match operates on the full ULID: `8v log show ses_01HW` resolves if unambiguous against the set of sessions in the current `events.ndjson`, errors if not (no silent fallback). For a 10K-session log a naive scan is fine; if it becomes a hotspot, add an in-memory index.

### 4.1 Default — `8v log`

```
  id          when                 dur    cmds  fail  out     tokens  agent
  ses_a1f3    2026-04-17 14:32     15m     42     4   34KB     8.5K   claude-code 2.1.112
  ses_9c02    2026-04-17 11:08     42m    108     0   210KB   52.4K   claude-code 2.1.112
  ses_7e4d    2026-04-17 09:50      3m      9     2   4KB      1.0K   -
  ses_4b11    2026-04-16 22:14   1h12m    231    19   980KB  245.0K   codex 0.121.0
  ses_legacy  -                    -   1,204     73   3.2MB  —        (legacy, pre-session-id)
  ...  (showing 10 of 87, --all to see all, --limit N to change)

blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.
```

### 4.2 Drill-in — `8v log last` or `8v log show <id>`

```
session ses_a1f3   2026-04-17 14:32-14:47  (15m)  claude-code 2.1.112  mcp
project /Users/.../oss/8v
---
  42 commands   38 ok   4 fail   p50 5ms  p95 240ms   34KB out

per-cmd p95    read 18ms  write 14ms  search 180ms  check 1.8s
               (run '8v stats' for p50/p99, ok%, argv-shape breakdown)

top commands   read 18  write 9  search 7  check 4  ls 4
failures       write x3   search x1
retries        read src/main.rs --full   x4 in 90s
               write handler.rs --find … x3 consecutive fail

blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.
```

The **per-cmd p95** line addresses the common founder question "is read/write/search slow in this session" without leaving the reader. Breakdown (p50/p99, ok%, argv-shape) lives in `8v stats` — one level deeper.

`-v` / `--verbose` adds the per-command table:

```
  #   time   cmd     argv (truncated)                    ms    ok
  1   14:32  ls      --tree --loc                         5    ok
  2   14:32  read    src/main.rs                          2    ok
  3   14:33  read    src/main.rs --full                  12    ok
  4   14:33  read    src/main.rs --full                  11    ok   <- retry
  5   14:33  read    src/main.rs --full                  10    ok   <- retry
  7   14:34  write   handler.rs --find "foo" --rep...     8    FAIL not found
  8   14:35  write   handler.rs --find "foo " --re...     9    FAIL not found
  9   14:35  write   handler.rs --find "foo\n" --r...     7    FAIL not found
```

### 4.3 Failures — `--failures`

View flag on drill-in and search. For sessions-list filtering: `8v log --has-failures`.

```
$ 8v log show ses_a1f3 --failures
3 failures, 1 cluster

cluster #1  write --find/--replace on handler.rs  (3 in 60s)
  14:34  write handler.rs --find "foo"   -> error: pattern not found
  14:35  write handler.rs --find "foo "  -> error: pattern not found
  14:35  write handler.rs --find "foo\n" -> error: pattern not found
```

### 4.4 Retries — `--retries`

```
$ 8v log last --retries
read  src/main.rs --full   x4 in 90s   44KB output
```

### 4.5 Search — `8v log search <query>`

```
$ 8v log search "handler.rs"
3 sessions, 14 matches

ses_a1f3  2026-04-17 14:34  write   handler.rs --find "foo"          FAIL
ses_a1f3  2026-04-17 14:35  write   handler.rs --find "foo "         FAIL
ses_a1f3  2026-04-17 14:36  read    handler.rs 42-60                 ok
ses_9c02  2026-04-17 11:14  read    handler.rs                       ok
ses_9c02  2026-04-17 11:15  write   handler.rs:45 "..."              ok
```

Query matches `command`, `argv`, `project_path`, error text.

### 4.6 JSON — `--json`

```
$ 8v log show ses_a1f3 --json | jq '.summary'
{
  "session_id": "ses_a1f3...",
  "caller": "mcp",
  "agent": { "name": "claude-code", "version": "2.1.112" },
  "started_ms": 1776435147000,
  "ended_ms":   1776436050000,
  "commands": 42, "ok": 38, "fail": 4, "incomplete": 0,
  "p50_ms": 5, "p95_ms": 240,
  "per_command_p95_ms": { "read": 18, "write": 14, "search": 180, "check": 1800 },
  "output_bytes_total": 34816,
  "top_commands": [["read",18],["write",9],["search",7]],
  "clusters": [
    { "kind": "retry",   "command": "read",  "argv_shape": "...", "count": 4 },
    { "kind": "failure", "command": "write", "path": "handler.rs", "count": 3 }
  ]
}
```

## 5. Flags

**Common filters** (shared verbatim with `stats`):

| flag | behavior |
|---|---|
| `--json` | structured output |
| `--since <dur>` | e.g. `1h`, `2d`. Relative to wall-clock at invocation. |
| `--until <dur\|time>` | closes the window for reproducibility |
| `--on <date>` | calendar day in local time (DST handling in §6.2) |
| `--project <path>` | filter by canonicalized `project_path` |
| `--caller cli\|mcp` | filter by entry point |
| `--agent <name>` | filter by `agent_info.name`; implies `--caller mcp` (CLI rows filtered out) |
| `--strict` | hard-fail on malformed NDJSON lines (default: skip with stderr warning) |

**Selection** (mutually exclusive clap ArgGroup "scope"):

| form | behavior |
|---|---|
| `8v log` | sessions list |
| `8v log last` | most recent session |
| `8v log show <id>` | specific session |
| `--all` | every session (only valid with sessions list or `search`) |
| `--limit N` | cap count (sessions list, search) |

**Views:**

| flag | scope | behavior |
|---|---|---|
| (none) | drill-in | compact summary |
| `-v` / `--verbose` | drill-in | per-command table |
| `--failures` | drill-in, search | failing commands + clusters |
| `--has-failures` | sessions list | boolean filter |
| `--retries` | drill-in | repeated argv only |

**Search-only:** `--files`, `-e <ext>`, `-C N`.

## 6. Signals — how each is computed

All from `CommandStarted` + `CommandCompleted` joined on `run_id`, scoped by `session_id`.

| signal | computation |
|---|---|
| count / ok / fail / incomplete | count rows; `incomplete` = Started without Completed (§6.2) |
| p50 / p95 `duration_ms` | from `CommandCompleted.duration_ms` (authoritative, monotonic). Render `-` when `n < 5`. Avg never shown. |
| **per-command p95** | same as above, grouped by `command`, restricted to top-K by count (default K = 5) |
| top commands | histogram of `command` |
| retries | `(command, normalized-argv)` pairs with ≥ 2 occurrences, first-to-last span ≤ `--retry-window` (default **30s**). Interleaved events allowed. |
| failure clusters | `(command, normalized-argv, project_path)` pairs with `success=false` ≥ 2 in ≤ `--retry-window`. |
| re-reads | `read`-family argv, distinct path tokens + counts |

### 6.1 Argv normalization (shared contract)

Normalization happens **per-event, using that event's own `project_path`** (not a global canonical path — each event stands alone):

- Canonicalize the event's `project_path` via `std::fs::canonicalize` when loading (collapses macOS `/Users/…` vs `/private/var/…`).
- Paths inside canonical `project_path` → relativized (`src/main.rs`).
- Paths outside → `<abs>`.
- Tempdir-like paths (`/tmp`, `/var/folders/…`, `$TMPDIR`) → `<tmp>`.
- Path separators normalized to `/`.
- **When `project_path` is `None`:** fall back to basename-only matching for `read`/`write` argv. This is an explicit degradation, not a silent fallback — `8v log` emits `warning: session <id> has events with no project_path; using basename matching` to stderr **once per session**, unless `--json` (then included as `"warnings":["..."]` in the JSON payload).
- Quoted string values → `<str>` in shape form; exact string preserved in `-v`.

### 6.2 Single-pass computation

One linear pass over `events.ndjson`. Memory O(sessions × distinct argv-shapes × distinct paths), not O(events). 10K events stays flat. If a reviewer finds re-scan per signal, that blocks merge.

### 6.3 Edge cases

| case | rule |
|---|---|
| Empty session (Started, no commands) | listed with `-` stats, `0 cmds`; no p50/p95 |
| Single-command session | counts shown; percentiles `-` (n<5) |
| All-identical session | retry cluster works; top-commands shows one row |
| Crosses local midnight | session belongs to local day of its first `CommandStarted` |
| DST transition | `chrono::Local`; fall-back-hour events assigned to first occurrence |
| Orphan `CommandStarted` | `incomplete` bucket; surfaced in summary (`38 ok 4 fail 1 incomplete`) |
| Duplicate `CommandStarted` for same `run_id` | first wins; subsequent logged to stderr as warning |
| Duplicate `CommandCompleted` for same `run_id` | first kept; subsequent warned |
| Malformed NDJSON line | skipped with stderr warning; exit 0. `--strict` hard-fails. |
| Clock skew across hosts | duration from `duration_ms` (monotonic); ordering by file offset; skew in blind spots |
| Legacy events (no `session_id`) | grouped under pseudo-session `ses_legacy` |
| `--limit N` with filters | filters first, then limit. `(showing 10 of 87)` counts post-filter pre-limit. |

**Signals dropped (cut after review):**
- `stuck-point` — redundant with failure clusters.
- `resolved-by` — heuristic produced false positives.

## 7. Blind spots (footer of every human view)

- Native Read/Edit/Bash outside 8v — invisible.
- Write "success" ≠ code-correct (compiles but breaks a test).
- Multi-machine: `session_id` is per-process, not per-agent-conversation.
- Clock skew: NFS-shared `~/.8v` from multiple hosts → `timestamp_ms` non-monotonic. Durations ok (monotonic), bucketing wrong.
- Malformed lines — skipped (or `--strict`).
- Legacy events (`ses_legacy`) lack session boundaries — grouped but not meaningful as "a session".

## 8. Out of scope (v1)

- Dashboards / time-series DB.
- Multi-day trend reports (use `--all --json | jq`).
- In-loop agent feedback.
- Network / cloud.
- Writing to the log — `8v log` is read-only.
- CI gating on exit code — no named user.
- `-f` live tail — cut; revisit if user emerges.
- `--compare <dim>` — lives in `stats`, not `log`.

## 9. Files touched

- `o8v-core/src/events/lifecycle.rs` — add `#[serde(default)] session_id: String` to both event structs. Extend `CommandStarted::new` signature (breaking change for internal callers — `dispatch.rs` and tests must update in the same commit).
- `o8v/src/main.rs` — generate CLI session id.
- `o8v/src/mcp/mod.rs` / handler — generate MCP session id on `initialize`.
- `o8v/src/dispatch.rs` — thread session id through `CommandContext`.
- `o8v/src/event_reader.rs` — gain `parse_events_lenient(content, strict: bool)`. The existing `parse_events` keeps its hard-fail contract (backward compat for internal callers); the new lenient entry point is what `log` uses. Warnings go to stderr via a small `tracing::warn!` path.
- `o8v/src/commands/log/` — new module: `mod.rs`, `sessions.rs`, `drill.rs`, `search.rs`.
- `o8v-core/src/render/log_report.rs` — report types + Renderable impls.
- `o8v/src/commands/mod.rs` — dispatch arm.
- Tests: `o8v/tests/e2e_log.rs`; unit tests under each touched module; existing `event_reader.rs` tests updated for new optional field.

## 10. Dispositions (all prior open questions resolved)

| # | Question | Disposition |
|---|---|---|
| 1 | Retry window default | **30s**, configurable via `--retry-window`. |
| 2 | Failure clustering shape | Normalized argv per §6.1; exact strings shown in `-v`. |
| 3 | `--agent` on CLI rows | Implies `--caller mcp`. No special-case for CLI. |
| 4 | MCP session on sleep/wake | Same id until transport closes (no rotation). |
| 5 | Legacy events rendering | Pseudo-session `ses_legacy`, labeled in sessions list. |
| 6 | `--strict` scope | Top-level common flag (applies to every subcommand). |
| 7 | `parse_events` vs lenient | New `parse_events_lenient` entry point; old API unchanged. |
| 8 | Prefix match scope | All sessions in the file; naive scan is fine for v1. |

## 11. E2E test plan

Minimum E2E set — each scenario must exist and fail on pre-fix code:

1. **`e2e_log::argv_present_regression`** — run `8v ls --tree`, assert `argv:["ls","--tree"]` lands in `events.ndjson`. This is the regression for bug #13.
2. **`e2e_log::session_id_stamped_cli`** — run any command, assert `session_id` is a valid `ses_*` ULID on both Started and Completed events.
3. **`e2e_log::session_id_stamped_mcp`** — spin up MCP server, issue two calls, assert both events share one `session_id`.
4. **`e2e_log::legacy_events_grouped`** — seed a fixture with pre-`session_id` lines, run `8v log`, assert they appear under `ses_legacy` without panic.
5. **`e2e_log::malformed_line_skipped`** — seed `events.ndjson` with one `^@` corrupt line, assert `8v log` exits 0 with stderr warning; `--strict` exits non-zero.
6. **`e2e_log::drill_shows_per_cmd_p95`** — fixture with mixed durations, assert `8v log last` renders the `per-cmd p95` line.
7. **`e2e_log::retry_cluster_detected`** — fixture with 4 identical `read x.rs --full` in 60s, assert `--retries` output lists it with `x4`.
8. **`e2e_log::orphan_started_counted`** — fixture with a Started but no Completed, assert summary shows `incomplete: 1`.

Additional unit coverage (argv normalization canonicalization, DST, duplicate run_id) in the reader module.
