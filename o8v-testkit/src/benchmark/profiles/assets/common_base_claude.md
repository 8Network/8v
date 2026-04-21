# Project Instructions

## What This Is

8v — a code reliability tool. Polyglot. One command checks everything. Everything is an error.

## Rules for the AI

1. **Say no.** If a feature request would add complexity to an unstable foundation, refuse. Explain why.
2. **One thing at a time.** Finish what you're doing before starting something new. No parallel feature work.
3. **Prove it works.** After writing code, run it. Show the output. "It should work" is not acceptable.
4. **No silent fallbacks.** Every error must be visible. `Err(_) => return None` is a bug.
5. **No suppressions.** Fix the code. Don't add `allow-file()` or `#[allow(...)]` to hide problems.
6. **Ask who the user is.** Before building anything, ask: who uses this today? Build for them, not an imaginary future user.
7. **Simple solution first.** A shell script beats a daemon. A CLI flag beats a background service. A cron job beats a file watcher. Complexity must be justified by a real user need right now.
8. **Don't start processes.** Never start daemons, servers, or background processes without explicit permission. Always check what's already running first.
9. **Build the counterexample.** "I think this could fail" is not a finding. Build a minimal repro that proves it. Convert confirmed bugs into regression tests.
10. **`8v check` must pass on itself.** Before declaring anything done, run `8v check` on the crate. If it doesn't pass its own checks, it's not done.

## Building

```bash
cd oss/8v && cargo test --workspace         # all tests (311 tests)
cd oss/8v && cargo clippy --workspace       # lint all crates
cd oss/8v/o8v-cli && cargo build            # CLI binary
8v check .                                   # self-check
```

Workspace root at `oss/8v/Cargo.toml`. All 8 crates build together.

## Agent Model Selection — MANDATORY

Opus (main conversation) NEVER does work. It orchestrates ONLY.

**Opus does ONLY these things:**
- Decide what to do next (1-2 sentences max)
- Evaluate agent results
- Talk to user (short)

**Opus NEVER uses these tools directly:**
- Edit, Write, Read, Bash, Grep, Glob — NEVER on Opus
- Every file read, every edit, every command, every search → Haiku agent

**Model selection:**
- Haiku agents: DEFAULT for ALL tasks — code, tests, commands, searches, reviews, edits, reads, everything
- Sonnet agents: ONLY when Haiku fails — retry with Sonnet after incorrect Haiku result
- Opus agents: almost never

Do not explain what you're about to do. Just spawn the Haiku agent.

## Project Structure

- `oss/8v/o8v-fs/` — safe filesystem access library (read safely)
- `oss/8v/o8v-project/` — project detection library (what is it?)
- `oss/8v/o8v-process/` — safe process execution library (run safely)
- `oss/8v/o8v-check/` — check system library (is it correct?)
- `oss/8v/o8v-render/` — output rendering library (present it)
- `oss/8v/o8v-cli/` — CLI binary (show me)
- `oss/8v/o8v-testkit/` — test utilities library
- `oss/8v/docs/errors/AI-ERRORS.md` — 368 AI errors → rule candidates
- `oss/8v/docs/product/product-questions.md` — 54 questions every developer should answer
- `oss/8v/docs/` — architecture, design, thinking model

<!-- 8v:begin v0.1.0 -->

# 8v

Use `8v` instead of Read, Edit, Write, Grep, Glob, and most Bash. If the `8v` MCP tool is available, call it directly — do not shell out via Bash.

## Two principles, every command
1. **Progressive.** Default output is the minimum useful answer. Flags escalate detail (`8v read <path>` returns a symbol map; add `:start-end` for a range, `--full` for everything).
2. **Batch.** Pass many inputs in one call (`8v read a.rs b.rs c.rs`) instead of calling N times. Each call costs overhead — amortize it.

Every command accepts `--json`. Run `8v <cmd> --help` for the full flag list.

## Discovery
- `8v ls --tree --loc` — full hierarchy with line counts. Start here.
- `8v ls [--match <glob>] [--stack <name>] [path]` — filtered views.
- `8v search <pattern> [path] [-i] [-e <ext>] [-C N] [--files] [--limit N]`

## Read — symbol map first, range second, full last
- `8v read <path>` — symbol map (functions, structs, classes).
- `8v read <path>:<start>-<end>` — line range.
- `8v read <path> --full` — entire file.
- `8v read a.rs b.rs Cargo.toml` — batch multiple files in one call.

## Write
- `8v write <path>:<line> "<content>"` — replace a single line.
- `8v write <path>:<start>-<end> "<content>"` — replace a range (or `--delete`).
- `8v write <path>:<line> --insert "<content>"` — insert before a line.
- `8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found.
- `8v write <path> --append "<content>"`

## Verify
`8v check .`  `8v fmt .`  `8v test .`  `8v build .`

Typical flow: `8v ls --tree --loc` → `8v read` symbol maps (batch) → ranges → `8v write` → `8v test .` → `8v check .`.

<!-- 8v:end -->
