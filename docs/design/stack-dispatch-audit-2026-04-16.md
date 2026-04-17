# Stack Dispatch Audit — `8v test` and `8v build` across all stacks

- **Status:** Blockers P0 (TS/JS, Kotlin, Ruby, Java) RESOLVED 2026-04-16 via `o8v-stacks/src/resolve_tool.rs` (27 tests). Remaining P1/P2 concerns still open for review.
- **Date:** 2026-04-16.
- **Author:** stack-dispatch audit.

## Goal

Stable, predictable `8v test` / `8v build` dispatch across every supported stack. No cryptic error modes. When an ecosystem has a canonical test/build runner, 8v drives it directly rather than depending on a user having wired up a script alias (e.g. `npm test`). Every failure must be visible and actionable per CLAUDE.md ("no silent fallbacks", "every error must be visible", "error messages must say what's wrong AND what the user should do").

## Scope

16 stack modules under `oss/8v/o8v-stacks/src/stacks/`:
deno, dockerfile, dotnet, erlang, go, helm, java, javascript, kotlin, kustomize, python, ruby, rust, shell, swift, terraform, typescript. (`node.rs` is a helper, not a stack.)

## Current dispatch table

| Stack       | `8v test` program + args                 | `8v build` program + args                    | Manifest assumption                       | Failure mode when assumption breaks                              | Severity  |
|-------------|------------------------------------------|----------------------------------------------|-------------------------------------------|-------------------------------------------------------------------|-----------|
| rust        | `cargo test --workspace`                 | `cargo build`                                | `Cargo.toml` exists                       | Graceful: "0 tests"; cargo errors are clear.                      | ok        |
| go          | `go test ./...`                          | `go build ./...`                             | `go.mod` exists                           | Graceful: "[no test files]" per package; clear compile errors.    | ok        |
| python      | `python3 -m pytest -q`                   | (none)                                       | `pytest` installed                        | If pytest missing → `No module named pytest`. Clear enough.       | concern   |
| typescript  | `npm test --silent`                      | `tsc`                                        | `scripts.test` defined in package.json; top-level `tsconfig.json` | **Cryptic: `npm ERR! missing script: test`.** `tsc` with no tsconfig → confusing. | **blocker** |
| javascript  | `npm test --silent`                      | `npm run build --silent`                     | `scripts.test`, `scripts.build` both defined | **Cryptic: `npm ERR! missing script: test` / `missing script: build`.** | **blocker** |
| deno        | `deno test`                              | `deno compile`                               | A file/entry to compile                   | `deno compile` with no entrypoint → cryptic; `deno test` is fine. | **blocker** (build) |
| dotnet      | `dotnet test`                            | `dotnet build` (with target discovery)       | `.sln` / `.slnx` / `.csproj` present      | `dotnet test` with no test project → cryptic MSB error.           | concern   |
| erlang      | `rebar3 eunit`                           | `rebar3 compile`                             | `rebar.config` valid                      | rebar3 errors are moderately clear.                               | ok        |
| java        | `mvn test`                               | `mvn package -q`                             | `pom.xml` valid                           | Maven prints a huge banner then an error — not great, but visible.| concern   |
| kotlin      | `gradle test`                            | `gradle build`                               | `build.gradle(.kts)` valid; `gradle` on PATH (not `./gradlew`) | If only wrapper exists, `gradle` missing → cryptic.              | **blocker** |
| ruby        | `rake test`                              | (none)                                       | `Rakefile` with a `test` task             | `rake aborted! Don't know how to build task 'test'` — cryptic.    | **blocker** |
| shell       | (none)                                   | (none)                                       | —                                         | n/a                                                               | ok        |
| swift       | `swift test`                             | `swift build`                                | `Package.swift` present                   | SwiftPM error clear.                                              | ok        |
| dockerfile  | (none)                                   | (none)                                       | —                                         | n/a                                                               | ok        |
| helm        | (none)                                   | (none)                                       | —                                         | n/a                                                               | ok        |
| kustomize   | (none)                                   | (none)                                       | —                                         | n/a                                                               | ok        |
| terraform   | (none)                                   | (none)                                       | —                                         | n/a                                                               | ok        |

## Failure-mode analysis

### Fails gracefully
- **rust** (`cargo test --workspace`): "running 0 tests" if no tests; errors are structured.
- **go** (`go test ./...`): prints `[no test files]` for empty packages.
- **swift, erlang**: native runners emit reasonably clear messages.
- **shell**: correctly treats "no .sh files" as pass (see `ShellCheckCheck::run`).

### Fails cryptically (blockers)
1. **typescript / javascript — `npm test --silent`, `npm run build --silent`**: if the user has no `scripts.test` / `scripts.build`, output is `npm ERR! missing script: test` with no guidance. The agent then thrashes because `--silent` also suppresses surrounding context. This was the originally reported motivation.
2. **kotlin — `gradle test` / `gradle build`**: many Kotlin projects ship only `./gradlew`, not a system `gradle`. Invoking `gradle` yields a PATH-not-found, not "use the wrapper" guidance.
3. **ruby — `rake test`**: projects without a `test` task in `Rakefile` get `Don't know how to build task 'test'`. No discovery of `rspec`, `minitest`, or `bundle exec rake test`.
4. **deno — `deno compile`**: requires an entrypoint; running it in a library project fails cryptically with `error: Expected a file argument`.

### Runs the wrong thing / incomplete coverage
- **typescript build = `tsc`** ignores `tsconfig.json` locations and project-references; no `--project` discovery. A monorepo gets "no inputs were found in config file".
- **python** has no `build_tool` (fine — Python has no universal build), but `pytest` is hardcoded even if the project uses `unittest`, `nose2`, or `uv run pytest`.
- **dotnet test** does not pass a target file (unlike `dotnet build`, which does). Multi-project repos hit MSB1011 the same way `dotnet build` used to.
- **java** assumes Maven; Gradle-Java projects are misrouted (handled by a different detector, but worth verifying the dispatch path).
- **kotlin** hardcodes `gradle` even when wrapper is present.

## Proposal

Per-stack recommendations, ordered by severity.

### P0 — blockers

**typescript / javascript test**
Before running `npm test --silent`, inspect `package.json`:
1. If `scripts.test` is defined → run `npm test --silent`.
2. Else, detect a canonical runner from `devDependencies` / local `node_modules/.bin` in priority order: `vitest` → `jest` → `mocha` → Node 20+ `node --test`.
3. Else return a structured error: `no test runner configured — add a "test" script to package.json, or install vitest/jest`.

**typescript / javascript build**
1. If `scripts.build` defined → `npm run build --silent`.
2. Else for TypeScript: if a `tsconfig.json` exists → `tsc --noEmit` (no build — tell the user explicitly that 8v is type-checking because no build script is configured).
3. Else return: `no build configured — add a "build" script or a tsconfig.json`.

**kotlin test/build**
Prefer `./gradlew` when present in the project root; fall back to `gradle` on PATH; else structured error `neither gradlew nor gradle found — install Gradle or run "gradle wrapper"`.

**ruby test**
Detect in order: `spec/` dir + `rspec` gem → `bundle exec rspec`; `test/` dir + `minitest` → `bundle exec rake test`; `Rakefile` with a `test` task (parse or probe via `rake -T`) → `bundle exec rake test`; else structured error.

**deno build**
Only enable `8v build` when a main module is discoverable (e.g. `deno.json` `tasks.build`, or a `main.ts` / `mod.ts`). Otherwise return `no build entrypoint — add a "build" task to deno.json or pass an entrypoint`.

### P1 — concerns

- **dotnet test**: apply the same target-discovery used by `DotnetCheck` to `dotnet test` (pass the discovered `.slnx`/`.sln`/`.csproj`).
- **python test**: detect `pytest` vs `unittest` (`tests/` layout, `pyproject.toml [tool.pytest]`); if neither, structured error rather than blindly trying `pytest`.
- **java test/build**: confirm Gradle-Java routing. If pom.xml absent but build.gradle present, the dispatch should pick Gradle.

### P2 — polish

- Drop `--silent` on npm when the command is about to fail — `--silent` hides the error trailer that would otherwise be helpful. Use `--silent` only on success paths, or strip it and let `o8v-render` format.
- Every structured error returned from a stack dispatcher must follow the template: `<what failed> — <what to do>`.

## Principles

1. Never run a command that returns a cryptic error when a clear one is possible.
2. When the ecosystem has a canonical test/build runner, invoke it directly. Don't depend on users having wired up `npm test` or a Rake task.
3. Error messages say **what** is wrong and **what** to do.
4. No silent fallback to "maybe it'll work" — if we cannot decide what to run, we return a diagnostic, not an attempt.
5. Detection lives in `o8v-project` / a new `o8v-stacks::dispatch` module; dispatch code remains a thin lookup.

## Non-goals

- Adding brand-new stacks (Zig, Nim, Elixir, …). Out of scope.
- Changing the public `8v test` / `8v build` CLI surface.
- Changing any `check` (lint/type) behavior; this audit only covers test + build.
- Auto-installing missing runners. We detect and report; we don't mutate user environments.

## Open questions

1. Should the "runner detection" be computed once during project detection (cached on the `Project` struct) or at dispatch time? Caching is faster but duplicates information on disk.
2. For monorepos with both `scripts.test` and a detected `vitest`, which wins? Proposal: the script wins (user intent); fall-back is only when no script is present.
3. Should `8v test` exit 0 or exit with a specific non-zero when "no test runner configured" (distinct from "tests failed")? Proposal: distinct exit code `2` for "nothing to run", matching `8v check` conventions.
4. Where does detection logic live — `o8v-project` (which answers "what is this?") or `o8v-stacks` (which answers "how do I run it?"). Proposal: dispatch-time runner selection lives in `o8v-stacks` behind a `resolve_test_runner(project: &Project) -> Result<TestTool, DispatchError>` function. Keeps `o8v-project` pure detection.
5. `--silent` on npm: drop unconditionally, keep on success only, or make it a verbosity-level concern in `o8v-render`?

## Migration

1. Land detection helpers in `o8v-stacks` with tests (fixture-based): package.json variants, Gradle wrapper presence, Rakefile `test` task presence, deno.json tasks.
2. Convert `StackTools.test_runner` / `build_tool` from static `TestTool { program, args }` to a `TestRunner` trait implemented per stack. Rust/Go/Swift implementations are one-liners (return the current static tool).
3. Update the `8v test` / `8v build` command handlers to surface `DispatchError` as structured render output.
4. Add fixtures for each blocker scenario under `o8v-testkit/fixtures/dispatch/`:
   - `ts-no-test-script/` — package.json without `scripts.test`, expect "no test runner configured".
   - `ts-vitest/` — devDependency only, expect vitest dispatch.
   - `kotlin-wrapper-only/` — `./gradlew` present, no `gradle` on PATH, expect wrapper dispatch.
   - `ruby-rspec/` — expect rspec dispatch.
   - `deno-library/` — no main, expect structured "no build entrypoint".
5. Do not ship partial: all P0 blockers land together or none do.
