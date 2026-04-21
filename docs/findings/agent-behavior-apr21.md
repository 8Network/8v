# Agent Behavior Findings — 2026-04-21

Observed from N=6 fix-go and N=6 fix-python-traversal experiments.
All sessions visible in `8v log` after observability fix (commit ff0a601).

## What agents actually do

### Native baseline — fix-go (10.8 tool calls/run)

```
Agent(spawn subagent: "Explore Go project")
  → Bash("find . -type f -name '*.go'")
  → Bash("ls -la .")
  → Read(lib.go)
  → Read(lib_test.go)
  → Read(go.mod)
  → Edit(lib.go) [ERROR — wrong string match]
  → Read(lib.go)          ← re-read after failed edit
  → Edit(lib.go)          ← retry
  → Bash("go test ./...")
  → Bash("go test ./...")  ← second run to confirm
```

Key problems:
1. **Sub-agent overhead**: Every run spawns an Agent for exploration. The agent description adds ~3,400 bytes to context.
2. **Separate discovery calls**: `find` + `ls` = 2 Bash calls for what `8v ls --tree --loc` does in 1.
3. **Separate reads**: 3 individual `Read` calls instead of 1 batched `8v read a b c`.
4. **Edit retry**: Failed edit forces re-read + retry loop. 2+ errors per run on average.
5. **Double test run**: 2 Bash test calls to confirm the fix.

### 8v treatment — fix-go (5.3 tool calls/run, 0 errors)

```
ToolSearch(select:mcp__8v__8v)   ← deferred tool load, always first
8v ls --tree --loc
8v read lib.go lib_test.go --full  ← batched
8v write lib.go --find "..." --replace "..."
[8v write lib.go:10-11 --delete]   ← optional: removes BUG comment
8v test .
```

4-5 calls. Zero errors across all 6 runs. Consistent across all sessions.

### Native baseline — fix-python-traversal (17.8 tool calls/run)

```
Agent(spawn subagent: "Explore Python project")
  → Bash("find . -name '*.py'")
  → Bash("ls -la .")
  → [Bash × 1-2 more discovery]
  → Read × 6-8 (separate files: src/*.py, tests/*.py, setup.py, ...)
  → Bash("python -m pytest") [ERROR — test fails, pre-fix]
  → Bash("python -m pytest") [ERROR — retry]
  → Bash("python -m pytest") [ERROR — retry]    ← LANDMINE: 5+ retries
  → [Edit/Read cycle]
  → Bash("python -m pytest") ← finally passes
```

All 6 native runs: stuck-loop landmine (3+ consecutive same-tool calls with same args).
Root cause: agents run pytest to understand the failure BEFORE fixing — then fix, then test.
The retry loop is behavioral, not a missing-tool issue (pytest IS installed).

### 8v treatment — fix-python-traversal (5.0 tool calls/run, 0 errors)

```
ToolSearch(select:mcp__8v__8v)
8v ls --tree --loc
8v read src/path_sanitizer/core.py tests/test_safe_join.py --full
8v write src/path_sanitizer/core.py:25-29 "..."
8v test .
```

4 calls. Clean path. Zero retries across all 6 runs. Token stddev=677 (nearly deterministic).

## Instruction gaps found

### Gap 1: `--find` + `--delete` combination

`8v stats` failure hotspot showed: `write lib.go --find "..." --delete`.
Agents try to combine `--find` (lookup by string) with `--delete` (remove content).
The `--find` flag only works with `--replace`. To delete by content, agents must:
1. Use `8v read` to find the line number
2. Use `8v write :X-Y --delete`

The CLAUDE.md does not state this constraint explicitly. Agents infer it incorrectly.

**Proposed instruction addition**: "Note: `--find` requires `--replace`. To delete content by
string match, read the file to find the line number, then use `8v write :X-Y --delete`."

### Gap 2: Sub-agent spawning (native baseline only)

Native agents spawn a sub-agent for exploration on every run. This is an artifact of Claude
training — when agents see unfamiliar project structure, they delegate exploration. 8v's
`ls --tree --loc` provides the project map in a single call that prevents this behavior.
Observation: **8v eliminates the sub-agent spawn pattern entirely** — not one 8v run
spawned a sub-agent. The `ls` output is sufficient to start working directly.

### Gap 3: Pytest retry loop (native baseline)

Native agents run `pytest` before fixing the code (to understand the failure output),
then after (to verify). With a broken test this creates: fail → fail → fail → fix → pass.
8v agents don't have this because `8v test .` is fast and they only run it once (after fix).
The behavioral difference: 8v's single `test` call at the END avoids the pre-fix retry loop.

## Quantified behavioral gaps

| Metric | Native (go) | 8v (go) | Native (python) | 8v (python) |
|--------|-------------|---------|-----------------|-------------|
| Tool calls/run | 10.8 | 5.3 | 17.8 | 5.0 |
| Errors/run | 2+ | 0 | 3+ | 0 |
| Landmines | 0/6 | 0/6 | 6/6 | 0/6 |
| Turns/run | 14.5 | 8.3 | 22.7 | 7.5 |
| Cost/run | $0.1050 | $0.0752 | $0.1424 | $0.1019 |

## Future feature ideas — token savings (not for now)

These are patterns observed across benchmark runs that would reduce tokens if built.
None to be built during Phase 0. Log them for Phase 3+ roadmap.

### F1: `8v write --find ... --delete`

Agents reliably attempt this combination. Supporting it eliminates 1 read-then-delete cycle.
Estimated savings: 1 tool call per run on comment-cleanup tasks.

### F2: `8v read --symbols <path>`

Currently `8v read <path>` returns symbol map. Agents sometimes read the full file
when they only need line numbers. A flag `--symbols` could be the default short-form
to save on output token cost. (Already exists as default — but MCP description could
be clearer about when to use `--full` vs default.)

### F3: `8v test . --watch-fail`

Stream the FIRST test failure and stop. Currently agents get the full test output
even when they only need to know which test failed. For large test suites this wastes
output tokens on irrelevant passing tests. Saves cache_creation on long pytest runs.

### F4: `8v write --multi` or batch write

Agents sometimes need to write to 3-4 files. Each write is a separate MCP call.
Batch write: `8v write a.py --find X --replace Y b.go:10 "..."` in one call.
Saves 2-3 tool calls on multi-file fix tasks.

### F5: `8v diagnose <path>` — single-call triage

Pattern observed: agents always do ls → read → identify problem. A single `8v diagnose .`
command that returns the project structure + files with errors + suggested fix locations
would reduce discovery to 1 call on complex projects. High implementation cost.

### F6: Incremental event pruning in `8v stats`

`events.ndjson` grows without bound. For an agent session of 100 calls/day,
the file will hit the 10MB safe_read limit in ~30 days (now bypassed with read_to_string).
A `8v events prune --older-than 30d` command would keep the file manageable.
Not urgent — `read_to_string` removes the hard cap. But operational hygiene.
