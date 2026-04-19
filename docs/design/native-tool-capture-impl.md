# Native Tool Capture — Level 2 Implementation Design

**Status:** Draft — pending founder review.
**Depends on:** `native-tool-capture.md` (Level 1), `o8v-core/src/caller.rs`, `o8v-core/src/events/lifecycle.rs`, `o8v-core/src/types/session_id.rs`, `o8v/src/hooks/claude.rs`.
**Freeze note:** `8v hook` is approved as hidden infrastructure (Level 1 §8, Decision 1).

---

## 1. Module Layout

```
o8v-core/src/caller.rs          — add Caller::Hook variant (serializes "hook")
o8v-core/src/types/session_id.rs — add from_claude_session_id() constructor
o8v/src/hook/                   — NEW module (singular, separate from hooks/)
o8v/src/hook/mod.rs             — Args, HookCommand enum, run()
o8v/src/hook/pre.rs             — PreToolUse handler (event emitter, always exit 0)
o8v/src/hook/post.rs            — PostToolUse handler (event emitter, always exit 0)
o8v/src/hook/redact.rs          — Bash command redaction pipeline
o8v/src/hook/store.rs           — temp file I/O for run_id correlation
o8v/src/commands/mod.rs         — add Hook(crate::hook::Args) variant (hide = true)
```

The existing `o8v/src/hooks/` (plural) is the hook-management command. The new `o8v/src/hook/` (singular) is the hook-execution command. These are distinct.

---

## 2. Subcommand Surface

```
8v hook pre   [hidden]  — read PreToolUse JSON from stdin, emit CommandStarted
8v hook post  [hidden]  — read PostToolUse JSON from stdin, emit CommandCompleted
```

Both subcommands: `#[clap(hide = true)]`. Neither appears in `--help`. Neither accepts flags. Both read JSON from stdin. Both exit 0 unconditionally (tool call must never be blocked by an emitter).

The existing `8v hooks claude pre-tool-use` (exit 2, blocker) is unrelated and unchanged.

---

## 3. Event Mapping

### 3.1 Caller::Hook schema change

`o8v-core/src/caller.rs`: add `Hook` variant to the `Caller` enum.

```
Hook  →  serializes as "hook"  (serde rename_all = "lowercase" already handles this)
```

`as_str()` and `Display` must include `Hook => "hook"`.

### 3.2 PreToolUse stdin fields consumed

| Stdin field     | Used for                                      |
|-----------------|-----------------------------------------------|
| `tool_name`     | `command` (lowercased), BLOCKED_TOOLS check   |
| `tool_use_id`   | temp file key for run_id correlation (§5)     |
| `session_id`    | SessionId derivation (§4)                     |
| `input`         | argv construction (§3.3), output byte count   |
| `cwd`           | `project_path` resolution                     |

### 3.3 Argv construction

| Tool  | argv                                              |
|-------|---------------------------------------------------|
| Bash  | `["bash", redact(input.command)]`                |
| Read  | `["read", basename(input.file_path)]`            |
| Edit  | `["edit", basename(input.file_path)]`            |
| Write | `["write", basename(input.file_path)]`           |
| Grep  | `["grep", input.pattern, basename(input.path)]`  |
| Glob  | `["glob", input.pattern]`                        |

`basename()` is the §6.1 normalization fence from `log-command.md`. Applied at write time. Redaction (§6) applied to Bash command string before basename or any other normalization.

### 3.4 PostToolUse stdin fields consumed

| Stdin field   | Used for                        |
|---------------|---------------------------------|
| `tool_use_id` | temp file lookup for run_id     |
| `session_id`  | SessionId derivation            |
| `output`      | `output_bytes = len(output)`    |
| `error`       | `success = error.is_none()`     |

`duration_ms = post_wall_clock_ms − pre_wall_clock_ms` (pre timestamp read from temp file; if Claude provides timestamps in payload, prefer those).

---

## 4. Session ID Mapping

`SessionId::from_claude_session_id(raw: &str) -> SessionId`

1. Compute SHA-256 of `raw` (UTF-8 bytes).
2. Encode digest as lowercase hex.
3. Take the first 26 characters.
4. Prepend `ses_`.
5. Return `SessionId("ses_" + first26)`.

This produces a string of length 30 (`ses_` + 26). The existing validator requires `ses_` prefix + `ULID_LEN` (26) chars. The SHA-256 hex alphabet is a superset of Crockford base32 — the validator must accept lowercase hex or the constructor must encode to Crockford base32 instead. **Decision required before coding:** verify whether `SessionId::try_from_raw()` validates alphabet or only length + prefix.

---

## 5. Run-ID Correlation (temp file)

**Location:** `~/.8v/hook_run/<session_id>/<tool_use_id>`

**Contents (newline-delimited):**
```
<run_id (UUID v4 or ULID)>
<pre_wall_clock_ms>
```

**Pre handler:**
1. Mint a new `run_id`.
2. Write temp file. If temp file already exists, skip (idempotent; §7 AC item 4).
3. Emit `CommandStarted` with `run_id`.

**Post handler:**
1. Read temp file by `tool_use_id` key.
2. Parse `run_id` and `pre_wall_clock_ms`.
3. Delete temp file.
4. If temp file missing: emit synthetic `CommandStarted` (`duration_ms = 0`), then `CommandCompleted`.
5. Emit `CommandCompleted` with matching `run_id`.

---

## 6. Redaction Pipeline

Applied in `hook/redact.rs` to the Bash `command` string only. Input → output, applied in order:

1. Replace `sk-[A-Za-z0-9]{20,}` with `<secret>` (API keys).
2. Replace `eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+` with `<secret>` (JWT).
3. Replace `://[^@/\s]+:[^@/\s]+@` with `://<secret>@` (URL credentials).

All patterns are regex. Use the `regex` crate (already in workspace or add as dependency). Patterns are compiled once via `OnceLock<Regex>`. No user-configurable patterns in v1.

---

## 7. Failure Modes

| Condition                        | Behavior                                                     |
|----------------------------------|--------------------------------------------------------------|
| stdin parse failure              | `eprintln!` error; exit 0; no event written                 |
| event store dir missing          | create `~/.8v/` before write; fail → eprintln + exit 0     |
| event store write failure        | `eprintln!` error; exit 0 (tool call never blocked)         |
| temp file dir missing            | create `~/.8v/hook_run/<session_id>/`; fail → eprintln      |
| temp file write failure          | eprintln; exit 0; PostToolUse will produce synthetic Start   |
| temp file missing at Post        | emit synthetic `CommandStarted`, then `CommandCompleted`     |
| tool_use_id absent in payload    | use `"unknown"` as temp file key; log warning to stderr     |

All handlers exit 0 unconditionally. No panic paths in production code.

---

## 8. Installation Command

`8v hook install` — hidden subcommand under `hook`.

Writes the hook block to `~/.claude/settings.json` (global only). Algorithm:

1. Read `~/.claude/settings.json`. If absent, start from `{}`.
2. Check for existing `8v hook pre` / `8v hook post` entries in `hooks.PreToolUse` and `hooks.PostToolUse`.
3. If both already present: print "already installed"; exit 0 (idempotent).
4. Append the two hook entries under the correct matchers.
5. Write back with `serde_json` pretty-print. Preserve unknown keys via `flatten`.

This reuses the merge pattern from `o8v/src/init/claude_settings.rs` but targets `~/.claude/settings.json` (home-relative) rather than the project-scoped `.claude/settings.json`.

---

## 9. Test Plan

All tests are failing-first: write the test, run against pre-fix code to confirm red, then implement.

1. **`caller_hook_serializes_as_hook`** — `Caller::Hook` serializes to JSON string `"hook"` and deserializes back. Fails until `Hook` variant is added.

2. **`session_id_from_claude_session_id_stable`** — Two calls with the same raw string produce identical `SessionId`. Different inputs produce different values. Fails until `from_claude_session_id()` exists.

3. **`session_id_from_claude_session_id_format`** — Result starts with `ses_` and has total length 30. Fails on wrong length.

4. **`redact_api_key`** — Input `"curl -H Authorization: Bearer sk-abc123xyz456def789ghi"` → output contains `<secret>`, not the key. Fails until redact module exists.

5. **`redact_jwt`** — Input containing a syntactically valid JWT shape → `<secret>` in output. Fails until regex compiled.

6. **`redact_url_credentials`** — Input `"git clone https://user:pass@github.com/repo"` → `<secret>` in userinfo position. Fails until pattern 3 applied.

7. **`pre_hook_writes_command_started`** — Simulate `PreToolUse` stdin JSON for `Read` tool → `CommandStarted` event written to temp event store with `caller = "hook"`, `command = "read"`, `argv = ["read", "<basename>"]`. Fails until pre.rs handler complete.

8. **`pre_hook_idempotent`** — Run pre handler twice with same `tool_use_id` → only one `CommandStarted` written, no panic, exit 0 both times. Fails until idempotency check on temp file exists.

9. **`post_hook_writes_command_completed`** — Simulate `PostToolUse` stdin after a known temp file → `CommandCompleted` written with matching `run_id` and positive `duration_ms`. Fails until post.rs reads temp file.

10. **`post_hook_synthetic_start_when_pre_missing`** — Post handler with no pre temp file → both `CommandStarted` (duration_ms=0) and `CommandCompleted` written. Fails until synthetic path implemented.

11. **`hook_exit_zero_on_bad_stdin`** — Feed malformed JSON to pre handler → process exits 0, no panic, stderr contains error text. Fails until error path hardened.

12. **`argv_normalization_strips_path`** — `Read` tool with `file_path = "/home/user/project/src/main.rs"` → argv contains `"main.rs"` not full path. Fails until basename normalization applied.
