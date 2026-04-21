> **INVALIDATED 2026-04-20.** Bug did not reproduce after `/mcp` reconnect. The original measurement was against a stale MCP-server binary holding a pre-slice-1 build. See the stale-binary discipline learning in `agent-behavior-and-tool-reliability-2026-04-19.md`.

# Findings: `8v read file1 file2 --full` — batch full flag bug investigation

**Date:** 2026-04-19
**Investigator:** AI (Sonnet 4.6)
**Scope:** Static code trace only. No code changes. No commits.

---

## (a) Document

`/Users/soheilalizadeh/8/products/vast/oss/8v/docs/findings/read-batch-full-bug-2026-04-19.md`
This file: 58 lines.

---

## (b) Suspect file:line

`o8v/src/commands/read.rs:24` — the `#[arg(long, overrides_with = "full")]` annotation on `pub full: bool`.

No other suspect remains. Full pipeline traced and eliminated:
- `read_to_report` (lines 207–237): passes `args.full` (static bool) uniformly to every `read_one` call
- `read_one` (lines 57–184): `else if full { Full } else { Symbols }` — no per-iteration state
- `dispatch.rs:212–213`: uses pre-parsed `command.execute(ctx)` only; `argv` is logging-only
- `resolve_mcp_paths` / `mcp/path.rs`: absolutizes paths, does not touch `args.full`
- MCP cap checks (`handler.rs`): all-or-nothing; cannot cause per-file divergence
- Render side (`read_report.rs`): pure pattern-match on the variant received

---

## (c) Root cause (one sentence)

The code path as written is correct — `args.full` is passed uniformly to all files — so the bug, if confirmed real, must originate at the clap 4.6.0 parse layer, specifically in how `overrides_with = "full"` (self-referential override) interacts with `SetTrue` action when the flag appears once with multiple positional args; static analysis cannot confirm this because `overrides_with` / `overrides_with_self` semantics for `SetTrue` with N positional args have no test coverage in the current test suite.

---

## (d) Confidence

**3 / 5.**

- Ruling-out confidence is 5/5 for the entire downstream pipeline.
- Positive confirmation of the clap annotation as the cause requires a runtime test.
- Static reading: `overrides_with = "full"` on a `bool` with `SetTrue` should leave `full = true` after one `--full`. No flag flip mechanism visible. The bug may not be real at runtime — the reproduction table may have been observed in an MCP-mediated context where something else (e.g., argument tokenization in `parse_mcp_command` via `shlex::split`) strips or misplaces the flag position.

---

## (e) Surprises

1. **The design doc says `overrides_with_self = "full"`, the implementation uses `overrides_with = "full"`.** In clap 4 derive, `overrides_with = "full"` where `"full"` is the field's own long-arg name is functionally identical to `overrides_with_self`. This discrepancy is cosmetic, not behavioral.

2. **The previously implemented fix (multiple `--full` support) has zero test coverage for the actual failing case.** `read_double_full_flag_accepted` and `read_triple_full_flag_accepted_matches_single` both exercise a single file. Case 4 (`8v read file1 file2 --full`) — two plain files, one `--full` — has no test anywhere in `e2e_cli.rs` or `mcp_e2e.rs`.

3. **The 5-case reproduction table is asymmetric in a revealing way.** Cases 1–3 all have at least one `:range` arg; they pass. Case 4 is the only case with two plain (non-range) files. This isolates the bug to the `else if full { Full } else { Symbols }` branch — which is correct code — pointing back to whether `args.full` is actually `true` at parse time for Case 4.

---

## (f) 8v feedback

- `8v search` with `--limit` truncates without showing total match count when results are cut. I had to re-run without `--limit` to confirm I saw all matches. A "N of M matches shown" line would eliminate the re-run.
- `8v read <path>:16-35` with a relative path inside the MCP tool fails when cwd is not the workspace root. Absolute paths are required everywhere; the tool should resolve relative paths against a configurable base or reject them with a clear error citing the cwd.
