# Design: MCP Adapter Output Cap

**Status:** Draft · 2026-04-19  
**Slice:** pre-flight + post-render guard only — no new commands, no new flags

## 1. Problem

Claude Code MCP client rejects responses above ~60,500 chars and silently writes to disk above ~57,000 chars. 8v has zero output gate today. `parse.rs:9` caps input at 65,536 chars; nothing caps output. A single `8v read --full` on a large file will overflow the transport.

## 2. Scope

Two guards only: **pre-flight** (abort before reading if raw bytes would exceed cap) and **post-render** (replace any oversized output with a structured error before returning). No new commands, flags, or CLI path changes.

## 3. Cap Value and Override

Default cap: **55,000 chars** (safety margin under the ~57K persist threshold; see R2 note below).  
Override: env var `O8V_MCP_OUTPUT_CAP` — parsed on the first `handle_command` call and cached for the process lifetime via `std::sync::OnceLock<usize>` (handler may be driven by a multi-threaded Tokio runtime; `OnceLock` is the standard thread-safe lazy init in stdlib, not `static mut`). Valid only if the parsed value is a positive integer (`> 0`). Any other value — zero, negative, non-numeric, empty string — returns a distinct observable error immediately, before any command is executed (see §9 Test 4).  
The 1.20× overhead factor (line-number prefixes + headers) is absorbed into the margin (see R3 note below).

## 4. Pre-flight Check

**Where:** `o8v/src/mcp/handler.rs`, `handle_command` — after the `ParseOutcome::Parsed(cmd, argv)` destructure at line 65, before `dispatch_command_with_agent` at line 67. Zero changes to `read.rs`.

**Source anchor:** handler.rs lines 59–67 —
```
let (parsed_command, argv) = match super::parse::parse_mcp_command(...)? {
    ParseOutcome::Parsed(cmd, argv) => (cmd, argv)          // line 61
...                                                          // line 65
crate::commands::dispatch_command_with_agent(...)            // line 67
```
Pre-flight inserts between lines 65 and 67.

**Condition:** `if let Command::Read(args) = &parsed_command { if args.full { ... } }`

**Logic:** Sum `std::fs::metadata(path).len()` for every path in `args.paths`. Multiply by 1.20. If product > cap, return `Err(structured_error)` immediately. Metadata reads are cheap — no content loaded. If a path's metadata fails, skip it in the sum and let dispatch handle the real error.

## 5. Post-render Safety Net

**Where:** `o8v/src/mcp/handler.rs`, `handle_command` lines 77–83 — inside the `Ok((out, _exit, use_stderr))` arm, BEFORE branching on `use_stderr`. This wraps both return paths (`Err(out)` when `use_stderr` and `Ok(out)` otherwise). The check must execute once against `out` before either branch:

```
Ok((out, _exit, use_stderr)) => {
    if out.len() > cap { return Err(oversized_error(out.len(), cap, command)); }
    if use_stderr { Err(out) } else { Ok(out) }
}
```

All subcommands share this path. The `Err(e) => Err(...)` arm is unaffected.

## 6. Structured Error Message

Plain text, always `Err(...)` so the agent sees `is_error: true`. Implementation must match this template verbatim (indentation included) for test stability:

```
Error: output too large for MCP transport
  output:  <N> chars
  cap:     55000 chars (override: O8V_MCP_OUTPUT_CAP)
  command: <original command string>

Use a line range instead of --full:
  8v read <path>:<start>-<end>
Or read the symbol map first:
  8v read <path>
```

## 7. Error Shape Contract

`handle_command` returns `Result<String, String>`. `Err(text)` is delivered as `is_error: true` via the rmcp tool macro. No new types needed.

## 8. What Does Not Change

CLI path: no cap applied. `parse.rs:9` input cap: unchanged. All other subcommands: post-render guard applies; pre-flight is read-specific only.

## 9. Test Strategy

Three failing-first integration tests in `o8v-cli`, same pattern as existing MCP parse tests:

**Test 1 — Pre-flight fires:** Set `O8V_MCP_OUTPUT_CAP=1000` and use a fixture ≥ 1001 bytes (≥ cap + 1 byte). Issue `read --full` via MCP caller. Assert: `Err` returned, `is_error: true`, structured error message matches §6 template, and the error message lists each file's byte size (pre-flight computes this from metadata; post-render cannot, proving pre-flight ran).

**Test 2 — Post-render fires:** Any MCP command that produces rendered output > cap. Use `O8V_MCP_OUTPUT_CAP=1000` and a fixture directory large enough that `ls --tree` output exceeds 1000 chars. Assert: `Err` returned, `is_error: true`, message matches §6 template.

**Test 3 — Cap override + under-cap passes:** `O8V_MCP_OUTPUT_CAP=100000`. Small file (< 1000 bytes). Issue `read --full` via MCP caller. Assert: `Ok` returned, content present.

**Test 4 — Invalid override values:** Parameterized over `O8V_MCP_OUTPUT_CAP` ∈ `{"0", "-1", "abc", ""}`. Each must produce a distinct observable error on first `handle_command` call, before any command executes.

**Test 5 — CLI not affected:** Same large fixture file, `read --full` via CLI caller (not MCP). Assert: content returned, no cap error.

## 10. Risk Triage

**R1 (resolved):** `use_stderr` branch no longer bypasses the check — §5 wraps both arms before branching.

**R2 (accepted, documented):** Persist threshold lower bound (~57K) is from empirical observation (see `docs/findings/mcp-transport-cap-2026-04-19.md`), not from Anthropic's spec. The 55K default provides a ~2K margin. If the threshold shifts, adjust the default; the env var override handles per-deployment tuning.

**R3 (accepted, documented):** The 1.20× overhead factor is validated only for single-file reads. Multi-file batch reads may produce higher overhead (repeated headers). If overhead exceeds 1.20×, the post-render guard catches it; the pre-flight may underestimate but cannot over-block.

## 11. Open Questions

None. All insertion points grounded by source read.

## 12. Non-Goals

Streaming output, per-subcommand caps, automatic chunking/pagination, changes to the `8v` tool schema or MCP server initialization.
