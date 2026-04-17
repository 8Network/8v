# Alerts + Signal Surface — Sketch

**Status:** Sketch. Explicitly **not a design**. Captures the idea so it can be revisited after `log` and `stats` ship and dogfood produces a second named user. Previous draft (174 lines, 7 detections, TOML schema, v1/v2 roadmap) collapsed after three-angle adversarial review — see commit history.
**Supersedes:** earlier draft in this file.

## 1. The idea

`log` and `stats` are passive. They answer when asked. They do not say "something broke" on their own. A third mode — **named detection with a threshold** — closes the loop. Instead of reading a failure-rate column, the user reads `write_find_replace_fail_rate: 22% (threshold 10%)` and acts on it.

Related: **8v knows more about itself than it emits.** MCP schema size in tokens, input tokens per call, per-turn accounting, detected project stack, error class. Each is free for us to produce. Adding them is a schema change; adding one without a consumer is waste.

## 2. Why this is a sketch, not a design

Review found that the detailed draft (v1 catalog, TOML loader, `alerts.toml` precedence rules, exit-code gates, 6 new schema fields + 1 new event type) **commits to shapes before there's a user who reads them**.

Specifically:

- Six of seven proposed detections had the same single user (founder, post-hoc). The "multidimensional feedback" framing (founder / user / agent) didn't survive contact with the current product — it's one axis today.
- Zero of six proposed schema fields had a consumer in the v1 catalog. Each `#[serde(default)]` field is a permanent migration.
- Three top-level commands (`log`, `stats`, `alerts`) for queries over one event log is sprawl. The simpler frame is: **every command is a query over `events.ndjson` with a render mode.** `alerts` collapses to `stats --slas`.
- During the feature freeze (2026-04-14), a fully specified third command primes premature implementation.

## 3. What survives

Two ideas worth keeping in writing:

1. **SLA is a concept, not a file.** An SLA is `(name, metric, target, window, filters)` — a named target about observable behavior. That shape is the same whether it's declared as a Rust constant (built-in detection with default threshold), loaded from `~/.8v/alerts.toml`, attached as an annotation on a command's `Args`, or emitted in JSON. One `Sla` type, many transports. The earlier draft conflated "SLA" with "entry in alerts.toml" and produced a false split between "hard-coded thresholds" and "SLAs" — they're the same concept at different transport points.

2. **The simplest surface** for acting on SLAs is `stats --slas`. Violations surface as rows in `stats` output with a `[violated]` tag; exit code matches `stats`'s "empty/insufficient = 2" rule so CI gating is one convention across both commands. No new subcommand. Config transport (`alerts.toml`, project-local variant, precedence) is a later question — first nail down the concept and ship the built-in SLAs that have defaults.

3. **Self-emitted signals are an orthogonal axis** from the command surface. The schema-change question is "what can 8v emit that nobody else can reconstruct, and for which detection?" The earlier draft inverted that: proposed the fields first, hoped for detections later. Correct order: each schema field is blocked on a named, agreed-upon consumer.

## 4. When to revisit

Conditions for reopening this as a real design:

- `log` and `stats` have shipped and are in regular use.
- Dogfood or a real user has produced at least **one concrete detection** that neither `log` nor `stats` can express as a simple query. "I want to know when X happens" — where X is not "show me sessions with failures" (log) or "show me the failure rate" (stats).
- Or: a benchmark / CI pipeline concretely needs a violation-exits-nonzero gate and the 5-line wrapper around `stats --json | jq` has become the wrong abstraction.

## 5. Adversarial findings parked for later

If this is ever reopened, the following rules from the review must carry forward:

- **One canonical definition for "failure cluster"** across log/stats/alerts — name them distinctly if the math differs.
- **Exit codes harmonized** across log/stats/alerts. Current log/stats rule: 0 = ran, 2 = empty/no-data. Alerts would add: > 2 = violation.
- **Every rate-based detection needs a sample-size floor** (`n_calls ≥ 20 && n_sessions ≥ 3`). `insufficient_data` is a third outcome alongside `ok` and `violated`.
- **Detections and SLAs are the same concept**, not two parallel mechanisms. A built-in detection is an SLA with a default threshold, expressed in code. A user-configured SLA is the same shape, expressed in a transport (TOML, annotation, whatever). One `Sla` type, many transports. Don't build two evaluators.
- **Recommendations name behaviors, not commits** — commit SHAs rot. Add a lint test asserting no `[0-9a-f]{7,40}` in recommendation strings.
- **Legacy events policy unified** across all readers — today log/stats/alerts each proposed different treatment of pre-`session_id` lines. Pick one per operation (bucketing, percentile aggregation, alert evaluation) and document once.
- **Config reads through `o8v-fs::safe_read` with containment extended** — don't open a fresh I/O surface just for TOML.
- **Schema fields gated on named consumers** — if no detection uses `turn_in_session`, don't add it. No speculative additions.

## 6. Out of scope (for this sketch and its revival)

- Agent in-loop feedback (axis 3). MCP is request-reply today; no push channel. The data-shape claim that v1 could "support it later" was aspirational. Park indefinitely.
- Cross-host / multi-machine aggregation.
- Persistent alert history / baselines (would require a second file alongside `events.ndjson`; defer until there's a regression-detection user).

---

The earlier fully-specified draft is preserved in git history (file was rewritten, not deleted, so `git log -p docs/design/alerts-and-signals.md` shows the v1 catalog, TOML schema, and §3 detection table for reference if someone reopens this).
