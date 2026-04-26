# Benchmarks

Each scenario gives Claude a broken codebase and asks it to fix it. We
run the same task twice — once with Claude's native tools, once with 8v
— and compare input tokens, output tokens, tool calls, and turns. Tests
must pass in both arms.

## Results

| Scenario | Input tokens | Output tokens |
|---|---|---|
| fix-failing-test (Rust) | −14% | −42% |
| fix-go | −21% | −52% |
| fix-python | −66% | −66% |
| fix-typescript | −12% | −39% |

N=6 per condition. Tests pass 6/6 in every scenario, both arms.

## Run

```sh
cargo test -p o8v --test demo_agent_benchmark -- --ignored --nocapture --test-threads=1
```

Requires a working `claude` CLI authenticated locally and the `8v`
binary on `PATH`. Each scenario takes a few minutes; the full suite
takes about an hour.

## Scenarios

Defined in `o8v/tests/demo_agent_benchmark.rs`. Each pair of tests
(`*_baseline` and `*_8v`) shares one fixture: same broken code, same
task prompt, only the tool surface changes.

- `fix_test_baseline` / `fix_test_8v` — Rust, failing unit test
- `fix_python_baseline` / `fix_python_8v` — Python, broken module
- `fix_go_baseline` / `fix_go_8v` — Go, failing test
- `fix_typescript_baseline` / `fix_typescript_8v` — TypeScript, type error
- `diagnose_baseline` / `diagnose_8v` — read-only investigation

## Output

Per-run JSON lands in `~/.8v/bench/`. Aggregated CSVs are written next
to the test run.
