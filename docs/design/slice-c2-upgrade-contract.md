# Slice C2 — upgrade contract (Level 1)

## Why this slice exists

Two bugs in `8v upgrade` violate the "exit code reflects outcome" contract. Source: `docs/findings/command-qa-init-hooks-upgrade-mcp-2026-04-20.md`.

- **U-1**: `upgrade --json` returns `upgraded: true` when the binary is already current. The field lies; agents that branch on it act as if a replace happened.
- **U-2**: `upgrade` exits 0 on network failure. The CI/agent that wraps `8v upgrade && <restart>` proceeds past a failed upgrade.

Both fall under pattern family P-A ("Silent success when should be failure") per register v2 §3.

U-3 (no `--check`/`--dry-run` mode) is a NEW-FLAG request. Blocked by the 2026-04-14 feature freeze. Out of scope — logged for post-Phase-0.

## Scope

- `upgrade` exit code must be non-zero on any network failure (DNS, TCP, HTTP non-2xx).
- `upgrade --json` `upgraded` field must reflect actuality: `true` only when binary was replaced on disk; `false` when already current; field absent when the attempt errored.
- When errored, `--json` must emit the two-level error envelope per error-contract Level 1 (`{"error":"...","code":"..."}`).

Out of scope:
- `--check`/`--dry-run` flag (U-3) — feature freeze.
- Upgrade channel selection, version pinning, rollback.
- Any change to how the new binary replaces the old one on disk.

## What changes, in one sentence per bug

- **U-1**: the path that returns "already current" must not emit `upgraded: true`. Shape: `{"upgraded":false,"current":true}` (Decision B: keep `upgraded` boolean, add `current: true`).
- **U-2**: any network error propagates to a non-zero exit code. `--json` error envelope for network failure: `{"error_kind":"network","error":"<human message>"}` (Decision B: enum-style `error_kind`; literal string `"network"` is the enum value).

## Why each change

U-1 cited in command-qa §U-1 with reproducer (run `upgrade --json` twice; second call still says `upgraded: true`).
U-2 cited with reproducer (disable network, run `upgrade`, observe exit 0).
Both trace directly to agent-facing harm: a downstream step that treats `$?` as "did it work?" gets the wrong answer.

## Counterexamples

1. **Rate-limit / 429 from upgrade server.** Not a "network failure" per se, but definitely not a success. Must map to non-zero exit + error envelope.
2. **Partial download.** If upgrade writes a truncated binary, the current code may already handle it — confirm or add a test.
3. **Already-current edge.** Binary hash matches remote latest, but remote has a newer pre-release. Behavior: "already current" vs "out of date on pre-release channel" — scope says no channel selection, so we say "already current" when versions match and document that.
4. **Offline cache hit.** If there's a cached manifest that says "current", and network is down, do we trust cache? Level 2 decision — default: no (fail, exit 1).
5. **Disk-full on replace.** Already non-zero exit today? Verify. If not, same class as U-2.

## Failing-first acceptance tests

- `upgrade_returns_nonzero_on_network_failure`
- `upgrade_json_upgraded_false_when_already_current`
- `upgrade_json_emits_error_envelope_on_network_failure`
- `upgrade_nonzero_on_http_429`
- `upgrade_json_field_absent_when_network_error`

Each must fail on current binary before any code change.

## Gate

No implementation until founder reviews Level 1 + counterexamples. Level 2 is a separate doc.

## Out of scope (explicitly)

- Any upgrade-server side contract.
- Colored/verbose upgrade output.
- A `rollback` subcommand.
- Telemetry on upgrade outcomes.
