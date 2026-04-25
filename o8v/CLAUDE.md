<!-- 8v:begin v0.1.16 -->

# 8v

Begin every session by running `8v ls --tree` before any other tool call.

Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations. If an `8v` MCP tool is available, call it directly — do not shell out via Bash.

## Two principles, every command
1. **Progressive.** Default output is the minimum useful answer. Flags escalate detail.
2. **Batch.** Pass many inputs in one call instead of calling N times.

## Discovery
- `8v ls --tree --loc` — full hierarchy with line counts. Start here.
- `8v ls [--match <glob>] [--stack <name>] [path]` — filtered views.
- `8v search <pattern> [path] [-i] [-e <ext>] [-C N] [--files]` — regex search across files.

## Read — symbol map first, range second, full last
- `8v read <path>` — symbol map; each line is `<line-number>  <symbol>`. Use line numbers with `:start-end`.
- `8v read <path>:<start>-<end>` — line range (1-indexed, end inclusive).
- `8v read <path> --full` — entire file. Last resort.
- `8v read a.rs b.rs Cargo.toml` — batch multiple files or ranges in one call.

## Write
- `8v write <path>:<line> "<content>"` — replace a single line.
- `8v write <path>:<start>-<end> "<content>"` — replace a range.
- `8v write <path>:<start>-<end> --delete` — delete a range.
- `8v write <path>:<line> --insert "<content>"` — insert before a line.
- `8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found.
- `8v write <path> --append "<content>"`
  Content args: `\n` = newline, `\t` = tab, `\\` = backslash. Pass as literal two-char sequences — do not rely on shell interpolation.

## JSON Contracts

### `8v read <path>:<start>-<end> --json`
Output shape: `{"Range":{"path":"...","start":<usize>,"end":<usize>,"total_lines":<usize>,"lines":[...]}}`

- Variant key: `"Range"` (not `"range"` or `"RangeRead"`)
- Line fields: `"start"` and `"end"` (NOT `"start_line"` / `"end_line"`)
- Locked by: `o8v/tests/e2e_read_json_range_fields.rs`

## Verify
- `8v check .` — lint + type-check + format-check.
- `8v fmt .` — auto-format in place.
- `8v test .` — run tests.
- `8v build .` — compile.

<!-- 8v:end -->
