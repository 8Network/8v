# o8v-core — Design Notes

## Architecture

```
o8v-project  →  o8v-core  →  o8v-cli
(what is it)    (is it ok)    (interface)
```

## Current State

- Check trait + ToolCheck adapter — external tools as checks
- 7 stacks with per-stack tool definitions, max strictness
- Timeout on subprocess execution (5 min, configurable later)
- Bounded output capture (1 MB)
- Tool-not-found detection (heuristic)
- Structured detection errors (not stringified)
- `#[non_exhaustive]` on CheckOutcome, `Debug` on all public types

## What Works

- Rust: cargo check + clippy (pedantic + nursery) + cargo fmt — all with --workspace
- TypeScript: ./node_modules/.bin/tsc + eslint — no npx resolution
- JavaScript: ./node_modules/.bin/eslint — no npx
- Python: ruff check --select ALL
- Go: go vet + staticcheck
- Deno: deno check (trusts deno.json config)
- DotNet: dotnet build --warnaserror (auto-discovers from project directory — detection already validated the target)

## Known Limitations

- **Unix-only.** Node tool resolution (`node_modules/.bin/<tool>`) and process group
  kill (`libc::killpg`) are Unix-specific. On Windows: Node shims are `.cmd` files
  (not resolved), process kill needs job objects (not implemented), and test helpers
  use `sh`/`true`/`sleep` (will fail). No Windows users exist today — this is deferred,
  not forgotten. See AI-ERRORS #188, #199, #201.

- **Deno check requires Deno 2.x.** `deno check` is called with no file arguments —
  it discovers files from `deno.json` config. This works in Deno 2.x but older versions
  (1.x) may exit 0 silently with no args. Adding explicit file arguments would override
  `deno.json` config, so we trust the tool's discovery. Minimum version: Deno 2.0.

## Needs Design Discussion

### Output Pagination
- Long tool output floods the terminal
- Need pagination or truncation strategy
- Decide: stream vs collect, page vs scroll, summary vs full

### Timeout Propagation / Override
- Current: hardcoded 5 min per check
- Need: configurable per-check, per-stack, or global
- Need: propagate timeout setting from CLI to library

### Workspace Scope
- ProjectKind passed to check planners but not yet used by JS/TS/Deno
- TypeScript workspaces may need per-member tsc runs
- Need design: does 8v check workspace members individually or as a group?

### Network Isolation
- 8v says "no network" but tools can trigger network access
- cargo check → downloads crates, dotnet build → NuGet restore, deno check → URL imports
- Need: --offline flags where supported? Document as known behavior?

### Future: 8v install
- When a tool is not installed, 8v reports error
- Future command: `8v install <tool>` to set up the stack's tools
- Separate design, separate conversation

## Testing (429 tests across workspace)

### Unit tests needed
- CheckResult::is_ok() with mixed outcomes
- CheckReport::is_ok() with detection errors
- checks_for() returns non-empty for each stack
- is_tool_not_found() heuristic with known patterns

### Integration tests needed
- Rust fixture: cargo check + clippy + fmt on clean project
- Rust fixture: workspace with broken member
- Tool not installed → CheckOutcome::Error (not Failed)
- Timeout behavior (mock slow command)
- Empty output → includes exit code in message

## Error Log

299 AI errors logged in AI-ERRORS.md. Key patterns from o8v-core development:
- npx package resolution is unreliable (use ./node_modules/.bin directly)
- File finders must match detection rules (files only, not directories)
- Tools have their own workspace/scope handling (trust the tool, don't override)
- Detection metadata must flow through to checks (not be discarded)
