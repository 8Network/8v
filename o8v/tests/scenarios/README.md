# Benchmark Scenarios

Each scenario defines one task that an AI agent must complete. Scenarios are
paired as (baseline, with-8v) to measure the effect of 8v on agent efficiency.

## How to run

Single scenario (one pass):
```
cargo test --test agent_benchmark fix_test_8v -- --ignored --nocapture
```

Full experiment (N=6 per condition, results persisted):
```
cargo test --test agent_benchmark experiment_fix_test -- --ignored --nocapture --test-threads=1
```

> **Always use `--test-threads=1`.** Experiments share `~/.8v/events.ndjson`
> and will produce corrupted measurements if run in parallel.

## Results

Results are stored in `~/.8v/benchmark-results/` as NDJSON. Each experiment
writes a `report.md` and `report.json` to a named subdirectory.

## Scenarios

| File | Task | Fixture | Verification |
|------|------|---------|--------------|
| fix_rust.rs | Fix a failing Rust test | fix-test-rust | cargo test |
| diagnose_rust.rs | Diagnose and fix Rust issues | diagnose-rust | cargo clippy |
| fix_python.rs | Fix failing Python tests | fix-test-python | pytest |
| fix_go.rs | Fix failing Go tests | fix-go | go test |
| fix_typescript.rs | Fix TypeScript type errors | fix-typescript | tsc --noEmit |

## Interpreting results

- **Cost vs control**: negative means 8v is cheaper. A 15% CI half-width is the
  publishability threshold. N=6 is often insufficient — use N=9+ for publication.
- **Turns**: fewer turns = less back-and-forth. 8v typically halves turns.
- **Landmines**: stuck loops where the agent retried the same failing command 3+
  times. These inflate token counts and variance. A result with >50% landmine rate
  should not be published.
- **CV%**: coefficient of variation. >20% means high variance; interpret with caution.
