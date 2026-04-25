# 8v

One command checks everything. Everything is an error.

## Install

```sh
curl -fsSL https://install.8vast.io | sh
```

Or build from source:

```sh
cargo build -p o8v
# binary at target/debug/8v
```

## Usage

```sh
8v check .              # check everything — 15 stacks, 25+ tools
8v fmt .                # format everything
8v read src/main.rs     # symbol extraction + line ranges for AI agents
8v write src/main.rs    # safe file editing (find/replace, insert, delete)
8v search "fn main"     # regex search, 3 output modes (compact/text/context)
8v ls .                 # file hierarchy with project labels and line counts
8v test .               # run project test runner
8v init                 # set up MCP, hooks, permissions
8v log                  # show recent 8v command history
8v stats                # token and call aggregates by command
8v upgrade              # upgrade to the latest release
```

## MCP Integration

8v exposes itself as a single MCP tool for AI agents.

| Tool set | Schema tokens |
|----------|--------------|
| 8v (1 MCP tool) | 641 |
| Native (Read + Edit + Write + Bash + Glob + Grep) | 1,377 |

One tool replaces six. 53% smaller schema.

Connect to Claude Code:

```sh
8v init    # configures MCP, CLAUDE.md, hooks, permissions
```

Or add manually to `.mcp.json`:

```json
{
  "mcpServers": {
    "8v": {
      "command": "8v",
      "args": ["mcp"]
    }
  }
}
```

## Status

Pre-release. Benchmark numbers withdrawn pending a foundation audit — the
binary they were measured against had ship-blocking bugs in commands those
benchmarks exercised. New numbers will be published only after the audit
lands and is reproducible end-to-end.

## Supported Stacks

Rust, TypeScript, JavaScript, Python, Go, .NET, Deno, Ruby, Java, Kotlin, Swift, Terraform, Dockerfile, Helm, Kustomize.

## Building

```sh
cargo test --workspace       # 2,455 tests
cargo clippy --workspace     # zero warnings
cargo build -p o8v           # CLI binary
8v check .                   # self-check
```

Workspace: 7 crates (o8v-fs, o8v-process, o8v-core, o8v-stacks, o8v-check, o8v-testkit, o8v).

## License

`o8v-fs` and `o8v-process` are licensed under MIT. All other crates are licensed under BSL-1.1, converting to Apache 2.0 on April 5, 2030.

---

Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
