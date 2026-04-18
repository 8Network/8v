# `--session` filter — Level 2 Implementation Design

**Status:** Draft. Depends on `session-filter.md` (Level 1). No code until adversarial review clears.
**Scope:** `8v log` and `8v stats` only. No new storage, no shared filter layer.

---

## 1. Layer Ownership

| Concern | Owner | File |
|---|---|---|
| Arg parsing | clap derive field | `log/mod.rs` `Args`, `stats.rs` `Args` |
| SessionId validation | `o8v-core` type | `types/session_id.rs` `SessionId::try_from_raw` |
| Event filtering | each command independently | `log/mod.rs`, `stats.rs` |
| Warning emission | `WarningSink` | `types/warning_sink.rs` |
| Output header (stats) | render layer | `StatsReport` / `StatsView` in `o8v-core` |

No shared filter layer. Duplication is one `retain` call per command — acceptable.

---

## 2. SessionId Parsing

Both commands add `session: Option<SessionId>` to their `Args` struct. The clap `value_parser` for the field calls `SessionId::try_from_raw` directly — no second parse site anywhere.

- Invalid format → clap rejects before `execute()` is called. Message comes from `try_from_raw`, not a custom branch.
- Empty-string sentinel (legacy events missing `session_id`) is rejected by `try_from_raw` — no special-case at the filter boundary.
- Valid format, no matching events → empty-window rule in each command (stderr + exit 2). Not a parse error.

---

## 3. `8v log --session <id>` Changes

**`log/mod.rs` `Args`:** add `session: Option<SessionId>` field.

**`LogCommand::execute` insertion point:** after `aggregate_events` returns `sessions: Vec<SessionAggregate>`, before the `match &self.args.subcommand` block.

Logic when `session` is set:
1. Exact-match lookup: `sessions.iter().find(|s| s.session_id == parsed_id.as_str())`.
2. Not found → emit `no matching events` to stderr, exit 2. Use the same empty-window guard already present in stats (add equivalent to log).
3. Found → call `build_drill_report` directly with the matched aggregate. Return `LogReport::Drill`. The sessions-list path is never reached.
4. `--all` or `--limit` present → emit stderr notice via `WarningSink` that they are ignored. Do not error.

`resolve_session_prefix` (used by the `Show` subcommand) is not called here. `--session` requires exact match only.

---

## 4. `8v stats --session <id>` Changes

**`stats.rs` `Args`:** add `session: Option<SessionId>` field.

**`StatsCommand::execute` insertion point:** after events are loaded, before the time-window duration parse block (currently computes `since_dur` / `until_dur` → `windowed` retain).

Logic when `session` is set:
1. For each of `--since` / `--until` / `--on` that is non-default: push a warning to `WarningSink` — `--since ignored when --session is set (session has a fixed time span)`.
2. Skip the time-window retain entirely. Filter events with a session-id retain instead: keep only events whose `session_id` field equals `parsed_id.as_str()`.
3. Apply remaining per-event filters (`--project`, `--caller`) AND-style after the session retain, using whatever filter path those flags use (once added — see §7).
4. Pass the filtered slice to the existing `aggregate_events` → `build_report` pipeline unchanged.
5. Empty after all filters → existing `filtered_empty` path (stderr + exit 2). No new branch.

**`StatsReport` (or `StatsView`):** add `session_id: Option<String>` field. Populated when `--session` is set. Render layer uses it to replace the `window:` date-range header line with a session-scoped header showing id, start/end timestamps, duration, and command count. JSON output gains a top-level `session_id` string field (omitted when absent).

---

## 5. One-Code-Path Check

- `SessionId::try_from_raw` called in exactly two places: the `value_parser` closure in `log/mod.rs` `Args` and the `value_parser` closure in `stats.rs` `Args`. No third parse site.
- `filtered_empty` logic lives in `stats.rs` today. Log needs the equivalent added — not copied, added to its own empty-result branch.
- `build_drill_report` in log is already called by the `Show` subcommand path. The `--session` branch reuses the same function with the same signature.

---

## 6. Test Plan (failing-first, all must fail on pre-fix binary)

1. **`log_session_exact_match_produces_drill_in`** — `--session <known-id>` output matches `log show <id>` output exactly.
2. **`log_session_unknown_id_exits_2`** — valid format, no matching events → stderr `no matching events`, exit code 2.
3. **`log_session_invalid_format_is_parse_error`** — `--session notanid` → parse error text (not unknown-session), exits non-zero before execute.
4. **`log_session_ignores_limit_with_notice`** — `--session <id> --limit 5` → stderr notice, output is drill-in, not sessions list.
5. **`stats_session_filters_to_session_events_only`** — events from other sessions absent from per-command table.
6. **`stats_session_window_header_shows_session_id`** — plain output header line starts with `session:`, not `window:`.
7. **`stats_session_json_has_session_id_field`** — `--json` output has top-level `session_id` string field.
8. **`stats_session_since_flag_ignored_with_warning`** — `--session <id> --since 3d` → stderr warning, full session events used.
9. **`stats_session_unknown_id_exits_2`** — valid format, no events → stderr `no matching events`, exit 2.
10. **`legacy_events_not_matched_by_session_filter`** — events with empty `session_id` string not returned; `try_from_raw("")` is the gate.

---

## 7. Open Questions (decisions required before coding)

1. **Missing AND-filter flags.** L1 §4 specifies `--project`, `--caller`, `--agent` AND-behavior with `--session`. None of these flags exist in current `log::Args` or `stats::Args`. Options: (a) implement `--session` without AND-behavior and document the gap, or (b) add the flags — new flag surface, currently frozen. **Founder decision required.**

2. **Exit-2 mechanism in log.** `stats.rs` uses `filtered_empty: bool` on `StatsReport`; the render layer converts it to exit 2. `LogReport` has no equivalent. Either add `LogReport::Empty` variant or thread `filtered_empty` through `LogReport`. Decide before coding to avoid rework.
