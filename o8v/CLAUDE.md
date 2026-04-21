<!-- 8v:begin v0.1.0 -->

# 8v

Use `8v` instead of Read, Edit, Write, Grep, Glob, and Bash for file operations. Use Bash only for git, process management, and environment operations. For anything that reads, edits, searches, or inspects files, use 8v — not Bash. If an `8v` MCP tool is available (search tools for `8v`), call it directly — do not shell out via Bash.

## Two principles, every command
1. **Progressive.** Default output is the minimum useful answer. Flags escalate detail (`8v read <path>` returns a symbol map; add `:start-end` for a range, `--full` for everything).
2. **Batch.** Pass many inputs in one call (`8v read a.rs b.rs c.rs`) instead of calling N times. Each call costs overhead — amortize it.

Every command accepts `--json`. Run `8v <cmd> --help` for the full flag list.

## Discovery
- `8v ls --tree --loc` — full hierarchy with line counts. Start here.
- `8v ls [--match <glob>] [--stack <name>] [path]` — filtered views. Valid `--stack` values: `rust`, `javascript`, `typescript`, `python`, `go`, `deno`, `dotnet`, `ruby`, `java`, `kotlin`, `swift`, `terraform`, `dockerfile`, `helm`, `kustomize`, `erlang`.
- `8v search <pattern> (regex) [path] [-i] [-e <ext>] [-C N] [--files] [--limit N]`
  Default output groups matches by file: `<path>:<line>:<text>`. `--files` lists only paths. `-C N` adds N context lines around each match.

## Read — symbol map first, range second, full last
- `8v read <path>` — symbol map. Each line: `<line-number>  <symbol>`. Example output:
    `12  fn main`
    `36  pub struct Args`
    `58  impl Args`
  The line numbers point at the definition; use them with `:start-end` to read the body.
- `8v read <path>:<start>-<end>` — line range (1-indexed, end inclusive). Use after the symbol map.
- `8v read <path> --full` — entire file. Last resort.
- `8v read a.rs b.rs Cargo.toml` — batch any combination of paths and ranges in one call: distinct files, multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`), or a mix. One call beats N sequential calls.
- Batch output contract: each file is preceded by `=== <label> ===` on its own line. Label is the relative path, or `<path>:<start>-<end>` for ranges. Single-file reads emit no header. `--full` uses the same `===` delimiter. `--json` replaces the text stream: single-file → `{"Symbols":{...}}`; batch → `{"Multi":{"entries":[{"label":"<path>","result":{...}},...]}}`
- `8v read a.rs b.rs c.rs --full` — full content of multiple files in one call. `--full` applies to every positional arg (repeats accepted, no-op beyond the first).

## Write
- `8v write <path>:<line> "<content>"` — replace a single line.
- `8v write <path>:<start>-<end> "<content>"` — replace a range.
- `8v write <path>:<start>-<end> --delete` — delete a range.
- `8v write <path>:<line> --insert "<content>"` — insert before a line.
- `8v write <path> --find "<old>" --replace "<new>"` — fails if `<old>` not found.
- `8v write <path> --append "<content>"`
  Content arguments are parsed by 8v (not the shell): `\n` becomes a newline, `\t` a tab, `\\` a literal backslash. Pass them as literal two-character sequences — do not rely on shell interpolation.

## Verify
- `8v check .` — lint + type-check + format-check. Non-zero exit on any issue.
- `8v fmt .` — auto-format files in place. Idempotent.
- `8v test .` — run project tests.
- `8v build .` — compile.
All verify commands accept `--json` and run on the whole project by default. Pass a path to scope to a subtree.

Typical flow: `8v ls --tree --loc` → `8v read` symbol maps (batch) → ranges → `8v write` → `8v test .` → `8v check .`.

<!-- 8v:end -->
