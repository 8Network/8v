# `8v log` — Design

**Status:** Draft. Rounds 1+2/3 review applied. No code until all 3 review rounds come back empty and the feature freeze (2026-04-14) lifts.
**Supersedes:** the earlier "agent feedback harness" framing (same file, renamed 2026-04-17). That framing muddled three things — observability, analysis, and in-loop feedback. This doc covers only the first two. In-loop feedback is a separate, later product.
**Depends on:** commit `e3f610d` (argv threading into `CommandStarted`, 2026-04-17) — every signal below needs argv.

## 1. Problem

`~/.8v/events.ndjson` has every 8v command call with timestamps, argv, success, duration, project, and agent. There is no tool to read it. "What did the agent do in that session?" and "where does `write --find/--replace` fail most?" require jq and grep. The raw log is the source of truth; what's missing is a reader.

## 2. Users

1. **Founder, post-hoc.** Reviewing an agent session (minutes to hours later). Decides which 8v ergonomics to fix next. First-class user.
2. **CI / scripting.** `8v log --json` consumed by scripts, benchmarks, or a future dashboard. Same data, different renderer.
3. **Agent in-loop self-correction.** Out of scope for v1. Explicitly deferred — build for user 1 first.

## 3. Session boundary

Every event gets a `session_id: String` field on both `CommandStarted` and `CommandCompleted`.

- **MCP entry point:** generate one session id on `initialize`, thread through `CommandContext`, stamp every event until transport closes.
- **CLI entry point:** one process = one session. Generate in `main.rs` before dispatch.

Only schema change. No daemon, no index file, no new event types. Session id format: **ULID with `ses_` prefix** (e.g. `ses_01HW4K…`). ULIDs sort lexicographically by creation time, 26 chars, human-glanceable. The `ses_` prefix is literal — both to namespace the id in logs and to disambiguate positional args from subcommand names (§4).

## 4. Surface

```
8v log                       # sessions list (last 10, newest first)  [DEFAULT]
8v log last                  # drill into the most recent session
8v log show <ses_id>         # drill into a specific session
8v log search <query>        # cross-session search
8v log -f                    # live tail
```

Rationale for default = sessions list (not "last session"): matches `ls` at repo root — the minimum-useful answer when given no target is the index, not a single item. User can always `8v log last` for the previous behavior.

Positional session ids must start with `ses_`. Anything that doesn't is either a reserved subcommand (`last`, `show`, `search`) or an error (`error: unknown subcommand or session id`). Prefix match on the 8-char suffix — `8v log show ses_01HW` resolves if unambiguous, errors if not (no silent fallback).

### 4.1 Default — `8v log`

Sessions list, last 10 newest first:

```
  id          when                 dur    cmds  fail  out     tokens  agent
  ses_a1f3    2026-04-17 14:32     15m     42     4   34KB     8.5K   claude-code 2.1.112
  ses_9c02    2026-04-17 11:08     42m    108     0   210KB   52.4K   claude-code 2.1.112
  ses_7e4d    2026-04-17 09:50      3m      9     2   4KB      1.0K   -
  ses_4b11    2026-04-16 22:14   1h12m    231    19   980KB  245.0K   codex 0.121.0
  ...  (showing 10 of 87, --all to see all, --limit N to change)

blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.
```

Columns: `out` (total `output_bytes` for the session) and `tokens` (sum of `token_estimate`) are the actual thrash indicators — more useful at a glance than per-command averages.

### 4.2 Drill-in — `8v log last` or `8v log show <id>`

Compact summary of one session:

```
session ses_a1f3   2026-04-17 14:32-14:47  (15m)  claude-code 2.1.112  mcp
project /Users/.../oss/8v
---
  42 commands   38 ok   4 fail   p50 5ms  p95 240ms   34KB out

top commands     read 18  write 9  search 7  check 4  ls 4
failures         write x3   search x1
retries          read src/main.rs --full   x4 in 90s
                 write handler.rs --find … x3 consecutive fail

blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.
```

Note: latency shown as p50/p95, **not** avg — avg is dominated by 0-1ms noise commands (`ls`, short `read`).

Add `-v`/`--verbose` for per-command table:

```
session ses_a1f3  2026-04-17 14:32-14:47  claude-code 2.1.112

  #   time   cmd     argv (truncated)                    ms    ok
  1   14:32  ls      --tree --loc                         5    ok
  2   14:32  read    src/main.rs                          2    ok
  3   14:33  read    src/main.rs --full                  12    ok
  4   14:33  read    src/main.rs --full                  11    ok   <- retry
  5   14:33  read    src/main.rs --full                  10    ok   <- retry
  6   14:34  search  "ContainmentRoot" -e rs             41    ok
  7   14:34  write   handler.rs --find "foo" --rep...     8    FAIL not found
  8   14:35  write   handler.rs --find "foo " --re...     9    FAIL not found
  9   14:35  write   handler.rs --find "foo\n" --r...     7    FAIL not found
 10   14:36  read    handler.rs 42-60                     3    ok
```

### 4.3 Failures-only view — `--failures`

A *view* flag valid on single-session drill-in and on `search`. For multi-session filtering use `8v log --has-failures` (boolean filter on the sessions list).

```
$ 8v log show ses_a1f3 --failures
3 failures, 1 cluster

cluster #1  write --find/--replace on handler.rs  (3 consecutive)
  14:34  write handler.rs --find "foo"   -> error: pattern not found
  14:35  write handler.rs --find "foo "  -> error: pattern not found
  14:35  write handler.rs --find "foo\n" -> error: pattern not found
```

(No "resolved-by" line — that heuristic produced false positives and is dropped from v1.)

### 4.4 Retries — `--retries`

View flag, single-session:

```
$ 8v log last --retries
read  src/main.rs --full   x4   44KB output
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

### 4.6 JSON — `--json` (any subcommand)

Structured output. `8v log -f --json` streams one event-or-record per line (NDJSON), which matches the input format and is directly tail-able by user #2.

```
$ 8v log show ses_a1f3 --json | jq '.summary'
{
  "session_id": "ses_a1f3...",
  "caller": "mcp",
  "agent":   { "name": "claude-code", "version": "2.1.112" },
  "started_ms": 1776435147000,
  "ended_ms":   1776436050000,
  "commands": 42, "ok": 38, "fail": 4,
  "p50_ms": 5, "p95_ms": 240,
  "output_bytes_total": 34816,
  "top_commands": [["read",18],["write",9],["search",7]],
  "clusters": [
    { "kind": "retry",   "command": "read",  "argv_hash": "...", "count": 4 },
    { "kind": "failure", "command": "write", "path": "handler.rs", "count": 3 }
  ]
}
```

## 5. Flags

**Common filters** (apply on every subcommand):

| flag | behavior |
|---|---|
| `--json` | structured output (NDJSON when combined with `-f`) |
| `--since <dur>` | e.g. `1h`, `2d`. Relative to **wall-clock at invocation**. |
| `--until <dur\|time>` | closes the window (for reproducibility in CI/scripts) |
| `--on <date>` | calendar day in **local time** (e.g. `2026-04-16`); see §6.2 for DST rule |
| `--project <path>` | filter by `project_path` |
| `--caller cli\|mcp` | filter by entry point |
| `--agent <name>` | filter by `agent_info.name` — **implies `--caller mcp`** (CLI events have no agent) |

**Selection** (mutually exclusive — enforced by clap ArgGroup "scope"):

| flag | behavior |
|---|---|
| (default, `8v log`) | sessions list |
| `8v log last` | most recent session |
| `8v log show <id>` | specific session |
| `--all` | every session (only valid with sessions list or `search`) |
| `--limit N` | cap count (sessions list, search) |
| `-f` / `--follow` | tail mode (cannot combine with `--all` or `show`) |

**Views** (apply to drill-in and search as noted):

| flag | scope | behavior |
|---|---|---|
| (none) | drill-in | compact summary |
| `-v` / `--verbose` | drill-in | per-command table |
| `--failures` | drill-in, search | failing commands + clusters |
| `--has-failures` | sessions list | boolean filter |
| `--retries` | drill-in | repeated argv only |

**Search-only:**

| flag | behavior |
|---|---|
| `--files` | unique matched paths instead of events |
| `-e <ext>` | filter argv tokens by extension |
| `-C N` | N events of context around each match |

## 6. Signals — how each is computed

All derived purely from `CommandStarted` + `CommandCompleted` joined on `run_id`, scoped by `session_id`.

| signal | computation |
|---|---|
| command count / ok / fail / incomplete | count rows, `success` field; Started-without-Completed → `incomplete` bucket (see §6.2) |
| p50 / p95 `duration_ms` | from `CommandCompleted.duration_ms` (monotonic, authoritative — never from timestamp subtraction). **Avg is never shown.** When `n < 5`, render `-` instead of a number. |
| top commands | histogram of `command`; omit row if count = 0 |
| retries | for each `(command, normalized-argv)` pair: if it occurs `≥ 2` times with first-to-last span `≤ --retry-window` (default **30s**), it's a retry cluster. **Does not require successive events** — interleaved commands between retries don't break the cluster. |
| failure clusters | for each `(command, normalized-argv, project_path)` pair: if `success=false` occurs `≥ 2` times with first-to-last span `≤ --retry-window`, regardless of non-matching events in between. |
| re-reads | for `read`-family argv, distinct path tokens + occurrence counts |

**Argv normalization (explicit spec):**
- `project_path` canonicalized via `std::fs::canonicalize` at read time (so `/Users/x/proj` and `/private/var/…/proj` macOS realpath collapse to one key).
- Paths inside canonical `project_path` → kept verbatim (relativized to it).
- Paths outside `project_path` → replaced by `<abs>`.
- Tempdir-looking paths (`/tmp`, `/var/folders/…`, `$TMPDIR`) → replaced by `<tmp>`.
- Path separators normalized to `/` (so Windows logs cluster with POSIX).
- When `project_path = None` (workspace resolution failed at emit time), fall back to basename-only matching for `read`/`write` argv — cross-session search still works on the filename.
- Quoted string values (e.g. `--find "foo "`) → replaced by `<str>` in "argv-shape" for clustering; exact string preserved in `-v` output.
- Nothing else normalized.

### 6.1 Single-pass computation

The reader makes **one linear pass** over `events.ndjson`, streaming events, and builds every §6 aggregate online. Memory is O(sessions × distinct argv-shapes × distinct paths), **not** O(events). A 10K-event session stays flat in RAM. Implementation must preserve this — a reviewer flagging "re-scan per signal" blocks merge.

### 6.2 Edge cases

| case | rule |
|---|---|
| Empty session (Started only, no commands dispatched) | show in sessions list as `-` for stats, `0 cmds`; do not compute p50/p95 |
| Single-command session | counts shown; p50/p95 rendered `-` (n<5 threshold) |
| Session where all commands are identical | retries cluster works correctly; top-commands shows one row at 100% |
| Session crossing local midnight | session belongs to the local day of its **first `CommandStarted`**; `--on` bucket uses that day |
| DST transition | handled via `chrono::Local`; fall-back-hour events assigned to the first occurrence of the ambiguous time |
| Orphan `CommandStarted` (no `CommandCompleted`) | counted in the `incomplete` bucket. For `-f`, apply a 500ms grace window before declaring orphan. `incomplete > 0` is surfaced in summary line (`42 ok 4 fail 1 incomplete`). |
| Duplicate `CommandStarted` for same `run_id` | first wins; subsequent logged to stderr as warning; matching `CommandCompleted` pairs with the first |
| Duplicate `CommandCompleted` for same `run_id` | treated as corrupt; first kept, subsequent warned |
| Malformed line in `events.ndjson` | **skipped with stderr warning** (`line N: skipped, invalid JSON: …`); exit 0. `--strict` restores hard-fail (matches existing `event_reader.rs` behavior). |
| Clock skew across hosts (NFS-mounted `~/.8v`) | duration from `CommandCompleted.duration_ms` (authoritative); event ordering by **file offset**, not timestamp; skew surfaced in blind-spots footer |
| File rotation during `-f` | follow by inode; on inode change or size shrink, re-open from start and resume; emit `{"event":"LogRotated"}` marker on the `--json` stream |
| `--limit N` with filters | filters applied first, then limit. `(showing 10 of 87)` counts post-filter, pre-limit. `--all --has-failures` does a full scan. |

**Signals dropped from v1** (previously proposed, cut after review):
- `stuck-point` — redundant with failure clusters.
- `resolved-by` — heuristic produced false positives (any subsequent `read` on a path counted as resolution, even when the fix never happened).

## 7. Blind spots (surfaced explicitly in every view)

- Native tool calls (Read/Edit/Bash outside 8v) — invisible.
- Write "success" that was semantically wrong (compiled but broke a test) — we record write ok; test failure is a later event the agent might not attribute.
- Multi-machine sessions — `session_id` is per-process, not per-agent-conversation.
- Clock skew across hosts — when `~/.8v/events.ndjson` is on NFS/shared storage and written by multiple machines, `timestamp_ms` is not monotonic. Durations use `CommandCompleted.duration_ms` (host-local monotonic), ordering uses file offset; `--on` / `--since` bucketing still uses `timestamp_ms` and will be wrong for skewed hosts.
- Malformed lines in `events.ndjson` — skipped with a stderr warning (or `--strict` to hard-fail).

Listed in the footer of every human-rendered view (not just JSON).

## 8. Out of scope (v1)

- Dashboards, Grafana, time-series DB.
- Multi-day aggregate trend reports (possible via `--all --json | jq`, but no built-in view).
- In-loop agent feedback.
- Any network / cloud.
- Write to the log — `8v log` is read-only over `events.ndjson`.
- CI gating on exit code — deferred until there's a real user for it.

## 9. Files touched when implementation begins

- `o8v-core/src/events/lifecycle.rs` — add `session_id: String` to `CommandStarted` + `CommandCompleted`.
- `o8v/src/main.rs` — generate CLI session id.
- `o8v/src/mcp/mod.rs` + handler — generate MCP session id on `initialize`.
- `o8v/src/dispatch.rs` — thread session id through `CommandContext`.
- `o8v/src/commands/log/` — new subcommand module: `mod.rs`, `sessions.rs`, `drill.rs`, `search.rs`.
- `o8v-core/src/render/log_report.rs` — report types + Renderable impls (human tables and JSON).
- `o8v/src/commands/mod.rs` — dispatch arm.
- `o8v/src/event_reader.rs` — extend to group by `session_id`, compute signals, implement argv normalization.
- Tests: `o8v/tests/e2e_log.rs` (new E2E file), unit tests under each touched module.

## 10. Open questions (must resolve before code)

1. **Retry window default.** Proposal: **30s** (matches observed failure burst timings in `~/.8v/events.ndjson`), configurable via `--retry-window`. ← founder confirm.
2. **Failure clustering shape.** Normalized argv (per §6), exact shown under `-v`. ← founder confirm.
3. **`--agent` filter on CLI rows.** Proposal: implies `--caller mcp`; CLI rows filtered out with no warning. Alternative: also match `"-"` to allow `--agent -` for CLI. ← founder decide.

(Previously-listed questions on session id format, exit codes, and "leakage" are resolved or withdrawn: ULID chosen, exit codes deferred, privacy axis was already-logged fields, not the new `session_id`.)
