# AI Errors Log

Mistakes made by AI during 8v development. Each entry documents what went wrong, why, and the rule that prevents recurrence.

---

## 2026-04-12: Parallel Event System Built Instead of Using the Bus

**What happened:** AI built EventWriter (700 lines) — a complete parallel event system with SHA256 IDs, series accumulation, file caching, log rotation, finalize lifecycle. This existed alongside the EventBus that was designed to handle all events.

**Why it happened:** The AI implemented the first solution that came to mind (accumulate diagnostics, merge state, write files) without asking what the user actually needed. The requirement was "is it getting better or worse?" — which only needs two snapshots. The AI built machinery for a problem that didn't exist.

**The fix:** Deleted 700 lines. Store last check result. Compare with current. One file.

**Rule:** Before building, ask: what does the user need? What is the minimum to deliver that? If the answer is simpler than what you're about to build, you're solving the wrong problem.

---

## 2026-04-12: Dispatch Scattered Across Interfaces

**What happened:** Both CLI (main.rs) and MCP (handler.rs) were building CommandContext, resolving workspaces, wiring subscribers, and choosing audience. The design said interfaces should be stupid — parse input, forward to dispatch, deliver result.

**Why it happened:** AI implemented features incrementally without enforcing the boundary. Each interface grew logic because "it was easier to add it there." Nobody enforced the rule that interfaces must not touch context.

**The fix:** Moved all context building into dispatch. CLI calls dispatch_command(command, Caller::Cli, interrupted). MCP calls dispatch_command(command, Caller::Mcp, interrupted). Nothing else.

**Rule:** Interfaces are stupid. If an interface is doing more than parsing and forwarding, something is wrong.

---

## 2026-04-12: StorageSubscriber Knew About Event Types

**What happened:** StorageSubscriber had an if/else chain: "are you CommandStarted? are you CommandCompleted?" — downcasting to specific types before it could serialize and write. Adding a new event type required updating StorageSubscriber.

**Why it happened:** The EventBus passed &dyn Any, so subscribers had to downcast. The AI treated this as normal. The founder pointed out: "give the data. then it's none of your business how it's stored."

**The fix:** Standard message bus pattern. emit() serializes once. Subscriber receives &[u8]. StorageSubscriber writes bytes. Done. Same pattern as every pub/sub system ever built.

**Rule:** Don't reinvent messaging. Producer serializes, bus carries bytes, consumer decides.

---

## 2026-04-12: Events Not Wired — Benchmarks Passed With Dead Code

**What happened:** MCP events (McpInvoked/McpCompleted) were being tested in benchmarks against mcp-events.ndjson, but the actual event system had been replaced by CommandStarted/CommandCompleted writing to command-events.ndjson. The old types still existed in testkit, the old file path still existed in StorageDir, and benchmarks still referenced them — all dead code.

**Why it happened:** When the event system was migrated from MCP-specific to unified, the AI didn't update all consumers. Testkit, benchmarks, and StorageDir kept referencing the old types and paths.

**The fix:** Updated testkit to CommandStarted/CommandCompleted, updated benchmarks to command-events.ndjson, removed StorageDir::mcp_events(), removed all McpInvoked/McpCompleted references.

**Rule:** When you replace a system, grep for every reference to the old one. Dead code that compiles is still a bug.

---

## 2026-04-12: Commands Bypass Architecture (Ongoing)

**What happened:** Of 11 commands, only 3 (build, test, run) correctly follow the dispatch pipeline. fmt has silent fallbacks. upgrade ignores context. check does its own workspace resolution. read/write/search/ls use std::env::current_dir() instead of context. hooks reports empty data.

**Why it happened:** Commands were implemented before the architecture was enforced. Each command was built to "work" — produce output — without following the designed pipeline. The AI built what compiled, not what was correct.

**Status:** Ongoing. Needs systematic migration command by command.

**Rule:** Working is not correct. Every command must follow the same pipeline: parse → dispatch → execute(ctx) → Report → render. No exceptions, no shortcuts.

---

## 2026-04-12: Refactoring Instead of Rethinking

**What happened:** When the diagnostic event system needed to move to the EventBus, the AI designed DiagnosticEvent types, a SeriesSubscriber, constructor logic — moving 700 lines of complexity to a new home. The founder stopped it: "What do we require?"

**Why it happened:** The AI's instinct was to refactor — preserve the logic, improve the structure. But the logic itself was wrong. Refactoring preserves assumptions. If the assumptions are wrong, the refactored code is still wrong, just tidier.

**The fix:** Went back to the requirement. "Is it getting better or worse?" → Two snapshots. Deleted everything.

**Rule:** Before refactoring, ask if the thing should exist at all. Requirement before refactor.
