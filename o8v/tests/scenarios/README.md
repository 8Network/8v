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

**fix-python-traversal**: Native condition has 6/6 stuck-loop landmines consistently
across multiple runs. Root cause: native agents repeatedly run `python -m pytest` on
failing tests before fixing the code, cycling 5+ times. `pytest` IS installed globally;
the retries are genuine behavioral confusion, not missing-tool errors. Despite landmines,
native agents fix the bug in every run (6/6 external verification passes).

Overall landmine rate: 6/12 runs = 50%. The >50% disqualification rule does NOT apply.
The native-only landmines are part of the measured signal: 8v eliminates retry loops.
Published result (v3, N=6): **-32.9% cost, CI 11.2%**, 66% fewer turns (24.0→8.2).

**Do not add "use `make test`" to the prompt.** Tested: pip install output destabilized
agents ($0.34/619k tokens on one run, CV exploded to 46.3%). The Makefile is in the
fixture for discoverability but must not be forced via prompt.

## Interpreting results

- **Cost vs control**: negative means 8v is cheaper. A 15% CI half-width is the
  publishability threshold. N=6 is borderline — use N=9+ for publication.
- **Turns**: fewer turns = less back-and-forth. 8v typically halves turns.
- **Landmines**: genuine stuck loops (same tool + same args repeated 3× in a row, or
  5+ consecutive same non-MCP tool calls). MCP tool sequential calls are NOT landmines.
  The landmine rate counts across all runs of all conditions. A result with >50%
  overall landmine rate should not be published. If ONLY the native condition has
  landmines, that is part of the measured behavioral difference — 8v prevents stuck
  loops. Disqualification applies when both conditions are landmine-heavy (broken fixture).
- **CV%**: coefficient of variation. >20% means high variance; interpret with caution.
- **N/A in verification columns**: gate not applicable to this task type (e.g. clippy
  is not checked for fix-go; build is not checked for diagnose tasks).
- **cache_read tokens dominate**: most input is served from the prompt cache.
  The `input` column (non-cached) is typically 7–13 tokens. This is expected behavior.
