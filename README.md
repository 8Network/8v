# 8v

Reduce AI agent token cost. One command checks everything. Everything is an error.

## Install

```sh
curl -fsSL https://releases.8vast.io/install.sh | sh
```

Or build from source:

```sh
cargo install --path o8v
```

## Usage

```sh
8v check .              # check everything — 15 stacks, 25+ tools
8v fmt .                # format everything
8v read src/main.rs     # symbol extraction + line ranges for AI agents
8v write src/main.rs    # safe file editing (find/replace, insert, delete)
8v search "fn main"     # regex search, 3 output modes (compact/text/context)
8v test .               # run project test runner
8v init                 # set up MCP, hooks, permissions
```

## MCP Integration

8v exposes itself as a single MCP tool for AI agents.

| Tool set | Schema tokens |
|----------|--------------|
| 8v (1 MCP tool) | 641 |
| Native (Read + Edit + Write + Bash + Glob + Grep) | 1,377 |

One tool replaces six. 53% smaller schema. 50.8% total token reduction on a fair two-arm benchmark.

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

## Token Savings (Proven)

| Claim | Result | Verification |
|-------|--------|-------------|
| Schema efficiency | 641 vs 1,377 tokens (53% smaller) | Schema measurement |
| Total token reduction | 50.8% (351K → 173K tokens) | Two-arm benchmark, both with MCP |
| Search efficiency | 86% savings in compact mode | `8v search` vs `grep -rn` |
| Tool call reduction | 55% fewer calls (11 → 5) | Same benchmark |

## Supported Stacks

Rust, TypeScript, JavaScript, Python, Go, .NET, Deno, Ruby, Java, Kotlin, Swift, Terraform, Dockerfile, Helm, Kustomize.

## Building

```sh
cargo test --workspace       # ~1,850 tests
cargo clippy --workspace     # zero warnings
cargo build -p o8v       # CLI binary
8v check .                   # self-check
```

Workspace: 10 crates (o8v-fs, o8v-process, o8v-project, o8v-core, o8v-stacks, o8v-check, o8v-events, o8v-testkit, o8v-workspace, o8v).

## License

`o8v-fs` and `o8v-process` are licensed under MIT. All other crates are licensed under BSL-1.1, converting to Apache 2.0 on April 5, 2030.

---

Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
