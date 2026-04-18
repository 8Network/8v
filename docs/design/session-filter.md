# `--session` filter for `8v log` and `8v stats` — Level 1 Feature Design

**Status:** Draft. Level 1 only (what + why). No implementation until adversarial review returns empty.
**Scope:** `8v log` and `8v stats` flags only. No schema changes, no new storage.
**Depends on:** `log-command.md`, `stats-command.md` (both at rounds 1-4 applied). Specifically, the `SessionId` type in `o8v-core/src/types/session_id.rs` and the `session_id` field already threaded through every event (log §3).

---

## 1. Problem

During a multi-day dogfood session, the founder could list all sessions with `8v log` and see their session ids. But when a specific session looked interesting — a run with many failures, or a long-duration MCP session — there was no way to drill aggregate stats for that one session. `8v stats` aggregates across a time window, not a session. `8v log show <id>` shows a session summary but not the full per-command stats breakdown (p50/p95/p99, argv-shape failure hotspots) that `8v stats` provides.

The gap: **cross-cutting stats for one session require eyeballing the drill-in summary and the raw event log in parallel.** No single command answers "what were the p95 durations and failure hotspots for session ses_a1f3?"

This is a visibility gap in the existing design, not a missing feature class. The session id is already on every event. The filter is not.

---

## 2. Scope

Add `--session <id>` as a common filter flag to both `8v log` and `8v stats`.

The flag accepts a full, valid `SessionId` — the `ses_<26 Crockford-base32 chars>` format defined in `o8v-core/src/types/session_id.rs`. The same `SessionId::try_from_raw` validation that gates all other session-id parsing applies here; an invalid format is a parse error, not an unknown-session error.

The flag slots into the existing common-filters table in `log-command.md §5` and `stats-command.md §5` — it is a filter on the event stream, not a new surface or subcommand. It does not introduce a new positional argument, a new output shape, or a new subcommand. It narrows the set of events before any existing view logic runs.

---

## 3. Behavior

### 3.1 `8v log --session <id>`

Without `--session`, `8v log` shows the sessions list (default: 10 most recent). With `--session <id>`, the sessions-list degenerates to a single-row view for that session, then **automatically drills in** — equivalent to `8v log show <id>`. The user gets the drill-in summary without having to type a second command.

The degenerate sessions-list row is not shown; the output is the drill-in view directly. This matches the intent: if you specify a session, you want to see it, not a list of one.

All view flags from `log §5` continue to apply: `--failures`, `--retries`, `-v/--verbose`, `--json`. The `--session` flag provides the scope; the view flags control what is shown within that scope.

### 3.2 `8v stats --session <id>`

`--session <id>` filters the event stream to only events with matching `session_id` before aggregation. The result is the same per-command table (§3.1 of stats-command.md), argv-shape drill (§3.2), or compare view (§3.3) — scoped to that session's events.

The `window:` header line changes from a date range to:

```
session: ses_a1f3  2026-04-17 14:32-14:47  (15m)  42 commands
```

If the session spans multiple calendar days (a long MCP session), the actual start/end timestamps are shown; no day-boundary truncation.

### 3.3 Unknown session

If `--session <id>` is valid in format but matches no events in `~/.8v/events.ndjson`, both commands apply the existing empty-window rule:

> Print `no matching events` to **stderr**, exit **2**. (stats-command.md §6, "Empty window" row.)

This applies equally to `8v log --session <unknown>` and `8v stats --session <unknown>`. No silent empty result, no guessing.

---

## 4. Flag interactions

The following interactions are decided here and must be implemented consistently in both commands.

### `--session` + `--since` / `--until` / `--on`

Session wins. A session has a fixed time span. Applying a time window that partially overlaps would silently truncate the session's events, which is surprising and wrong. When `--session` is present, `--since`, `--until`, and `--on` are ignored with a stderr notice:

```
warning: --since ignored when --session is set (session has a fixed time span)
```

This matches the principle from `log §5`: narrower filter takes precedence, and the narrow filter must not silently discard the user's intent.

### `--session` + `--project`

AND'd. If the session's events span multiple projects (possible when a single MCP session switches working directories), `--project` further narrows to events in that project. If the session has no events for that project, the empty-window rule applies (stderr + exit 2). No silent empty table.

### `--session` + `--caller` / `--agent`

AND'd. Both flags are event-level filters. If the session was an MCP session and the user passes `--caller cli`, the filter returns no events → empty-window rule. The combination is allowed; the result may be empty.

### `--session` + view flags (`--failures`, `--retries`, `--verbose`)

View flags apply after session filter. `--session` provides the event scope; view flags control the presentation of that scope. No conflict.

### `--session` + `--all` / `--limit`

`--all` and `--limit` apply to the sessions-list form of `8v log`. With `--session`, the sessions-list collapses to a drill-in (§3.1), so `--all` and `--limit` are irrelevant and ignored with a stderr notice.

### `--session` + `--compare agent`

Allowed. If the session has events from multiple agents (unusual but possible in a shared-log scenario), `--compare agent` shows the per-agent breakdown within that session.

---

## 5. Non-goals

**Partial/prefix matching: no.**

`log-command.md §4` already defines prefix matching for positional session ids in `8v log show <prefix>`: "resolves if unambiguous against the set of sessions in the current `events.ndjson`, errors if not." That rule applies to the `show` subcommand only, where the user types the id as a positional argument in an interactive flow.

`--session` is a flag that accepts a full, valid `SessionId`. Partial matching on a flag is not the same interaction — it is ambiguous by design and could silently resolve to the wrong session. The `--session` flag requires the full id. Users can copy the full id from `8v log` output (the sessions-list already shows the full id). No partial/prefix matching on `--session`.

---

## 6. Acceptance criteria

A reviewer can verify each of the following without running code:

1. `8v log --session ses_<valid-ulid>` produces the same output as `8v log show ses_<valid-ulid>` when that session exists in events.ndjson.
2. `8v stats --session ses_<valid-ulid>` filters events to that session before aggregation; the `window:` header shows the session id and time span, not a date range.
3. `8v log --session ses_<valid-ulid> --failures` shows only failures within that session.
4. `8v stats --session ses_<valid-ulid> --json` emits the same JSON shape as `8v stats --json` with a `session_id` field added to the top-level object.
5. An invalid format (e.g. `--session notanid`) produces a parse error from `SessionId::try_from_raw`, not an unknown-session error.
6. A valid format that matches no events produces `no matching events` on stderr and exits 2, in both commands.
7. `--session` + `--since` → stderr warning that `--since` is ignored; the session's full event set is used.
8. `--session` + `--project` with a mismatched project → empty-window rule (stderr + exit 2, not a silent empty table).
9. `8v log --session <id> --all` → stderr notice that `--all` is ignored; the output is the drill-in view.
10. The `SessionId` validation used by `--session` is `SessionId::try_from_raw` — no duplicated parsing logic.
11. The flag appears in `--help` output for both `8v log` and `8v stats` under the common-filters section, not as a subcommand.
12. Legacy events (no `session_id` field, deserialized as empty string) are never matched by `--session`; the empty-string sentinel is excluded by `SessionId::try_from_raw` validation at the filter boundary.
