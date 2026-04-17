# Design — errors-first truncation for build/test output

**Status:** DRAFT v3 — awaiting review.
**Date:** 2026-04-16.
**Related:** `docs/design/command-audit-2026-04-16.md` (item #1 on the post-freeze list).
**Supersedes:** v1, v2 (both reviewed adversarially; blockers called out under **Review findings addressed**).

## Problem

`8v build` and `8v test` capture stdout/stderr from cargo/pytest/etc.
and truncate to `--limit 30` lines per section, paginated. The
truncation is mechanical — the head of the captured stream.

Cargo's default output pattern is:

```
    Compiling crate1 v0.1.0
    Compiling crate2 v0.1.0
    ...
    Compiling crateN v0.1.0            ← 30+ lines of progress
error[E0433]: failed to resolve ...     ← the signal, paginated away
```

Page 1 is all noise. The agent has to page, which burns a turn. On a
large workspace the real error lands on page 2 or 3, and the agent
(which doesn't know that) may give up and ask the human.

`check` doesn't have this problem because `CheckReport` is already
structured — one diagnostic per entry, severity-sortable, paginated by
item. `build` and `test` are unstructured captured streams.

## Goal

When `build` or `test` fails, the first ~30 lines the agent sees must
be the error, not the progress.

## Proposal

Reuse the existing `Diagnostic` type and the existing NDJSON parser.
Add no new type. Add no new parser. Surface structured errors in a
preamble above the truncated stderr tail.

The primary extraction path per stack:

- **Rust — `cargo build`**: add `--message-format=json` (stable). Feed
  stdout to the existing `o8v_stacks::parse::cargo::parse`. It returns
  `Vec<Diagnostic>` from `o8v_core::diagnostic::Diagnostic` — the same
  type `8v check` already uses. Dedup, external-path filtering, and
  security tests come for free.
- **Rust — `cargo test`**: libtest's default human output does not
  expose failures via `--message-format=json` (that flag surfaces only
  compile errors, not test-result events). Use the libtest JSON event
  stream instead:
  ```
  cargo test --message-format=json -- -Z unstable-options \
      --format=json --report-time
  ```
  This **requires a nightly toolchain** on the host. A per-stack
  `libtest_json` converter (new, small — in `o8v-stacks/src/parse/`)
  maps `{type:"test",event:"failed",name,stdout}` events into
  `Diagnostic` values (severity=Error, rule=test name, message=first
  panic line, notes=full captured stdout). Stable fallback: no
  extraction, mechanical truncation as today (the current behaviour).
- **Python — pytest**: `--json-report` when the plugin is present;
  regex on `FAILED` + traceback block otherwise. Convert to
  `Diagnostic`.
- **Go — `go test`**: `-json`. Convert event stream to `Diagnostic`.
  For `go build`, regex on `*.go:L:C: error: msg` (no JSON mode).
- **TypeScript — tsc**: regex on `file.ts(L,C): error TSNNNN: msg`.
  No stable JSON mode.
- **Other stacks**: no extractor. `extract_errors` returns the empty
  vec and a `tracing::debug!` line fires.

Regex-based extraction is a per-stack fallback, owned by the stack
itself. It is never the main path where a structured format exists.

Dispatch is via a typed trait method on `StackTools`, not a string
match on stack name.

### Shape of `render_plain` output (failure case)

```
$ cargo build
exit: 101 (failure)
duration: 2.4s

errors (2):
  error[E0433] unresolved import `foo::bar`
    --> src/lib.rs:42:5
    | use foo::bar;
    |     ^^^^^^^^ no `bar` in `foo`
  error: aborting due to previous error

stderr (first 10 lines):
    Compiling crate1 v0.1.0
    Compiling crate2 v0.1.0
    ...
... 847 more lines (--page 2 for next 30)
```

Success case is unchanged (no diagnostics to hoist, no preamble).

## Typed dispatch

Extend `StackTools` in `o8v-stacks/src/stack_tools.rs` with a method
handle, alongside `checks`/`formatter`/`test_runner`/`build_tool`:

```rust
pub struct StackTools {
    pub checks: Vec<Box<dyn Check>>,
    pub formatter: Option<FormatTool>,
    pub test_runner: Option<TestTool>,
    pub build_tool: Option<BuildTool>,
    pub error_extractor: Option<ErrorExtractor>, // NEW
}

pub struct ErrorExtractor {
    /// Called only on failure (exit_code != 0).
    /// Returns `Vec<Diagnostic>` — reusing o8v_core's existing type.
    pub extract: fn(stdout: &str, stderr: &str, project_root: &Path, kind: RunKind)
                 -> Vec<o8v_core::diagnostic::Diagnostic>,
}

pub enum RunKind { Build, Test }
```

The `RunKind` is needed because a stack can need a different parser
for build vs test (rust: stable cargo JSON vs nightly libtest JSON).
Each stack file (`stacks/<name>.rs`) registers its own extractor.

## Types — no new struct

Reuse `o8v_core::diagnostic::Diagnostic` end-to-end. It already
carries severity, rule, location, span, snippet, related spans, notes,
suggestions, tool, stack. A narrower "ErrorFrame" (v2's proposal)
threw away severity/rule/span/suggestions and would have forced a
second adapter in every consumer.

`errors: Vec<Diagnostic>` is added to both `BuildReport` and
`TestReport` (see **Report duplication** below).

## Report duplication — acknowledged

`BuildReport` and `TestReport` (in `o8v-core/src/render/`) are
near-identical today — same fields, same render_plain, same
render_json, differing only in type name. v3 adds the same `errors`
field to both and accepts the duplication for now. Introducing a
shared `OutputReport<Kind>` wrapper is a separate refactor; this
design does not block on it. Tracked as a follow-up: consolidate once
errors-first lands and we see whether test-specific rendering
(per-test-name sections, flake indicators, etc.) diverges from build.

## Crate dependency direction

`o8v-stacks` already depends on `o8v-core`
(`o8v-stacks/Cargo.toml` line 13:
`o8v-core = { path = "../o8v-core", version = "0.1.0" }`). Returning
`Vec<o8v_core::diagnostic::Diagnostic>` from a stack extractor keeps
the direction intact. No new crate dependency is introduced.

## Where the code goes

Every file this change touches, path-verified:

### Stacks (`o8v-stacks`)
- `src/stack_tools.rs` — add `error_extractor: Option<ErrorExtractor>`
  and the `ErrorExtractor` struct and `RunKind` enum.
- `src/stacks/rust.rs` — (a) change `test_runner.args` from
  `&["test", "--workspace"]` to include
  `&["test", "--workspace", "--message-format=json", "--", "-Z",
  "unstable-options", "--format=json", "--report-time"]` when nightly
  is detected; fall back to plain args otherwise. (b) change
  `build_tool.args` from `&["build"]` to
  `&["build", "--message-format=json"]`. (c) register
  `ErrorExtractor { extract: rust_extract }` which dispatches on
  `RunKind`: `Build` → `parse::cargo::parse`; `Test` →
  `parse::libtest_json::parse` (new, small).
- `src/parse/libtest_json.rs` — **new file.** Parses libtest NDJSON
  events; reuses the same NDJSON-per-line pattern as
  `parse::cargo::parse` (partial/interrupted output is tolerated by
  `serde_json::from_str` per-line — same idiom).
- `src/stacks/python.rs`, `go.rs`, `typescript.rs` — add extractors.
- `src/stacks/{javascript,ruby,java,kotlin,swift,dotnet,deno,erlang,dockerfile,helm,terraform,kustomize,shell}.rs`
  — set `error_extractor: None`.

### Core types (`o8v-core`)
- `src/process_report.rs` — **no change.** `ProcessReport` stays a
  pure captured-output type. Diagnostics are a render-time concern.
- `src/render/build_report.rs` — add
  `pub errors: Vec<o8v_core::diagnostic::Diagnostic>` to
  `BuildReport`, render before stderr when non-empty and
  `render_config.errors_first` is on.
- `src/render/test_report.rs` — same addition to `TestReport`.
- `src/render/run_report.rs` — `render_process_output` signature
  gains `errors: &[Diagnostic]` parameter and renders the preamble.
  A small `render_diagnostic_preamble` helper lives alongside.
- `src/render/mod.rs` — `RenderConfig` gains `pub errors_first: bool`
  (default `true`).

### Command layer (`o8v`)
- `src/commands/build.rs`, `src/commands/test.rs` — accept
  `--errors-first` / `--no-errors-first` flag; on failure and when
  `errors_first` is on, resolve the project's `StackTools`, call
  `tools.error_extractor.as_ref().map(|e| (e.extract)(...))`, put the
  result on the report; when flag is off, skip extraction entirely
  (don't compute and discard). On success, skip extraction.

### Tests
- `o8v-stacks/src/parse/libtest_json.rs` unit tests (zero/one/many
  failed tests, interrupted NDJSON, assert panic, ignored tests).
- `o8v-stacks/src/stacks/rust.rs` extractor unit tests (build path,
  test path, adapter from `Diagnostic` chain).
- Per-stack extractor tests for python/go/typescript.
- `o8v/tests/e2e_build.rs` — integration test against
  `tests/fixtures/build-rust-broken`: assert `errors (` appears
  before `stderr:` and contains the expected `error[E`.
- `o8v/tests/e2e_build.rs` — regression test: successful build
  (`build-rust` fixture) produces byte-identical output to today with
  `--errors-first` on (no diagnostics → no preamble).
- New `o8v/tests/e2e_test.rs` file mirroring `e2e_build.rs` for
  `8v test` (nightly-gated test; skip with a `if !nightly { return }`
  guard — same pattern the workspace already uses).
- Fallback test: stack with `error_extractor: None` emits the
  `tracing::debug!` line and falls back to current behavior.

Note on snapshot paths: v2 cited `o8v-core/tests/snapshots/*`. That
directory does not exist. `o8v-core/tests/` contains `e2e_smoke.rs`,
`e2e_stack_dotnet.rs`, `e2e_stack_node.rs`, `e2e_violations.rs`,
`fixtures/` — no insta snapshots. The actual e2e tests for `build`
and `test` live in `o8v/tests/e2e_build.rs` and (to be created)
`o8v/tests/e2e_test.rs`. They are assertion-based, not snapshot-based
(`grep -n 'snapshot\|insta' o8v/tests/e2e_build.rs` returns nothing).
Re-audit before landing: `grep -rn 'cargo build' o8v/tests/fixtures/`
and `grep -rn 'cargo test' o8v/tests/fixtures/` for anything that
asserts on raw cargo output.

## Flag

`8v build` and `8v test` accept:

- `--errors-first` (default **on**): hoist diagnostics above stderr.
- `--no-errors-first`: suppress the preamble; render unchanged
  stdout/stderr only.

**One flag, two decisions.** The flag is read twice, at different
layers:

1. **Command layer (extraction).** `build.rs`/`test.rs` skip the
   extractor call entirely when the flag is off or when the run
   succeeded. No wasted parse work.
2. **Render layer (placement).** `render_process_output` places the
   preamble only if `errors_first` is on *and* the passed
   `&[Diagnostic]` slice is non-empty.

Both sites must check the flag — the render layer cannot assume
"frames present ⇒ render them" because the command layer might have
chosen to extract for a --json consumer that doesn't want the plain
preamble. In practice the two decisions align, but keeping them
separate avoids coupling.

Default **on** because the whole point is to help the agent by
default. The flag exists to make the change controlled, not opt-in.

## Nightly toolchain requirement

`cargo test` error hoisting requires a nightly rustc on the host
machine (the `-Z unstable-options --format=json` flags are
nightly-only). The rust stack detects this:

- At extractor call time, check `rustc --version` for "-nightly".
- If stable: emit `tracing::debug!("rust test extractor: stable \
  toolchain, falling back to mechanical truncation")` and return the
  empty vec. The CLI flow then renders the plain truncated tail —
  today's behaviour. No error, no user-visible regression.
- If nightly: invoke with libtest JSON flags and parse.

`cargo build` needs no nightly; `--message-format=json` is stable.

CI implication: the benchmark harness (and developer machines that
want the full experience) should use `rust-toolchain.toml` with
`channel = "nightly"` or have nightly available via `rustup toolchain
install nightly`. Document this in the benchmark README.

## Hoisted diagnostic cap

Cap = **10**. More than ten, the agent should look at the fixture,
not the dump. The 11th+ diagnostic is omitted with a
`... N more diagnostics (see stderr or --limit)` line.

The hoisted block has its own 10-item budget. It does **not** count
against `--limit 30`; the stderr tail still gets its full line
budget.

## Fallback and trace idiom

Unknown stack (`error_extractor: None`), or extractor returned zero
diagnostics despite failure:

```rust
tracing::debug!(
    stack = %stack_name,
    kind = ?run_kind,
    "extract_errors: unsupported stack or empty result, \
     falling back to mechanical truncation"
);
```

Aligned with the existing idiom in `o8v-stacks/src/parse/cargo.rs:39`
(`tracing::debug!(line, "skipping non-JSON line in cargo output")`).
No `events::trace` — that identifier is not used anywhere in the
workspace (grep returned zero hits).

## Partial / interrupted NDJSON

An interrupted `cargo build` or `cargo test` may emit a truncated
final NDJSON line. The existing parser in `parse::cargo::parse`
already handles this correctly: each line is `serde_json::from_str`'d
independently, and a failing line logs `tracing::debug!` and
continues (cargo.rs lines 38–41). The new `libtest_json` parser
reuses the exact same pattern. No extra handling needed.

## `8v check` is unaffected

`8v check` uses `StackTools.checks` (a `Vec<Box<dyn Check>>`), which
is independent of `test_runner`, `build_tool`, and the new
`error_extractor`. The cargo JSON parser `check` already uses is
untouched by this change. Regression surface for `check` is zero.

## Non-goals

- Building a full diagnostic parser beyond what `parse::cargo` and
  the new `libtest_json` already cover.
- Sorting / deduping / severity-filtering across stacks. Cargo's
  parser dedups its lib+test double-pass; we inherit that.
- Changing `check`.
- Making `ProcessReport` itself semantic. It stays a capture type.
- Consolidating `BuildReport` and `TestReport`. Tracked separately.

## Review findings addressed

| # | Reviewer concern                                          | Resolution in v3                                                                   |
|---|------------------------------------------------------------|-------------------------------------------------------------------------------------|
| B1 | v2: `cargo test --message-format=json` surfaces test failures — **false.** | Use nightly `-Z unstable-options --format=json --report-time`; document requirement; stable fallback = mechanical truncation. |
| B2 | v2: new `ErrorFrame` struct loses `Diagnostic` fidelity.  | Drop `ErrorFrame`. Reuse `o8v_core::diagnostic::Diagnostic` everywhere.             |
| 3  | Snapshot paths `o8v-core/tests/snapshots/*` don't exist.  | Correct enumeration: `o8v/tests/e2e_build.rs` (assertion-based, not snapshot) and a new `o8v/tests/e2e_test.rs`. |
| 4  | `BuildReport` / `TestReport` duplication.                 | Acknowledged. Accepted for now; consolidation tracked as follow-up.                 |
| 5  | Crate dependency direction.                               | Confirmed intact. `o8v-stacks → o8v-core` already in Cargo.toml line 13.            |
| 6  | `errors_first` decision site ambiguous.                   | Explicit: command layer decides extraction, render layer decides placement. One flag, two sites. |
| 7  | Trace idiom.                                              | `tracing::debug!`, aligned with `parse/cargo.rs:39`. `events::trace` is not used in the workspace. |
| 8  | Is `8v check` affected?                                   | No. `check` uses `StackTools.checks`, disjoint from `test_runner`/`build_tool`/`error_extractor`. |
| 9  | Interrupted NDJSON.                                       | Handled by per-line `serde_json::from_str` (existing pattern, reused).              |
| v1.1 | Reuse existing cargo JSON parser.                       | Done — rust build path is unchanged parser.                                          |
| v1.2 | Typed dispatch, not string match.                        | Done — `StackTools.error_extractor` + `RunKind` enum.                                |

## Migration

**Benchmark harness.** The stderr layout for failing `build`/`test`
runs changes (new preamble). Baseline token counts shift. Action:
re-run N=6 baselines for every task shape that contains a failing
build or test after this lands. Do not compare pre/post across this
change — the measurement surface moved. `fix-test`, `fix-go`,
`diagnose` shapes are the affected baselines. Record the pre/post
numbers explicitly in `learnings` so the shift is attributable.

**Nightly toolchain in CI.** The benchmark runner must have nightly
installed for `cargo test` extraction to exercise. Add to the
benchmark CI doc: `rustup toolchain install nightly`. The 8v release
build itself stays on stable; only the target-project invocation uses
nightly.

**Snapshot tests.** None to regenerate — no insta snapshots cover
build/test output today. The relevant tests in `o8v/tests/e2e_build.rs`
are assertion-based and will either pass unchanged (success path) or
need a new assertion added (failure path — the new preamble). Added,
not regenerated.

**Rollout.** One PR, flag-gated. Plan:
1. Land extractor infrastructure + rust build path.
2. Land rust test path (nightly-gated).
3. Land python / go / typescript extractors.
4. Re-run baselines.
5. Update benchmark docs.
6. Delete v1 / v2 of this design.
