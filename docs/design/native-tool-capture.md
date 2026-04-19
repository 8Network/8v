# Native Tool Capture — Feature Design (Level 1)

**Status:** Draft — pending founder review.
**Depends on:** `log-command.md` §6.1 (argv normalization), `stats-command.md` (event schema + Caller enum).
**Not a Level 2 doc.** No implementation detail here — only what and why.

---

## 1. Problem

`8v stats` reads `~/.8v/events.ndjson`. Every event in that file was produced by an 8v invocation — either via the CLI (`Caller::Cli`) or via the MCP server (`Caller::Mcp`). A Claude Code coding session routes the majority of its work through *native* tool calls: `Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob`. Those calls never touch 8v and produce no events.

**Dogfood finding (2026-04-19, multi-day session with `8v stats`):**
The founder ran `8v stats` after a long coding session. The output showed 87 8v commands. But the same session also contained hundreds of native `Read`, `Edit`, and `Bash` calls — none of which appeared. The "which commands cost the most" column was based on fewer than 20% of the actual tool calls. The p95 latency column was therefore meaningless as a proxy for agent cost. The "blind spots" line already acknowledges this in the `8v stats` output:

> `blind spots: native Read/Edit/Bash invisible`

This is not a display problem. It is a data problem. `8v stats` cannot fix what was never written.

**Consequence:** Token/latency totals undercount reality by at least a 4:1 ratio in typical sessions. Command cost rankings favor the subset of commands that happen to route through 8v, creating systematic bias in the benchmark and dogfood feedback loop.

---

## 2. Scope

### 2.1 What is in scope

- Capture every `PreToolUse` and `PostToolUse` hook event fired by Claude Code.
- Covered tools: `Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob` — the six native tool categories that replace 8v equivalents.
- Map each event pair to a `CommandStarted` + `CommandCompleted` record written to `~/.8v/events.ndjson`, using `Caller::Hook` (new variant, see §3.1).
- `8v stats` and `8v log` pick these up automatically because they read the shared event store — no changes needed in those commands.

### 2.2 What is out of scope

- Agent reasoning, thinking blocks, or intermediate text. We capture tool calls only.
- File *contents* in `Read` or `Edit` results. We capture paths and sizes, not diffs or payloads.
- Token counts from the Claude API. We have no access to the model's billing data from a hook. We estimate from byte sizes using the existing `bytes / 4` heuristic.
- Any command that is not a native tool call (MCP calls, agent-to-agent requests).
- Filtering or aggregation at capture time. Capture everything; filter in `log`/`stats`.
- Modifying hook behavior or blocking tool execution. These are passive observers only.

---

## 3. Event Mapping

### 3.1 New Caller variant

The existing `Caller` enum has two variants: `Cli` and `Mcp`. A third variant, `Hook`, is required:

```
Caller::Hook  →  serializes as "hook"
```

This is the only schema change. Every other field in `CommandStarted` and `CommandCompleted` already has a sensible mapping.

### 3.2 PreToolUse → CommandStarted

| Event field          | Source                                                                 |
|----------------------|------------------------------------------------------------------------|
| `event`              | `"CommandStarted"` (literal)                                           |
| `run_id`             | Minted by the hook binary (UUID v4 or ULID). Shared with PostToolUse. |
| `timestamp_ms`       | System clock at hook invocation.                                       |
| `version`            | 8v version string embedded at build time.                              |
| `caller`             | `"hook"`                                                               |
| `command`            | Tool name lowercased: `bash`, `read`, `edit`, `write`, `grep`, `glob`.|
| `argv`               | Derived from tool input JSON (see §3.3).                               |
| `command_bytes`      | `len(tool_name)` in bytes.                                             |
| `command_token_estimate` | `command_bytes / 4`                                              |
| `project_path`       | Resolved from the hook's working directory (see §3.4).                 |
| `agent_info`         | Omitted (`None`). Claude Code does not pass MCP client info to hooks.  |
| `session_id`         | Derived from Claude's `session_id` field (see §3.5).                  |

### 3.3 Tool input → argv

argv must be a flat `Vec<String>` that reproduces the logical invocation. The rules:

- **Bash**: `["bash", "<command string>"]`. The command string is the `command` field from the hook input.
- **Read**: `["read", "<file_path>"]`. Path from `file_path` field.
- **Edit**: `["edit", "<file_path>"]`. Path only — no old/new content. Content is never captured (non-goal §2.2).
- **Write**: `["write", "<file_path>"]`. Path only.
- **Grep**: `["grep", "<pattern>", "<path>"]`. Pattern and path from hook input.
- **Glob**: `["glob", "<pattern>"]`. Pattern from hook input.

Argv normalization from `log-command.md` §6.1 applies at write time: path separators normalized to `/`, absolute paths stripped to basename. This is the privacy fence (see §6).

### 3.4 PostToolUse → CommandCompleted

| Event field      | Source                                                                        |
|------------------|-------------------------------------------------------------------------------|
| `event`          | `"CommandCompleted"` (literal)                                                |
| `run_id`         | Must match the `CommandStarted` for this invocation. Hook passes it via env or temp file. |
| `timestamp_ms`   | System clock at hook invocation.                                              |
| `output_bytes`   | Byte length of the tool output JSON from Claude Code.                         |
| `token_estimate` | `output_bytes / 4`.                                                           |
| `duration_ms`    | `PostToolUse.timestamp_ms − PreToolUse.timestamp_ms`. Claude Code provides timestamps on hook input; if not, wall-clock delta. |
| `success`        | `true` if the tool call did not error. Determined from PostToolUse output presence and absence of error fields. |
| `session_id`     | Same derivation as CommandStarted (§3.5).                                     |

### 3.5 Session identity

Claude Code hooks receive a `session_id` string in the hook input payload. This is Claude's own session identifier, not our `ses_` ULID format.

**Decision: prefix Claude's session_id rather than mint a new one.**

Rationale: minting a new ULID per hook binary invocation would produce a new session for every tool call (the hook binary is a new process each time). Minting once per Claude session requires either a daemon or a shared temp file keyed on Claude's session_id — complexity that is not justified at this stage.

Instead: derive our `session_id` as `ses_hook_<first16charsOfClaudeSessionId>`. This:
- Is stable across all tool calls in a Claude session (Claude reuses its session_id).
- Is distinct from CLI/MCP sessions (those use `ses_` + ULID format).
- Is human-scannable: `ses_hook_` prefix immediately identifies provenance.
- Requires no shared state between hook invocations.

This session will appear as a distinct session in `8v log` and `8v stats`. That is correct — it *is* a distinct session (Claude Code vs. 8v CLI).

---

## 4. Installation

### 4.1 Decision: documented `.claude/settings.json` snippet, not `8v hooks install`

Rationale:
- `8v hooks install` would require 8v to write to `.claude/settings.json`, a file owned by Claude Code. Parsing and merging arbitrary JSON configs is error-prone and outside 8v's domain.
- The feature freeze rule prohibits new commands. `hooks install` would be a new subcommand.
- A documented snippet is verifiable, auditable, and already idiomatic for Claude Code users.
- Idempotency is simpler to guarantee when the user owns the config: they can see it and verify it.

### 4.2 Snippet

Add to `~/.claude/settings.json` (global) or `.claude/settings.json` (project-level):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|Read|Edit|Write|Grep|Glob",
        "hooks": [
          {
            "type": "command",
            "command": "8v hook pre"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Bash|Read|Edit|Write|Grep|Glob",
        "hooks": [
          {
            "type": "command",
            "command": "8v hook post"
          }
        ]
      }
    ]
  }
}
```

`8v hook pre` and `8v hook post` are subcommands of an existing or new `hook` subcommand. They read JSON from stdin and write events to `~/.8v/events.ndjson`.

### 4.3 Idempotency

The snippet is idempotent by nature: Claude Code merges hook arrays at the matcher level. If the user adds the block twice, both entries fire — which is a misconfiguration, not a silent data problem. The installation guide must warn: check for duplicates before adding.

If `8v hooks install` is ever built (post-freeze), it must read the existing config, check for an existing `8v hook pre/post` entry, and no-op if found.

---

## 5. Failure Modes

### 5.1 Hook binary not on PATH

Claude Code will fail to invoke the hook command. The tool call itself is **not blocked** — Claude Code's hook system is designed so that a failing hook does not prevent tool execution by default (exit code 0 is success; non-zero blocks only if configured as blocking). The hook must exit 0 even on failure.

**Consequence:** No event is written. This is a silent data gap. `8v stats` will show a "hook events missing" warning if a session boundary is detected without hook events. This is acceptable — it is an opt-in feature.

### 5.2 Event store write fails

The hook binary must handle write failures gracefully: log the error to stderr (Claude Code surfaces stderr in its hook error output), exit 0, and continue. The tool call must not be blocked.

This matches the existing rule: "Every error must be visible." The hook must not swallow errors silently.

### 5.3 PreToolUse fires without PostToolUse

This can happen if Claude Code crashes or the session is interrupted. The `CommandStarted` event is written without a matching `CommandCompleted`. `8v log` and `8v stats` already handle this: unpaired `CommandStarted` events are counted as in-flight and excluded from duration/output aggregation. No change needed in those commands.

### 5.4 run_id correlation between Pre and Post

The hook binary is a new process for every invocation. Pre and Post runs must share a `run_id`. The mechanism:

- The PreToolUse hook writes a temp file at `~/.8v/hook_run/<session_id>/<tool_call_id>` containing the `run_id`.
- The PostToolUse hook reads and deletes that file.
- If the file is missing (Pre never ran), Post writes a synthetic `CommandStarted` first with `duration_ms = 0`, then the `CommandCompleted`. This ensures the store is consistent.

The `tool_call_id` (provided by Claude Code in the hook input) is the natural key.

---

## 6. Privacy

### 6.1 What is captured

- Tool name (command field).
- For `Read`/`Write`/`Edit`/`Glob`: file paths.
- For `Bash`: the command string.
- For `Grep`: the search pattern and path.
- Tool output size in bytes (not content).

### 6.2 What is never captured

- File contents (the body of a `Read` result, the diff in an `Edit`).
- Tool output text.
- Reasoning or text produced by the agent.

### 6.3 Normalization fence

Argv normalization from `log-command.md` §6.1 is applied **at write time** (in the hook binary, before writing to `~/.8v/events.ndjson`), not at read time in `log`/`stats`.

Rationale: write-time normalization means raw paths never enter the event store. Read-time normalization would mean raw paths are persisted and only hidden at display time — a weaker privacy guarantee. Write-time is the correct fence.

For `Bash`, the command string is captured as-is. This may contain secrets (tokens, passwords in environment variables passed inline). **This is a known risk.** Mitigations:

- The event store is local and not transmitted.
- `8v hook pre` can apply a redaction filter for known secret patterns (e.g., strings matching `--token`, `--password`, `Bearer `) before writing.
- The user controls whether the hook is installed at all.

The redaction filter design is deferred to Level 2.

---

## 7. Acceptance Criteria

A reviewer can verify the feature is complete when:

- [ ] `Caller::Hook` variant exists, serializes as `"hook"`, and all existing tests still pass.
- [ ] `8v hook pre` reads a valid `PreToolUse` JSON payload from stdin and writes a `CommandStarted` event to `~/.8v/events.ndjson` within 50ms.
- [ ] `8v hook post` reads a valid `PostToolUse` JSON payload, correlates on `run_id` via temp file, and writes a `CommandCompleted` event.
- [ ] When `8v hook pre` is run twice with the same `tool_call_id`, the second invocation detects the existing temp file and skips (idempotent).
- [ ] When the event store directory does not exist, the hook creates it rather than failing.
- [ ] The hook binary exits 0 in all error conditions (store write fail, stdin parse fail, temp file missing).
- [ ] `8v log` shows hook events interleaved with native 8v events in the correct time order.
- [ ] `8v stats` includes hook events in command counts and duration percentiles.
- [ ] `8v stats` output distinguishes `caller=hook` from `caller=cli` and `caller=mcp` in `--compare agent` view.
- [ ] Argv for `Read`/`Write`/`Edit` contains only the basename, not the full path (normalization fence verified).
- [ ] A documented snippet in `docs/` shows exactly how to add hooks to `~/.claude/settings.json`.
- [ ] A test fixture verifies end-to-end: simulated hook stdin → event written → `8v log` reads it back correctly.

---

## 8. Resolved Decisions

1. **`8v hook pre` / `8v hook post` are allowed under the feature freeze as observability/stability infrastructure.** They are not user-facing commands. Both subcommands must be marked `hide = true` in clap so they do not appear in `--help`. The freeze exception is granted explicitly; no further approval is required.

2. **Bash redaction: §6.1 argv normalization applies, plus three secret patterns are redacted to `<secret>` at write time.** The patterns are:
   - API keys: `sk-[A-Za-z0-9]{20,}`
   - JWT tokens: `eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+`
   - URLs with embedded credentials: `://user:pass@` shape — the userinfo segment (`user:pass`) is scrubbed, leaving `://<secret>@`.
   No additional patterns are in scope for v1. The pattern list is fixed; it is not user-configurable.

3. **Installation scope: global `~/.claude/settings.json` only.** Project-scoped `.claude/settings.json` is a non-goal for v1. Global installation gives complete coverage across all projects without per-repo setup. The documented snippet targets only the global config.

4. **Session id MUST survive `/compact`.** Claude Code's `session_id` field is stable across `/compact` — `/compact` does not produce a new session_id. Use Claude's `session_id` directly as the stable key. Derive our `SessionId` as `ses_` + first 26 Crockford-base32 characters of SHA-256(claude_session_id). This produces a valid `SessionId` (matches the `ses_` + 26-char validator), is stable for the entire Claude session including across `/compact`, and requires no shared state. The earlier `ses_hook_<prefix>` scheme is superseded by this decision.

5. **`duration_ms` source: the PreToolUse temp file carries the wall-clock timestamp.** The temp file written by `8v hook pre` includes the millisecond timestamp at hook invocation. `8v hook post` reads that timestamp and computes `duration_ms = post_timestamp − pre_timestamp`. If Claude Code provides timestamps in the hook input payload, those are preferred; wall-clock from the temp file is the fallback. Approximate duration via temp file mtime is not acceptable — mtime precision is OS-dependent and not reliable to millisecond granularity.
