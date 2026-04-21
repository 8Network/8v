# QA Findings: init · hooks · upgrade · mcp
Date: 2026-04-20
Binary: `/Users/soheilalizadeh/8/products/vast/oss/8v/target/debug/8v` (v0.1.0)
Scope: pure observation — no source files modified, no commits created

---

## Per-Command Result Tables

### `8v init`

| Form | Exit | Notes | Verdict |
|------|------|-------|---------|
| `8v init --yes` (no git, no stack) | 0 | Creates `.8v/config.toml`, `.mcp.json`, `.claude/settings.json`, `CLAUDE.md`, `AGENTS.md`, `.aider.conf.yml`. Outputs "Pre-commit hook installed" + "Commit-msg hook installed" but no files are written under `.git/` (none exists). | FAIL (false positive) |
| `8v init --yes` (git repo, rust stack) | 0 | Detects `Cargo.toml` → rust. Writes hooks to `.git/hooks/pre-commit` and `.git/hooks/commit-msg`. All files verified present. | PASS |
| `8v init --yes` (second run, idempotent) | 0 | Reports "Skipped CLAUDE.md (AGENTS.md already has current 8v block)". Parenthetical names the wrong file — should reference CLAUDE.md's own block. | FAIL (wrong skip message) |
| `8v init --json` | 0 | JSON on stdout, human progress text on stderr. Correct channel split. | PASS |
| `8v init --help` | 0 | Help renders cleanly. | PASS |

### `8v hooks`

| Form | Exit | Notes | Verdict |
|------|------|-------|---------|
| `8v hooks git on-commit` (clean repo) | 0 | Exits 0, no output. Correct. | PASS |
| `8v hooks git on-commit-msg <file>` (message with Co-Authored-By) | 0 | Strips Co-Authored-By lines from commit message file in place. | PASS |
| `8v hooks claude pre-tool-use` (valid JSON payload via stdin) | 0/1 | Correctly allows `mcp__8v__*` and blocks native tools (Read, Edit, Write, Glob, Grep). Exit reflects block/allow decision. | PASS |
| `echo "" \| 8v hooks claude pre-tool-use` (empty/malformed input) | 0 | Emits parse error on stderr but outputs "hooks: passed" and exits 0. Agent receives false allow signal. | FAIL (false allow on parse error) |
| `8v hooks claude pre-tool-use --json` | 2 | `error: unexpected argument '--json' found`. Standard flags not propagated to leaf subcommands. | FAIL (missing flag) |
| `8v hooks --help` | 0 | Help renders. | PASS |

### `8v upgrade`

| Form | Exit | Notes | Verdict |
|------|------|-------|---------|
| `8v upgrade --json` (already current) | 0 | Returns `{"current_version":"0.1.0","error":null,"latest_version":"0.1.0","upgraded":true}`. Field `upgraded:true` when nothing was downloaded. Semantically incorrect. | FAIL (misleading field) |
| `8v upgrade --plain` (already current) | 0 | Outputs `current: 0.1.0 / latest: 0.1.0 / status: up-to-date`. Human-readable text is accurate; JSON field is not. | PASS (plain) / FAIL (JSON field) |
| `8v upgrade --json` (network failure) | 0 | `error` field populated with failure message, but exit code is 0. Callers cannot detect failure via exit code alone. | FAIL (silent failure) |
| `8v upgrade --help` | 0 | Help renders. No `--check` / `--dry-run` flag documented or available. | PASS (help) / GAP (no dry-run) |

### `8v mcp`

| Form | Exit | Notes | Verdict |
|------|------|-------|---------|
| `8v mcp < /dev/null` (immediate EOF) | 1 | stderr: `error: MCP server failed: connection closed: initialize request`. Exits cleanly, no zombie process. | PASS |
| Full handshake: `initialize` + `notifications/initialized` + `tools/list` | 0 (running) | Responds with `{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"rmcp","version":"1.3.0"}}`. Single "8v" tool in `tools/list`. | PASS |
| SIGTERM during active session | 143 | Exits with signal exit code 143. No zombie. | PASS |
| Malformed JSON input | 1 | Parse error on stderr, exits 1. | PASS |
| `8v mcp --json` | 2 | `error: unexpected argument '--json' found`. | FAIL (missing flag) |
| `8v mcp --plain` | 2 | `error: unexpected argument '--plain' found`. | FAIL (missing flag) |
| `8v mcp --help` | 0 | Only `-h/--help` shown. No other flags. | PASS (help) / GAP (no flags) |

---

## Top 3 Issues Per Command

### `8v init` — Top 3

**I-1 (HIGH): False-positive hook install in non-git directories.**
`8v init --yes` in a directory without `.git/` outputs "Pre-commit hook installed" and "Commit-msg hook installed" and lists `.git/hooks/pre-commit` under "Files modified". No files are actually written. The message lies to the user (and to any agent parsing the output). An agent using this output to verify setup would conclude hooks are active when they are not.

**I-2 (MEDIUM): Skip message references the wrong file.**
On re-init, the output reads: "Skipped CLAUDE.md (AGENTS.md already has current 8v block)". The parenthetical should explain why CLAUDE.md was skipped — i.e., that CLAUDE.md itself already has the block. Referencing AGENTS.md is incorrect and confusing.

**I-3 (LOW): No validation that the binary on PATH is `8v`.**
The installed hook scripts call `8v hooks git on-commit` bare — no path anchoring. If `8v` is not on PATH at hook execution time (e.g., in CI with a different environment), the hook silently fails with "command not found", and the commit proceeds unchecked. Init does not warn about this.

---

### `8v hooks` — Top 3

**H-1 (HIGH): Parse error on stdin exits 0 with false "passed" signal.**
`echo "" | 8v hooks claude pre-tool-use` emits a parse error on stderr but exits 0 and outputs "hooks: passed". In Claude Code's pre-tool-use hook protocol, exit 0 means allow. An agent presenting a malformed or empty hook payload gets an unconditional allow. This is a security-relevant false negative.

**H-2 (MEDIUM): `--json` flag rejected by leaf subcommands.**
`8v hooks claude pre-tool-use --json` → exit 2, "unexpected argument '--json' found". All other 8v subcommands accept `--json`. An agent scripting hook invocations cannot get structured output from this path.

**H-3 (LOW): No test for non-Co-Authored-By attribution patterns.**
`8v hooks git on-commit-msg` strips `Co-Authored-By:` lines. Other AI attribution patterns (e.g., `Generated-by:`, `Signed-off-by: Claude`) are not stripped. The scope of the stripping is narrower than the intent (remove AI attribution).

---

### `8v upgrade` — Top 3

**U-1 (HIGH): `upgraded: true` when nothing was upgraded.**
`8v upgrade --json` returns `upgraded: true` whenever the command completes without error, even when `current_version == latest_version` and no download occurred. Callers cannot distinguish "binary was replaced" from "already current". The field should be `false` when no replacement happened, or renamed to `is_current` / `was_upgraded`.

**U-2 (HIGH): Network failure exits 0.**
When the upgrade server is unreachable or returns an error, `upgrade --json` populates the `error` field but exits 0. Callers checking `$?` cannot detect the failure. Non-zero exit is the UNIX contract for command failure; violating it breaks pipelines and CI scripts.

**U-3 (MEDIUM): No `--check` / `--dry-run` mode.**
There is no way to check whether a new version is available without actually performing the upgrade. Every invocation hits the network and potentially replaces the binary. Agents or scripts that want to surface "update available" information must fully commit to a replacement. This combines a read operation (version check) with a destructive write (binary replacement) into a single irreversible call.

---

### `8v mcp` — Top 3

**M-1 (MEDIUM): Standard output flags (`--json`, `--plain`, `--no-color`, `--human`) rejected.**
All other 8v subcommands propagate these flags. `8v mcp` is a long-running server, not a query command, so JSON output of a "result" does not apply the same way — but the missing flag surface creates an inconsistent experience. At minimum, `--help` should document why these flags are absent.

**M-2 (LOW): `serverInfo.name` is "rmcp", not "8v".**
The MCP initialize response returns `{"serverInfo":{"name":"rmcp","version":"1.3.0"}}`. `rmcp` is the underlying framework library name, not the product name. Clients that display server identity (e.g., Claude Code's MCP manager) will show "rmcp" instead of "8v". This is a branding/identity leak.

**M-3 (LOW): No startup readiness signal.**
The MCP server silently waits for input after starting. There is no "ready" line on stderr, no PID file, and no way for a parent process to know when the server is accepting connections. The only feedback is the MCP handshake response itself. Init scripts that launch `8v mcp` in the background have no reliable way to wait for readiness.

---

## Cross-Command Inconsistencies

**X-1: `--json` flag propagation is inconsistent.**
`8v init --json`, `8v upgrade --json` work. `8v hooks claude pre-tool-use --json` and `8v mcp --json` reject the flag with exit 2. The flag is documented as universal ("every command accepts `--json`") but is not implemented uniformly.

**X-2: Exit code semantics for errors are inconsistent.**
`8v upgrade` exits 0 on network failure (with `error` field populated). `8v hooks claude pre-tool-use` exits 0 on parse error (with "passed" output). `8v mcp < /dev/null` correctly exits 1 on connection failure. There is no consistent rule: "if the command cannot complete its job, exit non-zero."

**X-3: Boolean field semantics in JSON output.**
`upgrade --json` uses `upgraded: true` to mean "command ran without error" (not "binary was replaced"). Other commands that return JSON do not have equivalent misleading boolean fields today, but the pattern is a footgun for future fields. The convention should be: boolean fields name exactly what they test.

**X-4: Hook install feedback does not distinguish "installed" from "already present".**
`8v init` in a git repo with existing hooks outputs "Pre-commit hook installed" for both new installs and no-op cases (hook already existed with the 8v invocation). There is no "hook already present, skipped" path in the output. Compare to how CLAUDE.md/AGENTS.md idempotency is handled: those report "Skipped X (already has current block)".

---

## Dogfood Check: Can an Agent Set Up From Scratch?

Scenario: new developer clones repo, runs `8v init --yes`, expects a fully configured environment.

**Step 1 — Git repo with Cargo.toml:** Works. Stack detected, hooks installed, all config files written. Agent can confirm via file presence.

**Step 2 — Non-git directory:** Broken. Agent reads "Pre-commit hook installed" in output and concludes hooks are active. They are not. An agent that subsequently runs `git commit` (after `git init`) will not have the hooks — but nothing told it to re-run `8v init`.

**Step 3 — Re-run after partial setup:** Works, but skip message is confusing. "Skipped CLAUDE.md (AGENTS.md already has current 8v block)" is misleading. An agent that parses this might conclude CLAUDE.md was intentionally omitted rather than already configured.

**Step 4 — MCP verification:** Agent cannot use `8v mcp --json` to programmatically confirm the server starts. It must run the full MCP handshake to verify. This is possible but undocumented.

**Overall:** An agent can set up a git-repo project from scratch. The non-git case is broken with misleading feedback. The re-run messaging has a correctness bug. Confidence for production agent use: **Medium** (git repos only).

---

## Proposed Next-Slice Candidates

Listed by severity and actionability:

1. **Fix: `8v init` must suppress hook install messages when no `.git/` exists.** Either skip hook installation silently, or output "No git repository found — skipping hook install." Do not report success for an operation that did not occur. (Fixes I-1, partially X-4.)

2. **Fix: `8v hooks claude pre-tool-use` must exit non-zero on parse error.** A malformed or empty stdin payload must not produce a "passed" result. The safe fallback for an unreadable payload should be to block (exit 1 or 2), not allow. (Fixes H-1.)

3. **Fix: `upgrade --json` field `upgraded` must reflect whether the binary was replaced.** When `current_version == latest_version`, `upgraded` must be `false`. (Fixes U-1.)

4. **Fix: `upgrade` must exit non-zero on network failure.** If the upgrade cannot determine the latest version or download the binary, exit 1. (Fixes U-2.)

5. **Fix: Skip message in `8v init` re-run must name the correct file.** "Skipped CLAUDE.md (already has current 8v block)" — not AGENTS.md. (Fixes I-2.)

6. **Feature: `--json` propagation to `8v hooks` leaf subcommands.** Follow the same pattern as other subcommands. (Fixes H-2, X-1.)

7. **Feature: `8v upgrade --check` dry-run flag.** Checks current vs latest, reports result, does not download or replace binary. (Addresses U-3.)

8. **Fix: `8v mcp` serverInfo.name should be "8v", not "rmcp".** One-line fix in the MCP server initialization. (Fixes M-2.)

---

## 8v Feedback (Dogfood Observations)

Recorded for cross-check against `~/.8v/events.ndjson`.

**F-1:** `8v hooks claude pre-tool-use --json` fails with exit 2. Expected `--json` to work on all leaf subcommands per CLAUDE.md docs. Friction: cannot script hook calls with structured output.

**F-2:** `8v mcp --json` fails with exit 2. No structured output mode available for the MCP server. Fine for interactive use; problematic for scripted health checks.

**F-3:** `8v upgrade --json` returned `upgraded: true` when already current. Parsed the JSON assuming `upgraded` means "binary was replaced". Required reading the `current_version`/`latest_version` fields to infer the real state. Field name is a trap.

**F-4:** `8v init` in a non-git directory reported hook installation. Verified with `find` that no files were created. The feedback loop is broken: output says one thing, filesystem says another. This is the kind of discrepancy that causes agent retry loops.

**F-5:** No `8v upgrade --check` means there is no safe way to surface "a new version is available" without triggering a replacement. Would use this in a CI step to gate on version currency.

---

*Observation only. No source files were modified. No commits were created. Binary under test left unmodified.*
