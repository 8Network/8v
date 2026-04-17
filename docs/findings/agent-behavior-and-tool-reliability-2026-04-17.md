# Agent Behavior and Tool Reliability — 2026-04-17

## Scope

Findings gathered on 2026-04-17 while hardening `8v init` and after a sonnet agent ran `cargo test --workspace` via Bash instead of `8v test .`. Sources: live agent session + `~/.8v/` event store.

---

## Event-Sourcing State (Disaster Area)

Three stores exist in `~/.8v/`:

- `events.ndjson` — 5,774 lines, canonical, active today.
- `command-events.ndjson` — 5,228 lines, legacy, should be removed.
- `mcp-events.ndjson` — 3,233 lines, legacy, should be removed. Stopped writing Apr 13.

Additional leak:

- `/Users/soheilalizadeh/8/mcp-events.ndjson` exists in the workspace root. Regression. Events must only land in `~/.8v/`.

Original design mistake: MCP was hard-coded into event naming/storage. Consolidation happened — events are emitted generically; MCP is just one emitter. Cleanup of legacy files and workspace-root leak did not finish.

---

## What the Canonical Store Captures

- `events.ndjson` records `CommandStarted` / `CommandCompleted`. The emit
  site (`o8v/src/dispatch.rs:117`) receives `command_str = command.name()`
  from `commands/mod.rs:151`, which is the bare subcommand ("read",
  "build"). The struct itself (`CommandStarted.command: String`) can hold
  a full argv — the stripping is at the call site, not the schema.
- Consequence: cannot distinguish `read --full` from `read :10-50` from
  a symbol-map read in the event log.
- **Correction** (was wrong in earlier draft): `CommandCompleted.success:
  bool` IS present on the struct (`o8v-core/src/events/lifecycle.rs:100`)
  and emitted per run. Failures ARE distinguishable — just not argv.
- Native `Bash(cargo test ...)` calls are invisible to the store. Only
  `8v run "cargo ..."` is traceable because the command string is embedded.

---

## What We CAN See (from `mcp-events.ndjson`, Apr 12–13, before it went stale)

- `read --full`: 244 events. `read :range`: 112 events. `read` symbol-map: 17 events. Progressive-read discipline ignored.
- Same file re-read many times in a session: `main.rs` 16x, `dispatch.rs` 12x, `handler.rs` 9x, `permission-model.md` 9x.
- `8v test/check/build`: 71 events. `cargo ... via 8v run`: 27 events.
- Failed `write --find/--replace`: 1 detectable (34-byte render). Rest (132 calls) look successful but silent failures below detection threshold are possible.

---

## Tool Reliability Problems Confirmed This Session

- `8v read a.rs b.rs c.rs --full` returned "not found" for all three. Batch + `--full` silently fails. Docs advertise batch support.
- `8v write --find/--replace` requires character-perfect whitespace match. On mismatch: "not found" with no diff hint. Causes retry loops.
- After a write, no way to verify the replaced region without a second read.
- Agent used native `Bash` for `cargo test --workspace` instead of `8v test .`. Event store did not capture it. Rule violation invisible.

---

## Agent UX Feedback (Captured Verbatim from Sonnet Agent Today)

- `--find/--replace` requires exact whitespace — retry loops result.
- Multiline replace needs heredoc-style primitive.
- No `8v diff` after edit.
- Discovery (ls → symbol map → range) takes 3+ turns before first write.
- Batch `--full` silently fails.
- Error from `--find/--replace` needs "closest match differed here" style output.

---

## Test Gap (Open Question — Do Not Answer, State Only)

- Two confirmed bugs: batch `--full`, `--find/--replace` whitespace.
- Question: what integration tests exist for these paths, and why did none fail? (Investigate separately.)

---

## The Meta-Problem

The event store was built to answer "what did the agent do?" — but cannot, because:

1. Arguments are stripped.
2. Bash is not captured.
3. Success/error is not recorded.
4. Legacy files fragment the signal.

Findings that should take one minute of telemetry query take hours of manual scrolling.

---

## Open Questions

1. Why does `events.ndjson` strip arguments while `mcp-events.ndjson` preserved them? Regression or intentional?
2. Why are `command-events.ndjson` and `mcp-events.ndjson` still being written to (or left on disk)?
3. Why is `/Users/soheilalizadeh/8/mcp-events.ndjson` in the workspace root? Writer path bug, or dead code still emitting?
4. Why is Bash not captured as an event? Is there a hook mechanism we did not wire up?
5. Why did E2E integration tests not catch the batch `--full` silent failure or `--find/--replace` ergonomics?
6. Can `McpCompleted` carry a success/error field without breaking consumers?
