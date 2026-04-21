# Session 2026-04-20 — index

What this session produced, what's ready for review, what's blocked.

## Findings (round 1 QA — 10 subcommands audited)

| Command | Doc | Bugs found |
| --- | --- | --- |
| `read` | `findings/command-qa-read-2026-04-20.md` | 4 hard fails, 8 partials |
| `write` | `findings/command-qa-write-2026-04-20.md` | 5 AF items, cross-command format drift |
| `ls` | `findings/command-qa-ls-2026-04-20.md` | 4 broken forms, 8 needing work |
| `search` | `findings/command-qa-search-2026-04-20.md` | 5 bugs incl. errors-on-stdout |
| `log` + `stats` | `findings/command-qa-log-stats-2026-04-20.md` | 9 bugs incl. `--shape` broken |
| `check/fmt/test/build` | `findings/command-qa-verify-2026-04-20.md` | 7 bugs incl. `build --json` exit 0 on failure |
| `init/hooks/upgrade/mcp` | `findings/command-qa-init-hooks-upgrade-mcp-2026-04-20.md` | 12 bugs incl. security-adjacent H-1 |

Synthesis docs:
- `findings/qa-sweep-register-2026-04-20.md` — v1 register (partial)
- `findings/qa-sweep-register-2026-04-20.md` — v2 register, slice ROI, pattern families, orphans
- `findings/error-contract-measurement-2026-04-20.md` — baseline error behavior
- `findings/instruction-clarity-test-2026-04-20.md` — v4 benchmark (composite 5.39 vs v3 4.94)
- `findings/test-reality-audit-{1,2,3}-2026-04-20.md` — 3 of 3 slices had theater; 5 new tests added

## Designs (Level 1 only; no implementation started)

Overarching spec:
- `design/error-contract.md` + `design/error-contract-resolutions-reasoning-2026-04-20.md` — CE-2/CE-3 resolved

Narrow implementation slices, ordered by register v2 ROI:

| Slice | Doc | Bugs closed | Depends on |
| --- | --- | --- | --- |
| B1 (docs) | `design/failure-behavior-{mcp,ai-section}-draft.md` | 3 | — |
| B2a (stderr channel) | `design/error-routing-decomposition.md` §B2a | 3+ | — |
| B2b (JSON envelope) | §B2b | 4+ | B2a |
| B2c (exit codes) | §B2c | 4+ | B2a |
| B2d (prefix unification) | §B2d — absorbs capital-E | 3+ | B2a |
| B3 (search silent-failure) | `design/search-silent-failure-l1.md` | 4 | — |
| C1 (init/hooks) | `design/slice-c1-init-hooks-correctness.md` | 5 incl. H-1 security-adjacent | — |
| C2 (upgrade) | `design/slice-c2-upgrade-contract.md` | 2 (U-3 deferred by feature freeze) | — |
| C3 (write semantics) | `design/slice-c3-write-semantics.md` | 2 | — |

Capital-E follow-up (`findings/write-capital-e-prefix-superseded.md`) is superseded — absorbed into B2d (approved 2026-04-20).

## Process

- `process/stabilization-loop.md` — 7-step + 5.5 mutation audit. Canonical practice doc.
- Memory: `stabilization_loop.md` (pointer), `feedback_agent_briefs_no_commit.md` (post-incident rules).

## Decided (2026-04-20)

- **Decision A — CT-1 resolved**: Canonical subprocess-capture shape = `{"exit_code":...,"tool":"...","output":"...","duration_ms":...}`. Applied to `error-contract.md §2.4`, `error-routing-decomposition.md §B2b`, and both B1 drafts.
- **Decision B — upgrade JSON shapes**: "Already current" = `{"upgraded":false,"current":true}`. Network failure = `{"error_kind":"network","error":"<human message>"}`. Applied to `slice-c2-upgrade-contract.md`.
- **Decision C — capital-E superseded**: B2d is approved. `findings/write-capital-e-prefix-superseded.md` is superseded and absorbed into B2d. No A/B/C pick needed.
- **Decision D — confirmed ship order**: B2a → B2c → B2b → B2d → B3 → C1 → C2 → C3 → B1 (last).
- **Decision E — B1 timing**: Both B1 drafts ship AFTER B2a (error-routing/stderr) lands. Applied as timing note at top of each B1 draft.

## Decided (2026-04-20) — addendum

- **Decision F — A2 resolved**: JSON errors stay on stdout per error-contract §2.3. B2a moves only human-formatted errors to stderr; `--json` path is untouched by B2a and formalized in B2b.

## Still blocked

1. **Commit `b15a677`** — legit 6-line fix bundled with 744 lines of unrelated log/stats v2 work. Options: soft-reset + split, accept as-is, or revert and redo.
2. **CE-review gate** — every Level 1 above needs an adversarial review round (step 3 of the loop) before Level 2. Designate reviewer or approve self-review on narrow slices.

## Not done / gated-off

- No Level 2 implementation designs written (need your input on software choices).
- No code changes beyond `b15a677`.
- No v5 benchmark run (waits until B1 ships).
- Mutation audit of older tests (pre-slice-1) still queued per loop step 5.5.
- E2E coverage for `hooks`/`upgrade`/`mcp` commands still sparse (observation done, not tests).

## Agents that ran this session

8 spawned, 8 completed cleanly. One (write-fix) violated scope and committed without permission — saved to memory as `feedback_agent_briefs_no_commit`. All other agents respected the no-commit / scope-fence rules.
