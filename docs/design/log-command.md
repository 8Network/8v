# `8v log` — Design

**Status:** Draft. Under adversarial review. No code until 3 review rounds come back empty and the feature freeze (2026-04-14) lifts.
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

This is the only schema change. No daemon, no index file, no new event types. The session id is a ULID or UUIDv7 so it sorts lexicographically by creation time.

## 4. Surface

Three subcommands under `8v log`:

```
8v log                       # last session, compact (default)
8v log -v                    # last session, per-command table
8v log sessions              # list of sessions
8v log <session_id>          # drill into one
8v log search <query>        # cross-session search
8v log -f                    # live tail
```

Prefix match on session ids — `8v log ses_a1` resolves if unambiguous, errors if not (no silent fallback).

### 4.1 Default — `8v log`

```
session  2026-04-17 14:32-14:47  (15m)  claude-code 2.1.112  mcp
         project /Users/.../oss/8v
---
  42 commands   38 ok   4 fail   avg 84ms   34KB out

top commands     read 18  write 9  search 7  check 4  ls 4
failures         write x3   search x1
retries          read src/main.rs --full   x4 in 90s
                 write handler.rs --find ... x3 consecutive fail
```

### 4.2 Verbose — `8v log -v`

```
session ses_01HW4K...  2026-04-17 14:32-14:47  claude-code 2.1.112

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

### 4.3 Sessions list — `8v log sessions`

```
  id          when                 dur    cmds  fail  caller  agent                 project
  ses_a1f3    2026-04-17 14:32     15m     42     4   mcp     claude-code 2.1.112   oss/8v
  ses_9c02    2026-04-17 11:08     42m    108     0   mcp     claude-code 2.1.112   oss/8v
  ses_7e4d    2026-04-17 09:50      3m      9     2   cli     -                     oss/8v
  ses_4b11    2026-04-16 22:14   1h12m    231    19   mcp     codex 0.121.0         products/self
  ... (showing 10 of 87, --all to see all, --limit N to change)
```

Default: last 10, newest first.

### 4.4 Failures-only — `8v log --failures`

```
3 failures, 1 cluster

cluster #1  write --find/--replace on handler.rs  (3 consecutive)
  14:34  write handler.rs --find "foo"   -> error: pattern not found
  14:35  write handler.rs --find "foo "  -> error: pattern not found
  14:35  write handler.rs --find "foo\n" -> error: pattern not found
  resolved by:  read handler.rs 42-60  ->  write handler.rs:45 "..."
```

### 4.5 Retries — `8v log --retries`

```
read  src/main.rs --full   x4   session total 44KB output wasted
```

### 4.6 Search — `8v log search <query>`

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

### 4.7 JSON — `8v log --json`

```
$ 8v log --json | jq '.sessions[0].summary'
{
  "session_id": "ses_01HW4K...",
  "caller": "mcp",
  "agent":   { "name": "claude-code", "version": "2.1.112" },
  "started_ms": 1776435147000,
  "ended_ms":   1776436050000,
  "commands": 42, "ok": 38, "fail": 4,
  "top_commands": [["read",18],["write",9],["search",7]],
  "clusters": [
    { "kind": "retry",   "command": "read",  "argv_hash": "...", "count": 4 },
    { "kind": "failure", "command": "write", "path": "handler.rs", "count": 3 }
  ]
}
```

## 5. Flags

Common to all subcommands:

| flag | behavior |
|---|---|
| `--json` | structured output for scripting |
| `--since <duration>` | time window (e.g. `1h`, `2d`) |
| `--on <date>` | calendar day (e.g. `2026-04-16`) |
| `--project <path>` | filter by `project_path` |
| `--caller cli\|mcp` | filter by entry point |
| `--agent <name>` | filter by `agent_info.name` |

Scope / selection:

| flag | behavior |
|---|---|
| (none, `8v log`) | last session |
| `--session <id>` / `<id>` positional | specific session |
| `--all` | every session |
| `--limit N` | cap count |
| `-f` / `--follow` | tail mode |

Views:

| flag | behavior |
|---|---|
| (none) | compact |
| `-v` / `--verbose` | per-command table |
| `--failures` | only failing commands + clusters |
| `--retries` | only repeated argv |
| `--files` | unique matched paths (search only) |
| `-e <ext>` | filter argv by extension |
| `-C N` | N events of context around each match (search) |

## 6. Signals — how each is computed

All derived purely from `CommandStarted` + `CommandCompleted` joined on `run_id`, scoped by `session_id`.

| signal | computation |
|---|---|
| command count / ok / fail | count rows, `success` field |
| avg / p95 `duration_ms` | from `CommandCompleted.duration_ms` |
| top commands | histogram of `command` |
| retries | group successive `CommandStarted` where `command` + normalized argv (strip volatile tokens: absolute paths outside `project_path`, timestamps, tempdirs) repeat within a time window |
| failure clusters | per `(command, argv-shape, project_path)` count `success=false` in a row |
| stuck-point | longest consecutive failure run on same `project_path` with no success on any command in that project |
| re-reads | for `read`-family argv, distinct path tokens + occurrence counts |
| resolved-by | for a failure cluster, first success on an argv that references any path from the cluster |

## 7. Blind spots (surfaced explicitly)

- Native tool calls (native Read/Edit/Bash outside 8v) — invisible to us.
- Write "success" that the agent perceived as wrong (compiled but broke a test) — we record write ok; test failure is a later event the agent might not attribute.
- Multi-machine sessions — `session_id` is per-process, not per-agent-conversation.

Each blind spot is listed in the footer of `8v log` output so the reader knows what the number *can't* tell them.

## 8. Out of scope (v1)

- Dashboards, Grafana, time-series DB.
- Multi-day aggregate trend reports (possible via `--all --json | jq`, but no built-in view).
- In-loop agent feedback.
- Any network / cloud.
- Write to the log — `8v log` is read-only over `events.ndjson`.

## 9. Files touched when implementation begins

- `o8v-core/src/events/lifecycle.rs` — add `session_id: String` to `CommandStarted` + `CommandCompleted`.
- `o8v/src/main.rs` — generate CLI session id.
- `o8v/src/mcp/mod.rs` (+ handler) — generate MCP session id on `initialize`.
- `o8v/src/dispatch.rs` — thread session id through `CommandContext`.
- `o8v/src/commands/log.rs` — new subcommand module (default, sessions, search, failures/retries views).
- `o8v-core/src/render/log_report.rs` — report types + Renderable impls (both human tables and JSON).
- `o8v/src/commands/mod.rs` — dispatch arm.
- `o8v/src/event_reader.rs` — extend to group by `session_id`, compute signals.
- Tests: `o8v/tests/e2e_log.rs` (new E2E file), unit tests under each touched module.

## 10. Open questions (must resolve before code)

1. **Session id format.** ULID (26 chars, sortable, human-glanceable) vs UUIDv7 (36 chars, standard). Recommend ULID; prefix `ses_` for namespace. → founder decision.
2. **Retry window.** What time gap distinguishes "retry" from "re-use"? Proposal: 2 minutes, configurable via `--retry-window`.
3. **Failure clustering.** Same `argv` exactly, or normalized? Normalized lets `write … --find "foo"` and `write … --find "foo "` cluster together (useful); exact is safer. Proposal: normalized, show exact under `-v`.
4. **Exit code.** Should `8v log` ever exit non-zero? Proposal: yes, if `--failures` returns any rows and `--exit-on-failure` is passed (CI use). Otherwise always 0.
5. **Session id leakage.** We stamp `session_id` on every event. Is that OK to write to disk unconditionally, or do some callers need an opt-out? Proposal: always on, no opt-out — the field is random bytes, not PII.
