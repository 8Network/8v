# Multi-Agent Init Design

## Status: Draft — pending validation from benchmark findings

## Problem

`8v init` assumes Claude Code is the only agent. It writes Claude-specific files
(`.claude/settings.json`, `CLAUDE.md`) without asking. Users who use Codex, or
both agents, get no setup.

## Design

### Agent selection

`8v init` asks which agents the user wants configured:

```
Which agents do you use?
  [x] Claude Code
  [x] OpenAI Codex
```

Multi-select. At least one required. `--yes` defaults to all detected agents
(agents whose CLI binary is in PATH).

### Per-agent configuration

Each agent gets its own setup module. All follow the same pattern:
read existing config → merge 8v entry → write back.

#### Claude Code (existing)

| File | Purpose |
|------|---------|
| `.mcp.json` | Register `8v` as MCP server |
| `CLAUDE.md` | Append 8v usage instructions |
| `.claude/settings.json` | Allow `mcp__8v__8v`, deny native tools |

#### OpenAI Codex (new)

| File | Purpose |
|------|---------|
| `.codex/config.toml` | Register `8v` as MCP server under `[mcp_servers.8v]` |
| `AGENTS.md` | Append 8v usage instructions (Codex reads AGENTS.md) |

Codex config format:
```toml
[mcp_servers.8v]
command = "8v"
args = ["mcp"]
```

Merge rules (same pattern as `.mcp.json`):
- File doesn't exist → create with 8v entry
- File exists, no `[mcp_servers.8v]` → add it, preserve everything else
- File exists, `[mcp_servers.8v]` present → leave untouched
- File exists, not valid TOML → error

#### AGENTS.md (shared)

Both Claude and Codex read `AGENTS.md`. The 8v section is written once,
shared by both agents. `CLAUDE.md` gets the same section for Claude
(Claude reads CLAUDE.md, Codex reads AGENTS.md).

### Trust model

Codex ignores `.codex/config.toml` unless the project is trusted in
`~/.codex/config.toml`. `8v init` does NOT modify global Codex config.
Instead, it prints:

```
Note: Run `codex` once in this directory to approve project trust.
```

### Detection

`8v init` detects installed agents by checking PATH:
- `claude` binary → Claude Code available
- `codex` binary → Codex available

Detection informs defaults in `--yes` mode and the selection prompt.

### `--yes` behavior

Non-interactive mode configures all detected agents. If no agents are
detected, configures for Claude (backward compatible — Claude might be
installed via npm globally or as a desktop app where the binary isn't
directly in PATH).

## Findings from manual testing (2026-04-16)

### Codex project-local config WORKS

`.codex/config.toml` with `[mcp_servers.8v]` registers the server. Codex
launches it and the agent sees it. Confirmed with `codex exec --json`.

### MCP tool approval is the blocker

`--full-auto` only auto-approves shell commands. MCP tool calls require
separate approval. With `--full-auto`, every MCP call fails with
"user cancelled MCP tool call".

`--dangerously-bypass-approvals-and-sandbox` makes it work — 8v MCP tool
returns results, agent uses them correctly.

Per-tool approval in config (`[mcp_servers.8v.tools.8v] approval_mode = "approve"`)
did NOT work in project-local config during testing. This may be a Codex bug
or a trust-level interaction. Needs further investigation.

**For benchmarks:** `--dangerously-bypass-approvals-and-sandbox` is acceptable
(controlled test environment).

**For init:** This is the key open question. `8v init` writes the config, but
the user needs to approve the MCP tool either:
- Interactively (Codex prompts on first use)
- Via global config approval
- Via trust level + per-tool approval (not working yet)

### Codex JSONL format (resolved)

`codex exec --json` produces NDJSON with these event types:
- `thread.started` — session ID
- `turn.started` / `turn.completed` — turn boundaries with `usage` (input, output, cached tokens)
- `item.completed` type `agent_message` — assistant text
- `item.completed` type `command_execution` — shell tool (command, output, exit_code)
- `item.completed` type `mcp_tool_call` — MCP calls (server, tool, arguments, result, error)

No cost data. No model ID in output.

### Token overhead

24K input tokens for "what is 2+2?" — Codex system prompt is heavy.
Compare to Claude's ~13K per turn with 8v MCP schema.

## Open questions

1. **MCP approval in init**: How to auto-approve 8v MCP tool for Codex users?
   Project-local `approval_mode` didn't work. May need global config or
   Codex interactive approval on first use.

2. **Codex native tool denial**: AGENTS.md instructions are the only lever.
   `disabled_tools` on shell server may work but needs testing.

3. **Web search**: Codex has `--search` (opt-in). Leave to user runtime choice.

4. **Sandbox mode**: `workspace-write` is the right default for coding tasks.
   Should `8v init` set this in `.codex/config.toml`?
