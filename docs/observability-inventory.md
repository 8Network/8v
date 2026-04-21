# Observability Inventory

What `8v log` and `8v stats` can and cannot tell you. For every field, the source is
explicit. Nothing is implied.

## What we have — from the 8v event store

Every call through the 8v MCP server or CLI writes two events to `~/.8v/events.ndjson`:
`CommandStarted` and `CommandCompleted`. From these we know:

| Field | Source | Available in log/stats |
|-------|--------|----------------------|
| Command name (`ls`, `read`, `write`, `search`, …) | `CommandStarted.command` | Yes |
| Full argument vector | `CommandStarted.argv` | Yes (log show) |
| Duration (ms) | `CommandCompleted.duration_ms` | Yes (P50/P95/P99 in stats) |
| Output size (bytes) | `CommandCompleted.output_bytes` | Yes (out/call in stats) |
| Input size (bytes) | `CommandStarted.command_bytes` | Yes |
| Success or failure | `CommandCompleted.success` | Yes (ok% in stats) |
| Session ID | `CommandStarted.session_id` | Yes (log groups by session) |
| Timestamp | `CommandStarted.timestamp_ms` | Yes |
| Caller: `cli` or `mcp` | `CommandStarted.caller` | Yes |
| Agent name and version | `CommandStarted.agent_info` | Yes |
| MCP protocol version | `CommandStarted.agent_info` | Yes |
| Retry clusters | derived from consecutive same-argv calls | Yes (stats retries column) |
| Project path | `CommandStarted.project_path` | Yes |

## What we have — from the agent stream parser

The benchmark and `8v log` capture Claude's tool stream via `--output-format stream-json`.
This gives us the agent-side view of every tool call:

| Field | Source | Available |
|-------|--------|-----------|
| Tool name (`Bash`, `Read`, `Edit`, `mcp__8v__8v`, …) | agent stream | Yes (tool histogram in report) |
| Tool input (full JSON) | agent stream | Yes (tool_calls_detail in report.json) |
| Tool output size (bytes) | agent stream | Yes (tool_calls_detail) |
| Tool error flag | agent stream | Yes (tool_calls_detail) |
| Token usage per turn | agent stream | Yes (breakdown in report) |
| Cache read / cache creation tokens | agent stream | Yes |
| Total cost (USD) | agent stream | Yes |
| Model ID | agent stream | Yes |
| Claude session ID | agent stream | Yes |
| Stop reason | agent stream | Yes |

## What we cannot measure

These are outside the system. They are not gaps to close — they are architectural
limits of the agent boundary. We know they exist; we do not guess at them.

| Field | Why we cannot measure it |
|-------|--------------------------|
| Per-tool-call cost | Anthropic only gives cost per turn, not per tool within a turn |
| Per-tool-call token attribution | Same — token counts are per-turn totals |
| Why the agent chose a tool | Model reasoning is opaque; we see inputs and outputs only |
| Whether the agent understood the problem | We can measure outcome (tests pass/fail), not comprehension |
| Cache hit/miss per call | We have session-level cache_read/creation totals; no per-call breakdown |
| Agent internal state between turns | Not exposed by any API |

## How they connect

An `mcp__8v__8v` entry in `tool_calls_detail` (agent stream) corresponds to one
`CommandStarted` + `CommandCompleted` pair in `events.ndjson` (8v store).

The link: timing. Each `CommandStarted.timestamp_ms` falls inside the turn window where
the agent called `mcp__8v__8v`. There is no shared ID — the agent stream and the event
store are separate pipes that agree only on time.

## What was broken (fixed 2026-04-21)

The benchmark pipeline deleted `events.ndjson` before every run. This meant:
- Zero benchmark events in `8v log` and `8v stats`
- A parallel per-run aggregator in `collect_events()` that duplicated (poorly) what
  `log`/`stats` already do
- No session history for any benchmark run

**Fix:** timestamp-based isolation replaces file deletion. `collect_events()` now filters
events by `timestamp_ms >= run_start_ms`. Events accumulate. Every benchmark run is a
session visible in `8v log`.
