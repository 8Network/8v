# Changelog

All notable changes to 8v will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.1.7] - 2026-04-25

## [0.1.6] - 2026-04-25

## [0.1.5] - 2026-04-25

## [0.1.4] - 2026-04-25

## [0.1.3] - 2026-04-25

## [0.1.2] - 2026-04-24

## [0.1.1] - 2026-04-24

### Added

- `8v search` — regex content search with 3 agent-optimized output modes (compact/text/context), 86% token savings vs grep
- `8v init` — sets up MCP, CLAUDE.md, AGENTS.md, hooks, permissions, and .8v/config.toml
- `8v hooks` — full hook system: git hooks (pre-commit, commit-msg) and Claude Code hooks (8 events)
- MCP server — exposes `8v` as a single tool for AI agents (641 tokens vs 1,377 for native tools)
- Event sourcing — `.8v/events/`, `series.json`, diagnostic tracking across runs
- MCP cost observability — `McpInvoked`/`McpCompleted` events in `.8v/mcp-events.ndjson`
- Test infrastructure — production-grade: typed events, fixtures on disk, zero raw std::fs, zero serde_json::Value
- Agent benchmark — two-arm fair comparison measuring token efficiency (50.8% token reduction proven)
- Symbol extraction in `8v read` — extract functions, structs, classes from source files
- Security hardening — containment boundaries, binary file handling, CRLF support

## [0.1.0] - 2026-04-06

### Added

- `8v check .` — correctness checking across 15 language stacks
- `8v fmt .` — code formatting across 9 language stacks
- `8v fmt . --check` — verify formatting without changes (CI mode)
- 15 language stacks: Rust, TypeScript, JavaScript, Python, Go, Deno, .NET, Ruby, Java, Kotlin, Swift, Terraform, Dockerfile, Helm, Kustomize
- 25+ tools integrated:
  - Rust: cargo check, cargo clippy, cargo fmt
  - TypeScript/JavaScript: tsc, eslint, prettier, biome, oxlint
  - Python: ruff, mypy
  - Go: go vet, staticcheck, gofmt
  - Deno: deno check, deno fmt
  - .NET: dotnet build, dotnet format
  - Ruby: rubocop
  - Java: javac
  - Kotlin: ktlint
  - Swift: swiftlint
  - Terraform: tflint
  - Docker: hadolint
  - Helm: helm lint
  - Kustomize: kustomize build
- Output formats:
  - Human (colored, default) — readable output with per-tool timing
  - Plain — unformatted for AI agents and tooling
  - JSON — structured output for programmatic access
- Command-line flags:
  - `--json` — JSON output format
  - `--plain` — plain text output (no colors, no formatting)
  - `--verbose` — verbose output including tool stderr
  - `--no-color` — disable colored output
  - `--timeout <seconds>` — per-tool timeout (default: 300s / 5 minutes)
  - `--limit <count>` — limit errors shown per tool
- Per-tool execution timing in human output
- Signal handling:
  - Graceful shutdown on first Ctrl+C (SIGINT)
  - Force exit on second Ctrl+C
- Exit codes:
  - 0 — all checks passed
  - 1 — one or more checks failed
  - 2 — nothing to check (no supported files detected)
  - 130 — interrupted by signal
- StackTools architecture: each language stack defines checks, formatter, and optional test runner
- 577+ tests across all 7 crates (o8v-fs, o8v-process, o8v-core, o8v-stacks, o8v-check, o8v-testkit, o8v)
- BSL-1.1 license
- Multi-platform support:
  - macOS (arm64, x86_64) — code-signed and notarized
  - Linux (x86_64, arm64)
  - Checksums for all binaries
