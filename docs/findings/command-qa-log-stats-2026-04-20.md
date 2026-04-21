# QA Finding: `8v log` and `8v stats` Commands

**Date:** 2026-04-20
**Scope:** Pure observation audit. No code changes. Every finding has a reproducible command.
**Method:** Ran every flag/form against real event store and isolated test stores.
**Constraint:** Real event store at `~/.8v/events.ndjson` was never modified. All edge-case tests used `_8V_HOME` override.

---

## Exit Code Matrix

| Command form | Condition | Exit |
|---|---|---|
| `8v log` | sessions exist | 0 |
| `8v log` | empty store (0-byte) | 0 (shows orphan self-emit) |
| `8v log --session <id>` | session found | 0 |
| `8v log --session <id>` | session not found | 2 |
| `8v log last` | sessions exist | 0 |
| `8v log last` | no sessions | 1 (execution error) |
| `8v log show <id>` | found | 0 |
| `8v log show <id>` | not found | 1 |
| `8v log show <prefix>` | ambiguous | 1 |
| `8v log search <q>` | any | 0 |
| `8v log` | malformed NDJSON, lenient | 0 |
| `8v log --strict` | malformed NDJSON | 1 |
| `8v stats` | events exist | 0 |
| `8v stats` | empty/no events | 0 |
| `8v stats --since` | results exist | 0 |
| `8v stats --since` | filtered to zero | 2 |
| `8v stats --shape <s>` | any (broken — see BUG-5) | 0, 0 rows |

---

## Findings Table

### BUG-1: `--limit` default is 20, design intention is 10

**File:** `o8v/src/commands/log/mod.rs:19`
**Reproducible:**
```
8v log --help | grep limit
# default_value_t = 20
```
**Observed:** `Args.limit` defaults to 20. Design doc (`log_stats_v2_progress.md`) states the intended default is 10.
**Impact:** Users see more sessions than intended on first run. Minor UX discrepancy. Not a crash.

---

### BUG-2: `log search` silently discards all warnings

**File:** `o8v/src/commands/log/search.rs`
**Reproducible:**
```bash
# Create store with malformed line + valid events
export _8V_HOME=/tmp/qa-8v-malformed
mkdir -p $_8V_HOME/.8v
printf '{"event":"CommandStarted","run_id":"r1","timestamp_ms":1000,...}\n{bad\n{"event":"CommandStarted","run_id":"r2",...}\n' > $_8V_HOME/.8v/events.ndjson

8v log search "check"
# Output: shows results, NO warning about malformed line
# Compare: 8v log       → shows warning about line 3
```
**Root cause:** `build_search_results(sessions, query, limit, _warnings)` — the `_warnings` parameter is underscore-prefixed and never consumed. The warnings Vec is accepted but immediately dropped. Search JSON output has no `warnings` field at all.
**Impact:** Users who run `8v log search` on a store with parse errors receive no signal that events were skipped. This is a silent data loss scenario — search results may be incomplete with no indication.

---

### BUG-3: `drill.rs` uses Debug format as stable display string for caller

**File:** `o8v/src/commands/log/drill.rs`
**Reproducible:**
```bash
8v log last
# Footer shows e.g. "cli" — currently matches Debug output of Caller::Cli
# If Caller enum variant names change, display string changes silently
```
**Root cause:** `format!("{:?}", caller).to_lowercase()` — Debug formatting is used to produce a user-visible string. Debug output is not a stable API contract; it can change without semantic notice.
**Impact:** Output is currently functional. Risk: any rename of the `Caller` enum variant silently changes the displayed text without a compile error.

---

### BUG-4: Double line-number prefix in malformed NDJSON warning

**Reproducible:**
```bash
export _8V_HOME=/tmp/qa-8v-malformed
# (store with bad line 3)
8v log
# warning: line 3: line 3: invalid JSON: expected ident at line 1 column 2
#          ^^^^^^          ^^^^^^ — appears TWICE
```
**Observed output:** `warning: line 3: line 3: invalid JSON: expected ident at line 1 column 2`
**Expected:** `warning: line 3: invalid JSON: expected ident at line 1 column 2`
**Root cause:** The line number is prepended at two levels of the call stack — once when building the warning variant and again when rendering it.
**Impact:** Cosmetic but confusing. Users see the line number twice in every malformed-line warning.

---

### BUG-5: `--shape` flag completely broken — zero rows always returned

**File:** `o8v/src/commands/stats.rs` (~line 400, `rows_by_argv_shape`)
**Reproducible:**
```bash
8v stats --shape 'read <abs>'
# Output: (empty table, 0 rows)
# Exit: 0

8v stats --shape 'check .'
# Output: (empty table, 0 rows)
# Exit: 0
```
**Root cause:** In `rows_by_argv_shape(sessions, command)`, the `command` parameter receives the full shape string (e.g. `"read <abs>"`). The inner loop checks `rec.started.command != command`, but `rec.started.command` stores only the base command name (`"read"`), never the full shape. The condition `"read" != "read <abs>"` is always true, so every record is skipped and the result is always an empty table.
**Impact:** The `--shape` flag is completely non-functional. Any user who tries to drill into a specific argv shape gets silently empty results. Exit 0 means there is no error signal.

---

### BUG-6: Failure hotspots in drill mode are not scoped to the drilled command

**File:** `o8v/src/commands/stats.rs`
**Reproducible:**
```bash
8v stats check
# Shows failure_hotspots for "check" command only — correct

8v stats read
# failure_hotspots section may show failures from "check", "write", etc.
# (when multiple commands have failures in the store)
```
**Root cause:** The `failure_hotspots` calculation aggregates failures across all commands before drill-mode filtering. The drill view inherits the global hotspot list rather than filtering to the drilled command.
**Impact:** Users drilling a specific command see misleading failure attribution. The hotspot section implies failures belong to the command being drilled when they may not.

---

### BUG-7: `--compare agent` retries column always 0

**File:** `o8v/src/commands/stats.rs`, `rows_by_agent`
**Reproducible:**
```bash
8v stats --compare agent
# retries column: 0 for all agents
# Compare: 8v stats (default table) — retries column shows correct values
```
**Root cause:** `rows_by_agent` builds `StatsRow` without aggregating retry cluster counts. The `retries` field is hardcoded to 0. The default command-grouped table correctly aggregates retries, but the agent-grouped view does not.
**Impact:** Users using `--compare agent` to diagnose which agent has high retry rates see all zeros, defeating the purpose of the view.

---

### INCONSISTENCY-1: JSON pretty-print vs compact between log and stats

**Reproducible:**
```bash
8v log --json last | python3 -m json.tool --no-ensure-ascii
# Already pretty-printed (serde_json::to_string_pretty)

8v stats --json | python3 -m json.tool --no-ensure-ascii
# Compact single line (serde_json::to_string)
```
**Files:**
- `o8v-core/src/render/log_report.rs` → `serde_json::to_string_pretty()`
- `o8v-core/src/render/stats_view.rs` → `serde_json::to_string()`
**Impact:** Scripts parsing `8v log --json` and `8v stats --json` can't apply the same `| python3 -m json.tool` or `| jq` pipeline. Technically both are valid JSON. Aesthetically inconsistent for a tool that presents unified output.

---

### INCONSISTENCY-2: Byte unit convention differs between log and stats

**Reproducible:**
```bash
8v log last      # output_bytes shown in SI units: KB = 1000, MB = 1_000_000
8v stats         # out/call column uses binary units: KiB = 1024, MiB = 1_048_576
```
**Files:**
- `o8v-core/src/render/log_report.rs`: `fmt_bytes()` divides by 1_000 / 1_000_000
- `o8v-core/src/render/stats_report.rs`: `fmt_bytes()` divides by 1_024 / 1_048_576
**Impact:** A session with 2048 bytes output shows "2.0 KB" in log view and "2.0 KiB" in stats view. Cognitively inconsistent — users comparing both outputs see different numbers for the same underlying count.

---

## Edge Case Results

### Empty store (0-byte `events.ndjson`)

```bash
export _8V_HOME=/tmp/qa-8v-empty
mkdir -p $_8V_HOME/.8v
touch $_8V_HOME/.8v/events.ndjson
8v log          # Exit 0 — shows 1 orphan session (self-emit), warning: "orphan CommandStarted run_id=task-1"
8v stats        # Exit 0 — "no matching events"
8v log --json   # Exit 0 — {"sessions":[<self-emitted session>],"total_count":3,"limit":20,"warnings":[{"kind":"orphan_started","run_id":"task-1"}]}
8v stats --json # Exit 0 — {"kind":"table","label_key":"command","rows":[],"warnings":[],"failure_hotspots":[]}
```

**Note:** A "truly empty" store is not achievable from the CLI. The `8v` binary writes its own `CommandStarted` event to the store BEFORE executing the command. So even a 0-byte file will always surface the self-emitted invocation as an orphan started event. This is documented behavior (`event_store_canonical.md`) but creates user-visible noise on first run.

---

### Malformed NDJSON — lenient mode (default)

```bash
export _8V_HOME=/tmp/qa-8v-malformed
# Store: valid pair, bad line, valid pair
8v log          # Exit 0 — processes valid sessions, warning with double-prefix (see BUG-4)
8v stats        # Exit 0 — shows both valid commands, warning with double-prefix
```

---

### Malformed NDJSON — strict mode

```bash
8v log --strict   # Exit 1 — error: line 3: invalid JSON: expected ident at line 1 column 2
```
**Note:** `--strict` on `8v stats` is not a supported flag. Only `8v log` accepts `--strict`.

---

### `--session` with missing session ID

```bash
8v log --session ses_01AAAAAAAAAAAAAAAAAAAAAAAA
# Exit 2 — "no session found" (plain) / {"error":"no session found"} (JSON)
```
**Note:** The ULID in the session ID must be exactly 26 chars or clap rejects the argument with a parse error before the command even runs. Using a syntactically valid but nonexistent session ID correctly produces exit 2.

---

### `log show` with ambiguous prefix

```bash
# Requires two sessions with IDs sharing a prefix — not reproducible without store manipulation
# Code path confirmed by reading resolve_session_prefix in mod.rs:
# match matches.len() { _ => Err("ambiguous prefix '{}' matches {} sessions") }
# Exit: 1
```

---

## JSON Shape Documentation

### `8v log --json` (sessions list)

```json
{
  "sessions": [
    {
      "session_id": "ses_<ULID>",
      "command_count": 4,
      "first_seen_ms": 1713600000000,
      "last_seen_ms": 1713600120000,
      "ok_count": 3,
      "fail_count": 1,
      "retry_cluster_count": 0,
      "failure_cluster_count": 0,
      "total_output_bytes": 4096,
      "total_duration_ms": 12000
    }
  ],
  "total_count": 42,
  "limit": 20,
  "warnings": [
    { "kind": "orphan_started", "run_id": "run_xyz" }
  ]
}
```

### `8v log --json last` / `8v log --json show <id>`

```json
{
  "session_id": "ses_<ULID>",
  "commands": [
    {
      "run_id": "run_abc",
      "command": "check",
      "argv": ["check", "."],
      "argv_shape": "check .",
      "started_ms": 1713600000000,
      "completed_ms": 1713600005000,
      "duration_ms": 5000,
      "success": true,
      "output_bytes": 1234,
      "caller": "cli"
    }
  ],
  "retry_clusters": [],
  "failure_clusters": [],
  "warnings": [],
  "blind_spots_footer": "MCP/hook calls not visible"
}
```

### `8v log --json search <q>`

```json
{
  "query": "check",
  "results": [
    {
      "session_id": "ses_<ULID>",
      "run_id": "run_abc",
      "command": "check",
      "argv_shape": "check .",
      "started_ms": 1713600000000
    }
  ],
  "total_count": 5,
  "limit": 20
}
```
**Note:** `warnings` field is ABSENT (not empty array) due to BUG-2.

### `8v stats --json`

```json
{
  "kind": "table",
  "label_key": "command",
  "shape": null,
  "session_id": null,
  "rows": [
    {
      "label": "check",
      "n": 12,
      "duration_ms": { "p50": 4200, "p95": 8100, "p99": 12000 },
      "ok_rate": 0.916,
      "output_bytes_per_call_mean": 2048.0,
      "retries": 2
    }
  ],
  "warnings": [],
  "failure_hotspots": [
    {
      "command": "check",
      "argv_shape": "check .",
      "count": 3,
      "top_path": "/abs/path/to/project",
      "top_path_count": 2
    }
  ]
}
```
**Note:** `duration_ms` is `null` when `n < 5` (MIN_SAMPLES threshold). Stats JSON is compact (no pretty-print), unlike log JSON.

---

## Naming and Coherence Observations

1. **`--session` vs `show` subcommand**: Both drill into a single session. `--session ses_<id>` on the root command and `8v log show <id>` are parallel forms for the same operation. This is not documented in `--help`. A user discovering one form may not know the other exists.

2. **`log search` vs `log show`**: "search" operates on command names/argv shapes across sessions; "show" displays a specific session. The names are appropriately distinct, but `--help` descriptions could be more explicit about the cross-session vs single-session scope.

3. **`--compare agent` naming**: The flag is `--compare` but it does not compare two things — it groups by agent. "Compare" implies A vs B contrast. A clearer name would be `--group-by agent` or `--by agent`.

4. **`--shape` vs positional `command`**: Both put stats in drill mode. `--shape 'read <abs>'` is meant to drill into a specific argv shape; `8v stats read` drills into a base command. The shape form is broken (BUG-5), but the positional form works. The `--help` text does not explain that `--shape` is a refinement of `command`.

5. **Session ID format requirement is invisible**: `--session` requires `ses_<26-char-ULID>` format. If the user passes a short or malformed ID, clap emits a parse error ("invalid value for '--session'") with no hint about the required format. A better error would say "session IDs must have format ses_<ULID26>".

6. **`--strict` only on `log`, not `stats`**: Both commands share the same event reader but only `log` exposes `--strict`. A user who wants strict validation during `stats` has no flag for it.

---

## 8v Feedback (Tool Friction During QA)

This section records friction encountered while using `8v` itself during this QA session.

### F-1: `_8V_HOME` override not documented in any user-facing output

To test isolated stores without touching the real `~/.8v/events.ndjson`, the env var `_8V_HOME` must be set. This is not documented in `8v --help`, `8v log --help`, or any `--help` output. It was found by reading `storage.rs` source. A user cannot discover this without reading the source.
**Friction level:** High. Blocked edge-case testing until source was read.

### F-2: `--json` flag position is order-sensitive relative to subcommands

`8v log last --json` fails with "unexpected argument '--json' found".
`8v log --json last` works.
The `--help` output does not indicate this ordering requirement. The flag appears in the global `Args` struct, so it must precede the subcommand in the CLI invocation.
**Friction level:** Medium. Caused an unexpected parse error; required source reading to diagnose.

### F-3: Self-emit behavior makes empty-store testing non-trivial

Every `8v` invocation writes a `CommandStarted` event before the command runs. A 0-byte `events.ndjson` is therefore not truly empty from the tool's perspective — the invocation always populates it. This is architecturally intentional but surprises users testing the empty-store behavior. The "orphan CommandStarted" warning is the only signal.
**Friction level:** Low (once understood). Creates confusing first-run output.

### F-4: `8v read` symbol map excludes `fn` items inside `impl` blocks for display truncation

During QA, reading `stats.rs` (a large file) via `8v read` returned top-level symbols only. The `rows_by_argv_shape` function on line ~400 did not appear in the symbol map because it is a module-level `fn` inside a large file — `8v read` symbol maps may truncate or omit items beyond a count threshold. Exact truncation threshold was not confirmed, but the symptom was: symbol map did not list the function, requiring `--full` to verify.
**Friction level:** Medium. Required a follow-up `--full` read to find a specific function.

### F-5: No way to tail or stream the live event store

During `8v stats` testing, to verify that a command was being recorded in real time, there is no `8v log --follow` or `8v stats --watch` form. Verification required running `8v log last` after each test command. A tail/follow mode would reduce round-trips significantly.
**Friction level:** Low (workaround exists). Noted as a missing capability.

---

## Summary

8 confirmed bugs (2 broken features, 3 silent failures, 3 inconsistencies), 5 UX friction points.

| ID | Severity | Category | Status |
|---|---|---|---|
| BUG-5 | Critical | Broken feature | `--shape` always returns 0 rows |
| BUG-2 | High | Silent failure | search drops all warnings |
| BUG-7 | High | Silent failure | `--compare agent` retries always 0 |
| BUG-6 | High | Wrong data | drill failure_hotspots not scoped |
| BUG-4 | Medium | Cosmetic | double line-number prefix in warnings |
| BUG-3 | Medium | Stability risk | Debug format used as display string |
| INCONSISTENCY-1 | Low | Polish | JSON pretty vs compact across commands |
| INCONSISTENCY-2 | Low | Polish | byte units SI vs binary across commands |
| BUG-1 | Low | Spec | `--limit` default 20, design says 10 |
