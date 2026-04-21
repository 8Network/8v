# Benchmark Scenarios

Each scenario defines one task that an AI agent must complete. Scenarios are
paired as (baseline, with-8v) to measure the effect of 8v on agent efficiency.

## How to run

Single scenario (one pass, no persistence):
```
cargo test --test agent_benchmark fix_test_8v -- --ignored --nocapture
```

Full experiment (N=6 per condition, results persisted):
```
cargo test --test agent_benchmark experiment_fix_go -- --ignored --nocapture --test-threads=1
```

> **Always use `--test-threads=1`.** Experiments share `~/.8v/events.ndjson`
> and will produce corrupted measurements if run in parallel.

> **Commit before running.** Results from a dirty tree show `(dirty, N files modified)`
> in the report and cannot be attributed to a specific commit. Run `git status` first.

> **Use a release binary for the MCP server.** Set `EIGHTV_BINARY=$(pwd)/target/release/8v`
> before running. The test runner itself is always a debug binary — that is expected.
> The MCP server should be release to avoid build-artifact skew in timing measurements.

## Results

Results are stored in `~/.8v/benchmark-results/` as NDJSON. Each experiment
writes a `report.md` and `report.json` to a named subdirectory.

## Scenarios

| File | Task | Fixture | Verification | Known variance sources |
|------|------|---------|--------------|------------------------|
| fix_rust.rs | Fix a failing Rust test | fix-test-rust | cargo test | Trivial fix — native can solve in 4 calls; 8v schema overhead may cost more |
| diagnose_rust.rs | Diagnose and fix Rust issues | diagnose-rust | cargo clippy | None |
| fix_python.rs | Fix failing Python tests | fix-test-python | pytest | Native landmines: pytest not in PATH; agent retries ≥5×. 8v side unaffected. |
| fix_go.rs | Fix failing Go tests | fix-go | go test | `// BUG:` comment — agents sometimes clean it up (extra write, ~19k more tokens) |
| fix_typescript.rs | Fix TypeScript type errors | fix-typescript | tsc --noEmit | High variance observed; stuck-loop landmines on both conditions |

## Fixture notes

**fix-go**: `lib.go` contains an explicit `// BUG: off-by-one` comment labeling the bug.
This creates two valid solution paths:
1. Agent replaces only the loop condition (5 MCP calls, ~103k tokens)
2. Agent fixes the loop AND deletes the BUG comment (6 MCP calls, ~122k tokens)

Both paths pass verification. The variance is real model non-determinism, not a detector
artifact. The comment is intentional — it tests whether agents use hints efficiently.

**fix-failing-test (Rust)**: At N=9 this task shows +1.9% cost for 8v (not publishable,
CV 24.2%). The task is too trivial — native solves it in 4 tool calls (~$0.06); 8v's
schema-loading overhead is not amortized. Good at demonstrating turn reduction (9.1→7.1)
but not cost savings. Use a more complex fixture for cost claims.

Agent Bash calls to `cargo test` during a session will fail if the agent does not `cd`
into the project directory first. The external verification in `pipeline.rs` runs from
the correct directory and is authoritative. High native variance traces to agents retrying
after in-session test failures that are CWD errors, not fix failures.

**fix-python-traversal**: Native condition has 100% landmine rate (6/6 runs). Root cause:
`pytest` is a dev dependency (`pip install -e ".[dev]"`) not installed globally. Agents
try `python -m pytest` which fails immediately; they retry 5+ times → landmine detector
fires. Despite landmines, native agents DO fix the bug (6/6 pass external verification).
The -35.9% cost delta and CI 8.5% are real, but 100% native landmine rate means this
result cannot be published as-is.

**Do not add "use `make test`" to the prompt.** Tested: pip install output destabilizes
agents (run 2 cost $0.34/619k tokens vs ~$0.12 baseline). CV exploded to 46.3%.
Native landmines went from 6/6 identical retries to 1 stuck loop + 1 error storm.
The Makefile is in the fixture for discoverability but should not be forced via prompt.

**To fix for publication**: install pytest globally on benchmark hosts (`pip install pytest`).
This is the only clean fix. Prompt engineering and Makefile both cause regressions.

## Interpreting results

- **Cost vs control**: negative means 8v is cheaper. A 15% CI half-width is the
  publishability threshold. N=6 is borderline — use N=9+ for publication.
- **Turns**: fewer turns = less back-and-forth. 8v typically halves turns.
- **Landmines**: genuine stuck loops (same tool + same args repeated 3× in a row, or
  5+ consecutive same non-MCP tool calls). MCP tool sequential calls are NOT landmines.
  A result with >50% landmine rate should not be published.
- **CV%**: coefficient of variation. >20% means high variance; interpret with caution.
- **N/A in verification columns**: gate not applicable to this task type (e.g. clippy
  is not checked for fix-go; build is not checked for diagnose tasks).
- **cache_read tokens dominate**: most input is served from the prompt cache.
  The `input` column (non-cached) is typically 7–13 tokens. This is expected behavior.
