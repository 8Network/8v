# Command audit — progressive, batch, output quality

**Date:** 2026-04-16.
**Status:** Lab notes + post-freeze punch list. Not an implementation plan.
**Author:** Claude (per founder direction).

## What we learned

Measured on fix-failing-test N=3 after trimming the MCP tool description
(2787 → 1660 bytes) and the `CLAUDE.md` AI section (3840 → 1660 bytes)
and rewriting both to lead with the two principles:

- Tokens: 141,881 → 101,584 (−28%)
- Cost:   $0.2409 → $0.0916 (−62%)
- cache_creation: 27,076 → 15,401 (−43%)
- Behavior: identical deterministic 5-tool sequence across all 3 runs,
  CV on tokens = 0.02%, zero landmines.

**Lever identified:** the agent-facing prompt surface (tool description
+ system-prompt section) dominated cost. Trimming to ~1.7× smaller and
leading with principles cut cost by 62%. 8v is now cheaper than the
native baseline on this task.

**Principle the agent applied:** given `8v ls --tree --loc` in the
instructions as the canonical first step, the agent used it verbatim —
one call, combined flags, not three. Progressive-and-batch is taught
by showing the combined form once, not by listing every flag.

## The two principles

Every 8v command should satisfy both. They are not options, they are
the bar:

1. **Progressive.** Default output is the cheapest useful answer. Flags
   escalate detail only when asked.
2. **Batch.** Accept many inputs in one call. Schema tax + turn
   overhead is paid per call — amortize it.

A third concern surfaced after the benchmark: **output quality.**
Progressive+batch control the *size* of output. Quality is whether the
output tells the agent what to do next. A 30-line build-failure dump
is small but useless if it doesn't surface the real error.

## Audit table

| Command | Progressive? | Batch? | Default output cap | Notes |
|---------|--------------|--------|---------------------|-------|
| `read`  | ✓ symbols → range → full | ✓ multi-path | symbol map | The reference implementation of the principles. |
| `ls`    | ✓ projects → `--tree` → `--loc`/`--meta` | ✗ one path | projects only | Flag combinators (`--tree --loc --match --stack`) in one call approximate batching. |
| `search`| partial — `-C` adds context, `--files` toggles mode | ✗ one pattern, one path | 20 files × 10 matches | Multi-pattern would be a real batch win. |
| `write` | partial — modes (`--insert`, `--delete`, `--find/--replace`, `--append`) | ✗ one file, one op | N/A | Batch here would mean a declarative edit list (path, op) in one call. |
| `check` | ✓ `--limit 10`, `--verbose`, `--page` | ✗ one path | 10 lines per check | Default `--limit 10` may hide signal when many diagnostics exist. |
| `fmt`   | ✓ `--check` mode, `--verbose` | ✗ one path | *no limit* | If fmt fixes 300 files the output is unbounded. |
| `build` | ✓ `--limit 30`, `--page` | ✗ one path | 30 lines per section | Stdout/stderr capped; errors at the bottom may be truncated. |
| `test`  | ✓ `--limit 30`, `--page` | ✗ one path | 30 lines per section | Same as build — truncation can hide the failing test name. |

## Output quality deep dive

The founder's concern: *"check, build, fmt didn't have proper output."*

What we have today:

- **`check`** emits one line per diagnostic: `path:line:col severity kind message`.
  This is good — structured, greppable, paginable. Default `--limit 10`
  per check is a concern if there are 50 dead-code warnings burying one
  real error.
- **`build`** / **`test`** capture cargo/pytest stdout and stderr,
  truncated to `--limit 30` lines per section, paginated. The *first*
  30 lines is usually "Compiling X" progress — the real error can land
  past line 30 and get paginated to oblivion. Agent has to guess to
  page.
- **`fmt`** has no default cap. A large fmt run produces unbounded
  output. Rarely a problem in practice but a real failure mode.

**What "proper output" would mean:** the first ~30 lines of a failing
command should always be *why* it failed, not *what it was doing*.
Errors first, noise second. Today the cap is mechanical (head of
stderr) rather than semantic (error summary).

## Post-freeze punch list

Ordered by value / cost.

1. **Errors-first truncation for build/test.**
   Extract the failing-test name and error frame, surface those first,
   then the head of noise. Current stacks parse this already for
   `check` — reuse that parser for `test`/`build` stderr.

2. **`fmt` should have a default `--limit`.** Match `check` at 10 lines
   or `build` at 30.

3. **Multi-path for `ls`, `search`, `check`, `fmt`, `test`, `build`.**
   One `8v check a b c` beats three calls. Needs a `Vec<String>` path
   arg and per-path result grouping in the report.

4. **Multi-pattern `search`.** `8v search pat1 pat2 --path dir`. Less
   obvious value than multi-path but real.

5. **Declarative multi-edit `write`.** A JSON list of ops or a
   `--edits` file. Much larger design question — defer until concrete
   demand shows up.

6. **Default-limit review.** `check --limit 10` is aggressive given
   real projects have dozens of warnings. Consider `--limit 30` to
   match build/test, or a "summary + top N" default shape.

## What's documented vs not (instructions + ai_section)

Flags that exist in code but are not in the trimmed instructions:

- `ls`: `--ext`, `--meta`, `--depth` (we mention `--depth` in old form
  but not new; `--ext`/`--meta` not at all)
- `search`: `--max-per-file`, `--page`
- `check`, `fmt`, `test`, `build`: `--page`, `--limit`, `--timeout`

**Rule:** the instructions teach principles and the happy path. `8v
<cmd> --help` is the reference. Duplicating the reference in the prompt
burns tokens and goes stale. Keep the prompt thin.

## Not covered by this audit

- `init`, `hooks`, `upgrade`, `mcp` — operational commands, not
  agent-facing in the hot path. The audit is for commands the agent
  calls during a task.

## Open questions

- **Is multi-path `check` worth breaking the single-project assumption?**
  Right now `check .` detects projects under the path. Multi-path means
  "check these specific projects". Different semantics. Needs a design
  pass, not a flag bolt-on.
- **Should `write` accept a batch of ops?** Agents today make 5–10
  writes per task. If each write pays a turn, batching could save
  turns. But atomicity + partial failure gets hairy. Defer.
