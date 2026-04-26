# 8v

You and your AI coding agent — same tool, same rules.

8v is one binary that reads, writes, searches, checks, builds, tests, and
formats code, using fewer input and output tokens than the agent's native tools.

Today, 8v supports Claude Code. More agents next.


## Install

```sh
curl -fsSL https://install.8vast.io | sh
```

Or build from source:

```sh
cargo build -p o8v --release
# binary at target/release/8v
```

## Setup

In any project:

```sh
8v init
```

In Claude Code, this disables Claude's native Read, Edit, Write, Grep,
and Glob in this project. From now on, file operations go through 8v.
Bash stays available for git, processes, and environment.

## Commands

```sh
8v ls                       # list files and projects in the tree
8v read src/main.rs         # symbol map (functions, types) — fast
8v read src/main.rs:40-80   # exact line range
8v read src/main.rs --full  # whole file
8v search "fn parse"        # regex across the codebase
8v write path:12 "..."      # replace a line
8v write path --insert      # insert, --delete, --append
8v write path --find a --replace b
8v check .                  # lint + type-check + format-check, every stack
8v fmt .                    # auto-format
8v build .                  # invoke the project's build tool
8v test .                   # invoke the project's test runner
8v log                      # what ran in this session
8v stats                    # what failed most often
8v upgrade                  # update 8v itself
```

Every command accepts `--json` for structured output.

## Stacks

Rust, TypeScript, JavaScript, Python, Go, Deno, .NET, Ruby, Java, Kotlin,
Swift, Terraform, Dockerfile, Helm, Kustomize, Erlang. Shell files always
go through shellcheck and shfmt.

## Benchmark

Each scenario gives Claude a broken codebase and asks it to fix it.
Native tools vs. 8v, one variable at a time.

| Scenario | Input tokens | Output tokens |
|---|---|---|
| fix-failing-test (Rust) | −14% | −42% |
| fix-go | −21% | −52% |
| fix-python | −66% | −66% |
| fix-typescript | −12% | −39% |

N=6 per condition. Tests pass 6/6 in every scenario, both arms.

```sh
cargo test -p o8v --test demo_agent_benchmark -- --ignored --nocapture --test-threads=1
```

## Workspace

```sh
cargo build -p o8v --release   # binary
cargo test --workspace         # full suite
8v check .                     # 8v on itself
```

Crates: `o8v` (binary), `o8v-core` (commands and rendering), `o8v-fs`
(safe filesystem), `o8v-process` (safe subprocess), `o8v-stacks` (stack
detection and dispatch), `o8v-check` (lint/type/format orchestration),
`o8v-testkit` (test infrastructure).

## License

`o8v-fs` and `o8v-process` are MIT.
All other crates are BSL-1.1, converting to Apache 2.0 on April 5, 2030.

---

Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
