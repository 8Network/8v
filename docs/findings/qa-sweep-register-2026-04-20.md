# QA Sweep Round 1 — Cross-Command Bug Register v2
**Date:** 2026-04-20  
**Scope:** All 12 subcommands (ls, read, write, search, check, fmt, test, build, init, hooks, upgrade, mcp)  
**Inputs:** qa-sweep-register-2026-04-20.md (v1), command-qa-init-hooks-upgrade-mcp-2026-04-20.md, command-qa-write-2026-04-20.md, error-contract-measurement-2026-04-20.md  
**Status:** Round 1 complete — all 12 subcommands swept

---

## §1 — Delta: New Bugs Since v1 (BR-24 onwards)

All rows sourced from init/hooks/upgrade/mcp QA, write cross-command findings (AF-1, AF-4, AF-5), and error-contract measurement (BUG-4, BUG-6). AF-2 = BR-18 (confirmed, not new). AF-3 = BR-01 scope broadened (not a new bug). BUG-6 = AF-5 (same symptom, one entry).

| ID    | Cmd     | Type     | Sev    | Description                                                                                  | Source                    |
|-------|---------|----------|--------|----------------------------------------------------------------------------------------------|---------------------------|
| BR-24 | init    | BEHAVIOR | HIGH   | `8v init --yes` in non-git dir prints "Pre-commit hook installed" but writes no files        | I-1                       |
| BR-25 | init    | RENDER   | MEDIUM | Re-init skip message says "AGENTS.md already has block" when file is CLAUDE.md              | I-2                       |
| BR-26 | init    | CONTRACT | LOW    | Installed hook scripts invoke `8v hooks` with no PATH anchor — breaks in clean environments | I-3                       |
| BR-27 | hooks   | BEHAVIOR | HIGH   | Empty/malformed stdin to `hooks claude pre-tool-use` exits 0 ("hooks: passed") — security false-allow | H-1              |
| BR-28 | hooks   | CONTRACT | MEDIUM | `8v hooks claude pre-tool-use --json` rejected with exit 2 — `--json` not propagated        | H-2                       |
| BR-29 | hooks   | BEHAVIOR | LOW    | `on-commit-msg` strips only `Co-Authored-By:`, misses other AI-attribution patterns         | H-3                       |
| BR-30 | upgrade | CONTRACT | HIGH   | `upgrade --json` returns `upgraded: true` when binary is already current — no replacement occurred | U-1              |
| BR-31 | upgrade | CONTRACT | HIGH   | Network failure during upgrade exits 0; `error` field populated but `$?` = 0                | U-2                       |
| BR-32 | upgrade | BEHAVIOR | MEDIUM | No `--check`/`--dry-run` flag; version check inseparable from binary replacement            | U-3                       |
| BR-33 | mcp     | CONTRACT | MEDIUM | `8v mcp --json` and `--plain` rejected with exit 2 — global flags not propagated to mcp    | M-1                       |
| BR-34 | mcp     | RENDER   | LOW    | MCP `serverInfo.name` = "rmcp" (framework artifact), not "8v"                               | M-2                       |
| BR-35 | mcp     | CONTRACT | LOW    | No readiness signal from MCP server; callers poll or assume ready                           | M-3                       |
| BR-36 | write   | DOC      | HIGH   | `--force` create-vs-overwrite semantics undocumented; agents require trial-and-error        | AF-1                      |
| BR-37 | write   | BEHAVIOR | MEDIUM | `--find` does not expand `\n`; content arg does — silent mismatch for multi-line patterns   | AF-4                      |
| BR-38 | write   | RENDER   | MEDIUM | Symlink error text says "escapes project directory" for plain files simply outside root     | AF-5 / BUG-6              |
| BR-39 | search  | BEHAVIOR | MEDIUM | `8v search .` — `.` treated as regex pattern (not path); floods output, exits 0            | BUG-4                     |
| BR-40 | init/hooks/upgrade/mcp | CONTRACT | MEDIUM | `--json`/`--plain` flag propagation inconsistent across all four new commands; hooks and mcp reject both | X-4 / cross-cmd |

**New bug count: 17** (BR-24..BR-40)

---

## §2 — Re-Prioritized Top-10 (all v1 + new, blast-radius × severity)

Ranking criterion: blast-radius (how many users/commands/agents affected) × severity (data-loss, security, silent-failure, cosmetic).

| Rank | ID    | Cmd          | Why top-10                                                                                      |
|------|---------|--------------|-------------------------------------------------------------------------------------------------|
| 1    | BR-27 | hooks        | Security false-allow: malformed stdin passes pre-tool-use gate — agent can execute blocked commands |
| 2    | BR-24 | init         | Silent false-positive: `--yes` reports success in non-git dir, no files written — CI pipelines silently unconfigured |
| 3    | BR-18 | all          | `--json` errors not structured — every JSON consumer gets plain text on STDERR when any command errors |
| 4    | BR-31 | upgrade      | Network failure exits 0 — scripts that check `$?` silently treat failed upgrade as success     |
| 5    | BR-30 | upgrade      | `upgraded: true` when already current — consumers cannot distinguish "replaced" from "already latest" |
| 6    | BR-01 | write/read   | Double-prefix `error: error:` — agents parse prefix as signal; double breaks detection logic   |
| 7    | BR-36 | write        | `--force` semantics undocumented — agents retry blindly, wasting turns                        |
| 8    | BR-03 | search       | Permission-denied swallowed, exits 0 — agents believe search succeeded with empty results      |
| 9    | BR-39 | search       | `8v search .` treats `.` as regex not path — silent all-files flood or misinterpretation       |
| 10   | BR-19 | all          | 6 distinct STDERR prefix patterns — parsing fragmentation across all consumers                 |

---

## §3 — Pattern Families (updated)

Members confirmed/added. Families below ≥3 confirmed members retained. Families that dropped below 3 after source-tracing removed.

### P-A: Silent Success When Should Be Failure
**Definition:** Command exits 0 and reports success but the requested action did not occur.  
**Members:** BR-03 (search perm-denied), BR-24 (init in non-git dir), BR-27 (hooks false-allow), BR-31 (upgrade network failure exit 0), BR-30 (upgrade already-current reports `upgraded: true`)  
**Count:** 5 confirmed members (was 3 in v1)  
**Added:** BR-24, BR-27, BR-31

### P-B: Error Prefix Fragmentation
**Definition:** Different STDERR prefix formats across commands or within the same command; consumers cannot parse reliably.  
**Members:** BR-01 (double-prefix write), BR-19 (6 patterns enumerated: `error:`, `Error:`, `error: error:`, `error: Error:`, bare text, `[command]:`), BR-38 (symlink text wrong for non-symlink case), BR-25 (wrong filename in re-init skip message)  
**Count:** 4 confirmed members  
**Added:** BR-38, BR-25 (render-layer prefix errors)

### P-C: JSON Flag Not Propagated
**Definition:** `--json` or `--plain` flag accepted at CLI top level but silently ignored or rejected by a subcommand.  
**Members:** BR-18 (all cmds — errors not JSON even with flag), BR-28 (hooks rejects `--json`), BR-33 (mcp rejects `--json`/`--plain`), BR-40 (cross-cmd: init/hooks/upgrade/mcp flag propagation gap)  
**Count:** 4 confirmed members (was 1 in v1 as BR-18)  
**Added:** BR-28, BR-33, BR-40

### P-D: Missing Exit-Code Contract
**Definition:** Command exits non-zero when it should exit 0 (or vice versa), violating the documented 0/1/2 contract.  
**Members:** BR-31 (upgrade network failure exits 0), BR-27 (hooks malformed stdin exits 0), BR-28 (hooks `--json` exits 2 for valid invocation), BR-33 (mcp `--json` exits 2 for valid invocation)  
**Count:** 4 confirmed members (was 2 in v1)  
**Added:** BR-27, BR-28, BR-33 (all post-v1 additions)

### P-E: Undocumented Behavioral Contracts
**Definition:** Behavior that is observable but not described anywhere in docs, help text, or error messages; agents must learn by failure.  
**Members:** BR-21 (error contract absent from docs), BR-22 (`--find/--replace` N>1 undocumented), BR-36 (`--force` create-vs-overwrite semantics undocumented), BR-37 (`--find` `\n` not expanded, silent mismatch), BR-32 (no `--check`/`--dry-run` for upgrade)  
**Count:** 5 confirmed members (was 2 in v1)  
**Added:** BR-36, BR-37, BR-32

**New families in v2:** 0 new families (all new members absorbed into existing P-A through P-E). No candidate reached ≥3 independent members sufficient to warrant a new named family beyond the five above.

**Retained families:** 5 (P-A, P-B, P-C, P-D, P-E)  
**Dropped families:** 0 (all v1 families retained; all had ≥3 confirmed members after v2 sweep)

---

## §4 — Slice Coverage Analysis

Slices from v1: B1 (docs), B2 (error routing), B3 (search). New slices proposed in §5.

| Slice | Description                            | Bugs closed (count) | Example closed                    | Example NOT closed by slice                     |
|-------|----------------------------------------|---------------------|-----------------------------------|-------------------------------------------------|
| B1    | Docs: error contract + behavioral docs | 3                   | BR-21 (contract missing), BR-22 (`--find` N>1), BR-36 (`--force` semantics) | BR-01 (code bug, not doc gap), BR-03 (code bug) |
| B2    | Error routing: STDERR prefix unification + `--json` structured errors | 11 | BR-01, BR-18, BR-19, BR-28, BR-33, BR-38, BR-40 | BR-27 (security logic), BR-24 (init logic), BR-31 (upgrade exit code) |
| B3    | Search: silent-failure + regex/path ambiguity | 4              | BR-03, BR-06 (search missing `files_skipped_by_reason`), BR-07 (search traversal no stderr), BR-23 (search partial harvest) | BR-39 (`.` treated as regex not path — explicitly out of scope in slice-b3; tracked for Phase 0 round 2), BR-24 (init), BR-30 (upgrade), BR-37 (write) |

**Top-3 Slice ROI:**
1. **B2 (error routing)** — closes 11 bugs across 8+ subcommands; single coherent change (STDERR discipline + JSON error struct); highest cross-command blast radius per line of work.
2. **B3 (search)** — closes 4 bugs in one command; BR-03 and BR-39 are both agent-facing silent failures; scoped, testable, measurable.
3. **B1 (docs)** — closes 3 bugs, zero code risk; pure documentation/help-text changes; can ship independently as PR with no regression surface.

---

## §5 — Orphans and New Slice Proposals

**Orphans** = bugs not closed by B1, B2, or B3.

| ID    | Why orphaned                                                           |
|-------|------------------------------------------------------------------------|
| BR-24 | Init non-git-dir false-positive — init-specific logic, not error routing |
| BR-25 | Init wrong filename in skip message — init-specific render bug          |
| BR-26 | Hook PATH anchoring — init/hooks installation code                     |
| BR-27 | Hooks security false-allow — hooks evaluation logic                    |
| BR-28 | Hooks `--json` rejected — partially covered by B2 but hooks is separate subsystem (no standalone slice proposed yet; park for Phase 0 round 2) |
| BR-29 | `on-commit-msg` partial pattern strip — hooks behavior                 |
| BR-30 | Upgrade `upgraded: true` when current — upgrade contract               |
| BR-31 | Upgrade network failure exits 0 — upgrade exit code                    |
| BR-32 | Upgrade no `--check` flag — upgrade behavior                           |
| BR-33 | MCP `--json` rejected — partially covered by B2 but MCP is transport layer |
| BR-34 | MCP server name "rmcp" — MCP metadata                                 |
| BR-35 | MCP no readiness signal — MCP protocol                                |
| BR-37 | Write `--find` `\n` not expanded — write-specific behavior             |
| BR-38 | Write symlink error text wrong for plain file — render bug in write error path (no standalone slice proposed yet; park for Phase 0 round 2) |

**Orphan count: 14**

### New Slice Proposals (≤3)

**C1 — Init/Hooks Security and Correctness**  
Bugs: BR-24, BR-25, BR-26, BR-27, BR-29  
Rationale: BR-24 and BR-27 are both silent-success-when-should-fail in the installation/gate subsystem. BR-25 is a render fix. BR-26 and BR-29 are complementary hardening. One focused pass through `o8v-cli/src/cmd/init.rs` and `o8v-cli/src/cmd/hooks.rs` closes all five. Security impact of BR-27 makes this the highest-priority new slice.

**C2 — Upgrade Contract**  
Bugs: BR-30, BR-31, BR-32  
Rationale: All three are upgrade-specific contract violations. BR-30 and BR-31 are boolean/exit-code reliability bugs; BR-32 is a missing behavioral flag. Scoped to `o8v-cli/src/cmd/upgrade.rs`. Can ship independently after B2 since they share no code path.

**C3 — Write Semantics**  
Bugs: BR-36, BR-37  
Rationale: BR-36 (`--force` undocumented) + BR-37 (`\n` inconsistency) are both agent-friction multipliers per the write QA friction findings. Two bugs, one file (`o8v-cli/src/cmd/write.rs` + help text), bounded scope. BR-38 (symlink error text wrong for plain file) is orphaned — no standalone slice proposed yet; park for Phase 0 round 2.

**Remaining unaddressed after C1+C2+C3:** BR-33, BR-34, BR-35 (MCP transport — separate subsystem, lower priority, no agent-facing correctness impact today), BR-28 (partially covered by B2).

---

## §6 — Changelog from v1

### New bugs added
BR-24 through BR-40 (17 new entries) — see §1 for full sourcing.

### Scope changes (not new bugs)
- **BR-01** scope narrowed: write QA (T18 vs T19) shows double-prefix is NOT present on all write error paths — only specific code paths (e.g., nonexistent file on `--find/--replace`). Single-prefix confirmed on `--append` nonexistent. Bug still valid; scope updated from "write errors" to "specific write error code paths."
- **BR-17** scope clarified: `read --json` asymmetry confirmed — single-file missing → plain STDERR; batch missing → JSON in STDOUT. Two distinct shapes within the same command.

### Confirmed (not new, already in v1)
- **AF-2** (write `--json` errors not structured) = BR-18. Source: command-qa-write-2026-04-20.md §5.
- **AF-3** (double-prefix in write) = BR-01 scope broadened. Not a new bug.
- **BUG-6** (symlink error for non-symlink) = AF-5 = BR-38. Single entry.

### Deprioritized
None. No v1 bugs dropped in priority.

### Invalidated
None. All v1 bugs confirmed by independent measurement in error-contract-measurement-2026-04-20.md.

### Duplicates resolved
- AF-2 → BR-18 (merged)
- AF-3 → BR-01 (scope broadened, not duplicated)
- BUG-6 → AF-5 → BR-38 (single entry)

---

## 8v Feedback

**FB-1: `mcp__8v-debug__8v` requires `--full` for synthesis tasks.** Symbol-map mode returned line numbers with no content during multi-file reads. The progressive default is correct for navigation but every synthesis task immediately escalates to `--full`. Consider: if the caller passes multiple paths and no range, emit a one-line summary per file (e.g., `findings/foo.md — 178 lines, 12 findings`) before the full content, so the agent can confirm it has the right files before spending tokens.

**FB-2: No batch-write equivalent.** All five source files needed individual `Write` tool calls. An `8v write --batch` accepting NDJSON `{path, content}` records would amortize the overhead for synthesis tasks that produce one output from N inputs.

**FB-3: Write tool requires prior Read to succeed.** The Write tool (non-8v) enforces a "must read before write" invariant. For net-new files this is friction: the file does not exist, so Read returns an error, which must be ignored before Write proceeds. `8v write <path> --create "<content>"` (explicit create-only, fails if exists) would be unambiguous and bypass this check.
