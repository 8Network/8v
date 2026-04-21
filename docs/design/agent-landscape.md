# Agent Landscape & 8v Positioning

*Started: 2026-04-18. Living document — update as we learn.*

---

## The problem agents have today

Every coding agent (Aider, Continue, OpenHands, Claude Code, Codex) does the same thing:

1. Call `read_file` → get raw file contents (full context, even if 2000 lines)
2. Call `write_file` → overwrite the whole file
3. Call `bash` → run arbitrary shell commands
4. Loop until it compiles or gives up

This works but is expensive and fragile:
- **Context explosion** — full files in every turn, even for a 1-line fix
- **No structure** — bash errors are raw strings, no schema
- **No cost control** — agent reads what it wants, when it wants
- **Retry loops** — quoting errors, path issues, no guardrails

We measured this tonight: qwen2.5-coder:14b spent 13 tool calls looping on Cargo.toml quoting errors. The agent knew what to do but the tool layer failed it.

---

## The agent tools landscape (as of April 2026)

### Aider
- **What**: CLI coding agent. Give it a task, it edits files, runs tests, fixes errors in a loop.
- **How**: Uses git diff format for edits. Supports Ollama (local models).
- **Strength**: Battle-tested edit loop. Handles compile errors well. Simple CLI.
- **Weakness**: Uses raw file tools. No structure below the model.
- **Relevant to 8v**: Aider is the benchmark to beat. If 8v-as-agent can do what Aider does but cheaper (fewer tokens, structured errors), that's the proof.
- **Status**: To be tested with qwen2.5-coder:14b.

### Continue.dev
- **What**: VS Code / JetBrains extension. Local model as inline coding assistant.
- **How**: Connects to Ollama. Context-aware completions + chat + edits.
- **Strength**: IDE-native, good UX, widely used.
- **Weakness**: Tool layer is raw file access. No efficiency layer.
- **Relevant to 8v**: 8v could be a Continue tool provider — replace raw reads with symbol maps.

### OpenHands (OpenDevin)
- **What**: Full agent environment. Docker sandbox, real shell, browser.
- **How**: Gives the model a full Linux environment. Most capable but heaviest.
- **Strength**: Can do anything — install packages, run servers, browse web.
- **Weakness**: Heavy infra, Docker required, slow. Raw tools, no cost control.
- **Relevant to 8v**: Too heavy for our use case. Interesting reference.

### Claude Code (this tool)
- **What**: Anthropic's CLI agent. What we're using right now.
- **How**: Uses MCP for tools. Has 8v as a registered tool.
- **Strength**: Best instruction following. Handles multi-step tasks well.
- **Weakness**: Requires Anthropic API (not local). Expensive at scale.
- **Relevant to 8v**: Our primary test environment. 8v is already in its tool loop.

### Codex (OpenAI)
- **What**: OpenAI's CLI agent. Similar to Claude Code.
- **How**: JSONL format, `apply_patch` for edits.
- **Relevant to 8v**: Second benchmark agent after Claude Code. Being tested.

---

## 8v's position in this stack

```
User task
    ↓
Agent loop (Aider / Continue / Claude Code / Codex / 8v-agent)
    ↓
Tool layer ← THIS IS WHERE 8V LIVES TODAY
    ↓
Filesystem / compiler / tests
```

**Today**: 8v is a better tool layer. Instead of `read_file` → 2000 lines, agents call `8v read` → symbol map (50 tokens). Instead of `bash cargo build` → raw error string, `8v build` → structured result.

**Tomorrow**: 8v IS the agent loop. It runs the task, calls its own tools, manages context, reports structured results. The agent *is* 8v. External agents become optional consumers of 8v's output.

---

## What we learned from tonight's experiment (2026-04-18)

We tried to build a local agent loop from scratch:
- Model: qwen2.5-coder:14b via Ollama
- Tool: 8v MCP
- Task: Build a Rust CLI calendar

**What broke:**
1. **Tool call format** — qwen2.5-coder:14b outputs tool calls as markdown JSON, not structured `tool_calls`. Needed custom parsing.
2. **Multi-line content in CLI** — writing multi-line Rust via `8v write` requires careful quoting. Models get this wrong.
3. **Cargo.toml loop** — model spent 10+ turns trying to fix a Cargo.toml it didn't understand. No guardrail stopped it.
4. **Timeout** — qwen3:8b timed out (300s) generating a large Rust file in a single response.

**What worked:**
- MCP handshake + roots/list protocol
- `8v write` and `8v build` executed correctly when called with proper syntax
- The model knew the right solution — the tool layer failed it, not the reasoning

**Key finding**: The agent loop (retry, error feedback, tool execution) is the hard part — not the model's knowledge. Aider has 3 years of solving exactly this. We should use it rather than rebuild it.

---

## Decision: No integration with Aider or Continue.dev (2026-04-18)

**Decision**: 8v will not integrate with Aider, Continue.dev, or similar agent tools. Not now, not planned.

**Why not:**

Aider has `--lint-cmd` and `--test-cmd`. Any developer can point those at `8v check .` in 30 seconds. There is nothing 8v-specific to add. Writing `.aider.conf.yml` from `8v init` would be noise, not value.

More fundamentally: Aider and Continue are built on raw file tools. Their architecture is the problem 8v is solving. Wrapping them with 8v lint hooks is a band-aid on the wrong layer.

These tools are also bad. The UX is poor, the agent loop is fragile, and they are not where the market is going.

**Where 8v integrates:**

Only with agents that use MCP — Claude Code, Codex, and what comes next. That is the correct integration point.

---

## Next steps

- [x] Tested Aider with qwen2.5-coder:14b — compiled first try, Aider works
- [x] Decided: no integration with Aider or Continue.dev
- [ ] Define what "8v as agent" means concretely — what does it own vs delegate?
