---
name: Benchmark Infrastructure
type: design
status: draft
connects:
  - to: benchmark-audit.md
    relation: replaces
---

# Benchmark Infrastructure — Design

Everything wrong with the current benchmark infrastructure, and the shape of what replaces it.

Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.

---

## What's Wrong

### 1. Events, not "command events"

The storage file is called `command-events.ndjson`. The StorageSubscriber writes generic bytes from the EventBus, but the file name couples storage to one event type. CommandStarted and CommandCompleted are just two events. There will be others. The naming creates a bias — it implies commands are the only events worth storing.

It should be flat event storage. Every event goes there. One stream. Consumer decides.

### 2. The event infrastructure is inconsistent

When we deleted the EventWriter (700 lines) and o8v-events crate, we replaced it with:
- Two-snapshot delta for check (last-check.json)
- A `command-events.ndjson` file written by StorageSubscriber

But StorageSubscriber is in the o8v binary crate, not in a library. Benchmarks can't use the same infrastructure to read events. McpMeasurement in o8v-testkit reads the raw file directly, bypassing the infrastructure entirely. There's no round-trip test proving events written by the real pipeline can be read back by the measurement code.

### 3. Silent fallbacks destroy measurement data

The current benchmark code has silent fallbacks throughout:
- `unwrap_or(0.0)` on cost — silent zero instead of error
- `unwrap_or(-1)` on exit code — invented value
- `serde_json::from_str` failures silently `continue` — dropped data
- `McpMeasurement::from_home().unwrap_or_else(... zero())` — if measurement fails, pretend zero

In a measurement system, a silent fallback doesn't hide an error — it produces a wrong number that looks right. That's worse than crashing. Every one of these is a Rule 4 violation.

### 4. No isolation between benchmark arms

Arms share the real HOME directory. `command-events.ndjson` is cleaned between arms with `remove_file()`, but:
- If a test crashes, stale data persists
- Events from arm 1 can leak into arm 2 measurements
- No structural guarantee of isolation

### 5. Wrong permission mode

v4 arms use `bypassPermissions`. This skips hooks. Hooks are what block native tools and force the agent to use 8v. The benchmark measures the worst of both worlds — agent uses native tools AND 8v — not what a real user experiences. v7 got this right with `acceptEdits`. The default in `run_claude()` is `bypassPermissions` — the wrong default.

### 6. Agent misuses 8v

The agent uses `8v run "grep ..."` instead of `8v search`. It shells out through 8v to use native commands. The prompt and CLAUDE.md don't enforce correct 8v usage. 8v's token-efficient output is never used. The benchmark measures 8v overhead without 8v benefits.

### 7. Baseline and 8v arms do different work

The baseline agent runs cargo check, clippy, fmt separately. The 8v agent runs 8v check (which does all three) but also explores, formats, and tests. The comparison measures different workloads. Not apples-to-apples.

### 8. Versioning is wrong

v1, v2, v3... v8 are not versions. They're value claims: quality, efficiency, completeness. But the naming implies sequential experiments. Each claim needs scenarios with different setups, not one hardcoded test per "version."

### 9. Code duplication

git init + config + add + commit is copy-pasted 6 times across test functions. Project setup is not factored into infrastructure.

---

## Two Data Sources

### Internal (8v controls)

Events emitted by the EventBus, stored by StorageSubscriber. Today: CommandStarted, CommandCompleted. Tomorrow: potentially many more. These tell us what 8v did — which commands were called, how much data went in and out, how long they took.

### External (agent stream)

Claude CLI stream-json output. Per-turn token counts (input, output, cache_read, cache_creation). Tool calls (name, arguments). Response text. Total cost. Init message size (system prompt tax).

This tells us what the agent did — which tools it chose, how many tokens it spent, what it concluded.

### What we discussed but haven't designed

- Agent feedback: at the end of a run, ask the agent for structured feedback. How was the experience? What worked? What didn't? What was missing? What was confusing? This is qualitative data alongside the quantitative.
- Claude Code hooks: could provide richer signal about agent behavior beyond what stream-json gives us.
- Schema overhead measurement: how many tokens do tool definitions cost per turn?

---

## The Shape

```
Scenario → Setup → Run → Collect → Data
```

1. **Scenario** — defines what we're measuring. Fixture, prompt, setup configuration. Not a version number.
2. **Setup** — creates an isolated environment. Uses 8v's own infrastructure (not raw file paths).
3. **Run** — executes the agent, streams output.
4. **Collect** — gathers both internal events and external stream into one structured record. No fallbacks. Crash on bad data.
5. **Data** — the structured measurement. Analysis happens separately, not mixed into collection.

---

## Crate Restructuring (Confirmed)

Current: 8 crates. Target: 5 library + 1 binary + 1 testkit.

### What moves where

**o8v-workspace → deleted.** All contents move to o8v (main binary):
- WorkspaceRoot, StorageDir, ConfigDir, resolve_workspace()
- These are application concerns, not library concerns

**o8v-project → deleted.** Contents split:
- Detection logic (detectors/, detect_all()) → o8v-stacks (detection is a stack concern)
- ProjectRoot, Project types → o8v (application layer)

### Target layout

- **o8v-core** — types, traits, events, EventBus
- **o8v-fs** — safe filesystem I/O
- **o8v-stacks** — stack definitions + detection
- **o8v-process** — process execution
- **o8v-check** — check system
- **o8v-testkit** — test utilities
- **o8v** — application: WorkspaceRoot, ProjectRoot, StorageDir, commands, dispatch, events read/write, CLI, MCP

### Order

1. Absorb o8v-workspace into o8v
2. Split o8v-project: detectors → o8v-stacks, project types → o8v
3. Delete both crates
4. Build event reader in o8v (now that StorageDir is here)
5. Fix benchmarks on the new foundation

## Open Questions

These need answers before any code:

1. **Event storage naming and structure.** `command-events.ndjson` → what? Just `events.ndjson`? One file or namespaced? What's the namespace strategy if we have many event types?

2. **Where does McpMeasurement live?** It reads events. Should it use the same infrastructure that writes them? If so, event reading needs to be in a library crate, not the binary.

3. **How do we isolate benchmark arms?** Per-arm HOME directory? Per-arm storage directory? How does the Claude CLI still function with a fake HOME (it needs API keys)?

4. **What are we actually benchmarking?** Not v1-v8. What are the claims? Each claim maps to scenarios. The claims need to be defined before we design scenarios.

5. **How strict is data collection?** Current code silently drops bad data. The rule should be: crash on bad data, never invent values. But what about flaky agent behavior? The agent might not use 8v in a given run — is that a data point or a test failure?

6. **What external data can we actually collect?** The Claude stream-json gives us tool calls and tokens. What else is available? Hooks? What's the full list?

7. **Agent feedback format.** What exactly do we ask? How do we structure the response so it's machine-readable alongside the quantitative data?

---

## Infrastructure Audit (2026-04-13)

Full codebase audit for architectural inconsistencies. 29 findings across 6 patterns.

### Bypassed o8v-fs (14 findings)

Production code that uses raw `fs::read_to_string` / `fs::write` instead of `o8v-fs` safe_read/safe_write:

- `o8v/src/init/ai_docs.rs` — 7 raw fs calls
- `o8v/src/init/mcp_setup.rs` — 13 raw fs calls
- `o8v/src/init/claude_settings.rs` — 13 raw fs calls
- `o8v/src/init/mod.rs` — raw fs::write for config.toml
- `o8v/src/hooks/git.rs` — 10 raw fs calls, some with .unwrap()
- `o8v/src/hooks/install.rs` — 13 raw fs calls
- `o8v/src/storage_subscriber.rs` — fs::read_to_string with .unwrap()
- `o8v/src/bin/event_cost.rs` — 11 raw fs calls
- `o8v/tests/agent_benchmark.rs` — raw fs for events, mcp.json, settings

### Silent fallbacks in data paths (8 findings)

Code that invents data when reading fails:

- `agent_benchmark.rs` — 5x `McpMeasurement::from_home().unwrap_or_else(... zero())`
- `agent_benchmark.rs` — 4x `.unwrap_or(0)` on metrics
- `command_events.rs` — `unix_ms()` returns 0 on clock failure
- `dispatch.rs` — 2x `.unwrap_or(0)` on SystemTime
- `check.rs` — `unwrap_or_default()` on corrupt last-check.json
- `measurement.rs` — `zero()` constructor exists to be a silent fallback
- `release_server.rs` — `unwrap_or_default()` on file read

### Missing Deserialize (6 findings)

Types that serialize to disk/stream but have no Deserialize:

- `CommandStarted`, `CommandCompleted` — written to events NDJSON, cannot be read back
- `Caller` — embedded in events, no Deserialize
- All diagnostic types (`Diagnostic`, `Location`, `CheckEntry`, etc.)
- All check JSON output types (`CheckResultJson`, `CheckEntryJson`, etc.)
- All streaming JSON event types

### Duplicated types (3 findings)

The same concept defined independently in multiple places:

- Command events: `o8v-core/command_events.rs` + `o8v-testkit/measurement.rs` + `agent_benchmark.rs`
- Each defines its own structs with different fields for the same data

### Code in wrong crate (2 findings)

- `StorageSubscriber` in `o8v/src/` (binary crate) — cannot be tested or reused without full binary
- `McpMeasurement` in `o8v-testkit/` — reads production event data, crosses test/production boundary

### Split systems (2 findings)

- Command lifecycle events (EventBus → command-events.ndjson) vs diagnostic delta (check → last-check.json) vs benchmark measurement (McpMeasurement) — three disconnected systems
- Event types serialize in o8v-core, but readers are in o8v-testkit and agent_benchmark — no shared read infrastructure

---

## What This Doc Does NOT Contain

- No solutions. We haven't designed them yet.
- No generated scenarios. We haven't defined what we're benchmarking.
- No code. Design comes first.
- No assumptions about what the infrastructure looks like. We start from the problems.
