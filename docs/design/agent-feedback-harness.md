# Agent Feedback Harness — Design

**Status:** Draft, awaiting founder review. No code until the open question (§5) is answered and the feature freeze (2026-04-14) lifts.
**Depends on:** commit `e3f610d` (argv threading into `CommandStarted`, 2026-04-17) — the design is only computable after that fix landed.

## 1. Problem

After an agent session, there is no offline way to see which 8v commands the agent retried, which errors it repeated, and where it got stuck — so we cannot close the loop between "agent called 8v" and "agent succeeded with 8v."

## 2. Users

1. **Primary — the founder** reviewing an agent session post-hoc (minutes to hours later). Decides which ergonomics to fix next. First version is built for this reader.
2. **Secondary (deferred) — the agent itself** consuming a digest mid-session to self-correct. Out of scope until the founder use case is validated. Per rule: ask who the user is, don't build for imaginary future users.

## 3. Minimum viable signals

All derived from `CommandStarted` + `CommandCompleted` joined on `run_id`. Session window = events since the last idle gap > N minutes (`--since`, `--run-id`, `--all` flags escalate).

| # | Signal | Computation |
|---|---|---|
| 1 | **Retry clusters** | Group successive `CommandStarted` where `command` + normalized argv (strip volatile tokens) repeat within a short window. Surfaces: `read a.rs --full` ×4 in 90s. |
| 2 | **Failure streaks** per command shape | Join Started→Completed on `run_id`; per `(command, argv-shape)` count `success=false` in a row. Surfaces: `write --find/--replace` failed 6× consecutively on `handler.rs`. |
| 3 | **Stuck-point** | Longest run of consecutive failures on same `project_path` + `command` with no intervening success on any command in that project. This is the "where it got stuck" signal. |
| 4 | **Command mix** | Histogram of `command` over session. For `read`, share of `--full` vs range vs symbol-map (parsed from argv) — direct read on progressive-CLI discipline. |
| 5 | **Session duration & idle gaps** | First Started → last Completed `timestamp_ms`; list gaps > threshold. Wall-clock context. |
| 6 | **Re-reads of same target** | For `read` argv, distinct path tokens + occurrence counts. Surfaces `main.rs 16×` waste. |

**Optional (drop if it bloats v1):** duration outliers — per-command p95 `duration_ms`, flag runs > 3× p95 as slow outliers.

No new event types proposed. Every signal is computable from today's schema plus the argv fix. Gaps we cannot compute (Bash calls outside 8v, silent write failures that report `success=true`) must be listed explicitly in the output as **"blind spots"** — no silent fallbacks.

## 4. Output shape

Single subcommand: **`8v agent-feedback`** (not a separate binary).

- **Default (human):** compact table — session window, total commands, top 3 retry clusters, top 3 failure streaks, stuck-point (if any), one-line blind-spots footer.
- **`--json`:** full structured report (all signals, every cluster, every gap) for batching and scripting.
- **`--verbose`:** same depth as `--json` but rendered as tables.
- **Scope:** `--run-id <id>` / `--since <duration>` / `--session last`.
- **Exit code:** non-zero if any failure streak exceeds a configurable threshold, so CI and in-loop agents can gate on it.

Justification for subcommand over separate binary: single-binary constraint; the data source is already owned by 8v; every analyzer-style capability (`check`, `ls`, `read`) is a subcommand — a separate binary would fragment the surface.

## 5. Open question (blocks implementation)

Should the session boundary be:

- **(A) inferred** from idle gaps in `events.ndjson` — simple, read-only, works today with zero schema change; or
- **(B) explicitly tagged** by the caller — MCP/CLI entry point emits a session id, which is a schema change the freeze forbids right now.

The answer decides whether v1 is pure analyzer (A) or needs a coordinated event-schema change after the freeze lifts (B). Recommend A for v1 unless the founder has a use case that inferred windows cannot serve.

## 6. Out of scope (v1)

- Real-time in-session feedback to the agent.
- Cross-session trend analysis (multi-day).
- Fix suggestions. We surface the signal, not the remedy.
- Any cloud upload.

## 7. Files touched when implementation begins

- `o8v/src/event_reader.rs` — session windowing + argv normalization.
- `o8v/src/commands/` — new `agent_feedback.rs` module.
- `o8v-core/src/render/` — report type + Renderable impl.
- `o8v/src/commands/mod.rs` — dispatch arm.
- `o8v-core/src/events/lifecycle.rs` — untouched unless open question resolves to (B).
