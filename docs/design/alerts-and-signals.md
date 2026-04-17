# `8v alerts` + Signal Surface — DRAFT

**Status:** **DRAFT.** First pass, no review rounds yet. Not ready to implement. Purpose: capture the multidimensional observability framing before it drifts.
**Siblings:** `log-command.md` (temporal reader), `stats-command.md` (aggregates). This doc proposes the **active detection** layer on top of both.
**Depends on:** `session_id` schema change from `log-command.md` §3.

## 1. Problem

`log` and `stats` are passive readers. They answer questions if you ask. They don't say "something broke" on their own.

What's missing: **named conditions that fire when crossed**. Not "here's a table with a failure rate column" — "**8v: write --find/--replace fail rate jumped from 4% to 22% after commit a1f3c91**". That's a detection, not an aggregate. The user reads one line and knows what to fix.

Also missing: **signals we could emit about ourselves but don't**. 8v already knows its schema size in tokens, how many tool calls happened in a session, the input side of each call (not just output). That data is free — we're the ones running. If it's not in `events.ndjson`, it's our fault, not the agent's opacity.

Three users, three feedback loops:

1. **Feedback to the founder.** "Your last release regressed `check` p95 by 3×." Actionable, post-hoc.
2. **Feedback to the user** (human running agents). "This session had 14 `write --find/--replace` failures on the same file — the agent is stuck." Actionable, near-real-time.
3. **Feedback to the agent** (in-loop). "`read` already returned this file 3 times — consider using the cached output." Actionable, in-session. **Still out of scope for v1**, but the data needs to be there so this feature can land later without another schema change.

Observability is multidimensional. `log` and `stats` cover axis 1 (what happened, in aggregate). This doc covers axis 2 (**named detections**) and axis 3 (**self-emitted signals we should already be logging**).

## 2. Two things, one doc

### 2A. Signal surface expansion (schema change)

What 8v can emit that it currently doesn't. Each signal is cheap for us to produce and impossible for the agent to reconstruct.

| signal | source | event / field | why |
|---|---|---|---|
| **MCP schema size in tokens** | MCP server knows the tool description it sent | new event: `SchemaEmitted { session_id, tokens, bytes }` emitted once on `initialize` | directly measures the per-session tax the agent pays just to have 8v available |
| **Input tokens per call** | the command string + argv the agent sent | extend `CommandStarted` with `input_tokens: u64` | today we log `output_bytes` / `token_estimate` but not input; one-sided accounting |
| **Per-call turn counter** | session state in MCP handler | extend `CommandStarted` with `turn_in_session: u32` (monotonic within session) | answers "the agent took 47 turns to do this" without needing the agent's own logs |
| **Caller identity (full)** | already have `caller`, `agent_info` — add transport + pid | extend `CommandStarted` with `transport: "stdio\|socket\|…"`, `pid: u32` | disambiguates parallel MCP sessions on the same host |
| **Project detection outcome** | `o8v-project` runs during command anyway | extend `CommandStarted` with `project_stack: Option<String>` | lets `stats --compare stack` land without a read-time project re-resolution |
| **Command error class** | we produce `CommandError::{Execution, …}` | extend `CommandCompleted` with `error_class: Option<String>` | "not found" vs "permission denied" vs "timeout" — same `success=false` today, different fixes |

All fields gated by `#[serde(default)]` for backward compat (same rule as `session_id`).

### 2B. `8v alerts` — named detections

A small catalog of detections, each a pure function of the signal surface. Each detection has:
- a **name** (`write_find_replace_high_fail_rate`)
- a **query** over events (spec'd as a shared computation, like log §6 / stats §4)
- a **threshold** (can be default or user-configured in `~/.8v/alerts.toml`)
- a **recommendation** (one-line human-readable suggestion)
- a **target audience** (`founder` / `user` / `agent`)

Example:

```
$ 8v alerts
window: last 7d  18 sessions  2,341 commands

[high] write_find_replace_fail_rate
  14 sessions affected, overall 22% fail rate (threshold: 10%)
  top path: handler.rs (9 failures)
  recommendation: --find/--replace errors now include a closest-match hint since e3f610d.
                  Check if agents are on an older build.

[medium] read_full_reuse
  session ses_a1f3: read src/main.rs --full x4 in 90s (44KB wasted)
  recommendation: surface a "cached" flag in read output, or switch the agent to
                  ranged reads.

[low] schema_size_trend
  MCP schema: 3.1KB (was 2.8KB 7d ago, +10%). Within budget.
  recommendation: none.

[sla] check_p95_latency                   [VIOLATED]
  target: p95 < 500ms   actual: 1,810ms   window: 7d
  regression first seen: ses_4b11 (2026-04-16 22:14)
  recommendation: bisect commits since 2026-04-16 against the fixture set.
```

Severities: `[high]`, `[medium]`, `[low]`, `[sla]`. Exit code non-zero if any `[high]` or `[sla]` fires — this is the one place `8v`'s CLI can gate CI on the log, which `log` and `stats` deliberately don't.

### 2C. SLAs — `~/.8v/alerts.toml`

Small config. One file, checked into the repo if the founder wants CI enforcement.

```toml
# Latency targets
[[sla]]
name = "check_p95_latency"
command = "check"
metric = "p95_duration_ms"
target_lt = 500
window = "7d"

# Behavioral targets
[[sla]]
name = "write_find_replace_ok_rate"
command = "write"
argv_shape_contains = "--find"
metric = "ok_rate"
target_gt = 0.90
window = "7d"

# Size targets
[[sla]]
name = "mcp_schema_size_tokens"
event = "SchemaEmitted"
field = "tokens"
target_lt = 1000
```

Each SLA becomes an alert. Violations surface in `8v alerts` and `8v alerts --json`.

## 3. Catalog (v1 starter set)

Exactly the detections we can write today, no more. Extensible later.

| name | severity | source signals | notes |
|---|---|---|---|
| `write_find_replace_fail_rate` | high | stats failure hotspots | threshold 10% default |
| `read_full_reuse` | medium | log §6 re-reads | surfaces per-session waste |
| `consecutive_failure_cluster` | high | log failure clusters | ≥ 3 in a row |
| `schema_size_trend` | low | `SchemaEmitted` week-over-week | purely informational |
| `session_token_budget` | low | sum `token_estimate` + `input_tokens` per session | per-session context-cost view |
| `caller_mismatch_regression` | medium | cross-agent stats (`stats --compare agent`) | when one agent's ok% drops while others' stays stable |
| `sla_<name>` | sla | `~/.8v/alerts.toml` | user-defined |

Detections are code, not config (except SLAs). The catalog grows with discovery. Each detection is a named test you can run on any slice of the log.

## 4. Surface

```
8v alerts                    # all currently-firing alerts (DEFAULT)
8v alerts <name>             # drill into one alert's matching events
8v alerts list               # list the catalog (names + descriptions, firing or not)
8v alerts --sla-only         # only SLA violations (CI gate shape)
8v alerts --json             # structured
```

Shared flags from `log` + `stats` (`--since`, `--project`, `--agent`, `--strict`, etc.).

## 5. What lands first vs later

**v1 (when freeze lifts):**
- Signal surface fields (2A) — schema change, one commit, additive.
- `8v alerts` with 3 detections: `write_find_replace_fail_rate`, `consecutive_failure_cluster`, `read_full_reuse`.
- `alerts.toml` loader with 2 SLA kinds (`p95_duration_ms`, `ok_rate`).

**v2 (after dogfood feedback):**
- Cross-agent regression detector.
- Alert suppression / baseline comparison (diff this week vs last week).
- Alert history (firings written back into `events.ndjson` as `AlertFired` events — closes the loop).

**Still out of scope:**
- Agent in-loop feedback (axis 3). Requires an MCP push channel; 8v's current MCP transport is request-reply. Parked.
- Anything networked.

## 6. Open questions (need dispositioning — don't implement yet)

1. **Is `alerts` a subcommand of `log`, of `stats`, or top-level?** Argument for top-level: different verb, different user-facing primitive. Argument for `log alerts`: it's one more view on the event log. Lean top-level, same reason `log` and `stats` split.
2. **Alert firings as events?** If `AlertFired` is itself written to `events.ndjson`, you can query for "when did this alert first fire, relative to which commit". Cleaner, but another event type.
3. **Per-project vs global config?** `.8v/alerts.toml` in the project, or `~/.8v/alerts.toml` global, or both merged? Lean both.
4. **Bisect hint in `[sla]` output.** The sample above says "first seen: ses_4b11". Computing that requires a linear scan per alert — fine for v1, worth noting if the log grows.
5. **Token accounting honesty.** `input_tokens` on `CommandStarted` would be our estimate (number of bytes / 4, or tiktoken if we bundle it). Not the agent's real billed tokens. Document the difference or risk confusion with provider invoices.
6. **Do we reach backwards?** Pre-`session_id` events have no `session_id` and no schema size. Alerts on legacy data will under-report. Either exclude legacy from alerts (clean, explicit) or include with a "partial" flag. Lean exclude.

## 7. Framing

- `log` = *what happened* (temporal).
- `stats` = *how often / how fast* (aggregate).
- `alerts` = *something is wrong / some target was missed* (detection).

Three verbs, three primitives, one event log underneath. Progressive: `log` is what a user types when they want to see; `stats` when they want to measure; `alerts` when they want to be told without asking.

---

**Next step (when founder is ready):** Round 1 adversarial review of this draft. Current doc has no review rounds — it's a starting point, not a decided design.
