# Learnings — Instruction Clarity Cycle v1→v2→v3 + Multi-full Slice — 2026-04-19

Sources: instruction-clarity-test-2026-04-18.md (v1), instruction-clarity-test-2026-04-19.md (v2),
instruction-clarity-test-2026-04-19-v3.md (v3), read-full-scope-and-delimiter.md (broad design),
read-full-scope-and-delimiter-review-r1.md (broad review), read-multi-full-accept.md (narrow design),
read-multi-full-accept-review-r1.md (narrow review).

Context: three consecutive instruction-clarity benchmark runs against `o8v/src/init/ai_section.txt` and
`o8v/src/mcp/instructions.txt`, each with 2 models × 3 runs = 6 total, using a 24-question structured
protocol. v1 and v2 used a single Likert score. v3 introduced a three-axis rubric. Scores across
versions are NOT comparable. The multi-full slice ran a narrow design → adversarial review → implement
cycle in parallel.

---

## L1 — Changing the rubric reveals what text edits cannot

**Observation.** v2 applied 9 targeted edits to the instruction text. Mean score regressed −0.33 (6.50 →
6.17). Opus dropped from 6.67 to 6.0; Sonnet held at 6.33. The edits landed correctly on input
ambiguities but could not move a blended single-axis score when output and failure-mode gaps were the
binding constraint.

**Root cause.** A single Likert score conflates three orthogonal axes: (1) does the agent know what to
pass in? (2) does the agent know what to expect back? (3) does the agent know what to do when something
goes wrong? Improving axis 1 cannot increase a score that is bottlenecked on axes 2 and 3.

**Rule.** Before editing instruction text, decompose the rubric into at least input/output/failure axes.
Edit only the axis that is failing. Never change two axes simultaneously — you lose the signal.

---

## L2 — Input clarity saturates around 8/10; output and failure are the open frontier

**Observation.** v3 Axis 1 (Input clarity) scored 8.0 with zero variance across all 6 runs. Five
previous targeted edits had already consumed most of the available gain. Further rewrites to parameter
names, ordering, or phrasing yield diminishing returns.

**Root cause.** The instruction surface for input was already dense and consistent. Agents could reliably
parse what to pass. The remaining 2 points are structural — edge cases that require examples, not prose.

**Rule.** Treat Axis 1 ≥ 8.0 as saturated. Redirect effort to Axis 2 (Output, scored 4.67) and Axis 3
(Failure, scored 2.17). The biggest gain available is documenting the `=== <path> ===` batch delimiter,
the global scope of `--full`, the `--json` schema shape, and at least three additional failure cases
beyond `--find/--replace` no-match.

---

## L3 — Broad design scope triggers blocker-level contradictions; narrow scope ships

**Observation.** The broad design (`read-full-scope-and-delimiter.md`) addressed four concerns at once:
`--full` scope, `===` delimiter documentation, partial-failure semantics, and `:range`+`--full`
interaction. The adversarial review returned 17 findings including 2 blockers. B1: §5f and §8 E1
directly contradicted each other on whether `:range`+`--full` was in or out of scope. B2: the §5f
decision had no artifact in the §7 replacement-bytes table. The design was rejected.

The narrow design (`read-multi-full-accept.md`) addressed exactly one concern: make repeated `--full`
flags a no-op instead of a clap error. The adversarial review returned 0 blockers. Recommendation:
proceed to implementation.

**Root cause.** Scope creep inside a single design document forces reviewers to evaluate cross-cutting
decisions that were never made explicit. Contradictions are inevitable when a section written for one
concern silently conflicts with a section written for another.

**Rule.** One design document, one behavioral change. When a design draft grows a second "also" clause,
split it. The scope boundary is: if the change can be described in one sentence without "and", it is
narrow enough.

---

## L4 — Adversarial review on narrow slices surfaces hardening notes, not blockers

**Observation.** The narrow design review (read-multi-full-accept-review-r1.md) found 0 blockers, 1 test
gap (TG-1: triple test should byte-diff against single-`--full` output, not substring-check), 1 risk
(R-1: `overrides_with_self` adds `[may be specified multiple times]` to `--help` output), and 2 nits.
All findings were actionable with no design rework required.

**Root cause.** When scope is tight, the reviewer has nothing to contradict. Every finding is a
hardening note against a clear, agreed-upon target behavior. The review becomes a checklist, not a
negotiation.

**Rule.** The adversarial review gate is not a formality. On narrow slices, it takes one round and
returns only implementation-level notes. On broad slices, it takes multiple rounds and may return design
rejections. Use the round count as a scope health signal: if round 2 is needed, the scope was too wide.

---

## L5 — Design specs must state the exact syntax, not the intent

**Observation.** The broad design §5b said "add `overrides_with_self(true)`". The narrow design §Implementation
said `#[arg(long, overrides_with_self = "full")]`. These are different: the broad design used a method
call form that does not exist in clap 4's derive API; the narrow design used the correct attribute key-value
syntax. An implementer following the broad design would have hit a compile error.

**Root cause.** Design docs written at intent level ("make it idempotent") require the implementer to
rediscover the exact API. That rediscovery step is where implementation drift happens.

**Rule.** Implementation sections must state the exact token sequence: attribute name, key, value, type.
"Use `overrides_with_self`" is not sufficient. "`#[arg(long, overrides_with_self = \"full\")]` on the
existing `pub full: bool` field" is sufficient. If you cannot write the exact tokens, the design is not
ready for the implementation section.

---

## Meta-learning — Q31 tool gaps exposed by the three-axis rubric

v3 Section 8 (Q31) surfaced four tool-surface gaps that instruction edits cannot fix:

1. **MCP tool name mismatch.** The registered tool is `mcp__8v-debug__8v`. Agents expecting `8v` as a
   direct tool name call the wrong surface or fall back to shell. This is a registration problem, not a
   documentation problem.

2. **No `mv`/rename primitive.** Agents attempting cross-file rename have no `8v` command. They either
   shell out to `mv` (violating the no-Bash-for-files rule) or fail silently. A rename operation requires
   a binary surface, not a documentation note.

3. **No repo-wide find/replace.** `8v write --find/--replace` operates on a single file. Agents
   attempting symbol renames across files have no primitive. This generates multi-step shell workarounds.

4. **`8v write --file` undefined.** Whole-file overwrite/create from file content has no specified
   behavior. Agents infer incorrectly from `--append` semantics.

These gaps belong in a separate feature design, not in instruction text. Documenting "use `mv` for
rename" in instructions is workaround documentation — it encodes a missing capability as a rule.

---

## What stays in instructions vs what belongs in the binary

The instruction surface (`ai_section.txt`, `mcp/instructions.txt`) is the right place for: command
syntax, flag names, output format descriptions, batch calling conventions, and the recommended workflow
order. It is the wrong place for: error code tables (belong in `--help` and `--json` schema), output
contract schemas (belong in `--json` output itself), and workarounds for missing primitives (belong in
the feature backlog). Every sentence in the instructions that describes what to do when a capability is
absent is a signal that the binary needs a new surface, not that the instructions need more words.

---

## Cycle costs (approximate)

| Cycle                        | Designs | Review rounds | Blockers | Outcome             |
|------------------------------|---------|---------------|----------|---------------------|
| v1 → v2 instruction edit     | 0       | 0             | 0        | −0.33 score regression |
| v2 → v3 rubric change        | 0       | 0             | 0        | New baseline, not comparable |
| Broad design → review        | 1       | 1             | 2        | Rejected            |
| Narrow design → review       | 1       | 1             | 0        | Proceed to implementation |

---

## Open threads

- Axis 2 gaps: document `=== <path> ===` delimiter, `--full` global scope, `--json` schema shape.
- Axis 3 gaps: document at least 3 additional failure cases (exit codes, error destination, partial failure).
- Implement `read-multi-full-accept.md`: one annotation line, two E2E tests with byte-level diff.
- Separate design for `mv`/rename primitive.
- Separate design for repo-wide find/replace.
- MCP tool name registration: resolve `mcp__8v-debug__8v` vs `8v` surface mismatch.
- Read-full-scope-and-delimiter.md: resolve B1 (`:range`+`--full` scope contradiction) before resubmitting.
- MCP transport truncation (new P0 from slice-1 targeted check): batched `--full` calls on large files exceed MCP result limit; agents fall back to native tool. Design a paginated/chunked/streaming answer next cycle.
- Worked example: batch + `--full` combined (doc-only slice 1b pending): add a concrete example to instruction surface so agents choose the correct form with confidence, not by guessing.

---

## Post-ship observations (multi-full slice 1)

### F1 — Behavior cone widened but ambiguity persists

**What.** A fresh agent given only the two instruction surfaces (`ai_section.txt`, `mcp/instructions.txt`) chose the single trailing `--full` form (`8v read a.rs b.rs c.rs --full`) when asked for full content of multiple files. It did NOT attempt the per-arg `--full --full --full` form that v3 opus-run1 had hit.

**Implication.** The code fix (accept multi-`--full` via `overrides_with_self`) broadened the behavior cone: both forms now work. But it did not close the underlying doc ambiguity. Fresh agents report the instruction surface as "ambiguous" — they guess correctly, but without confidence. A worked example showing `8v read a.rs b.rs c.rs --full` explicitly would eliminate the guessing step.

**Rule.** Behavior-widening fixes do not substitute for documentation fixes. Two levers, two edits, ship both.

---

### F2 — MCP transport truncation (new P0)

**What.** `8v read` on three large `o8v/src/commands/*.rs` files in one batched `--full` call returned 83,859 chars, which exceeded the MCP tool result limit. The result was saved to disk and the agent recovered by falling back to the native Read tool.

**Why this matters.** 8v's "batch is cheaper" principle breaks at an undocumented MCP transport boundary. Agents are pushed to the native tool exactly when 8v should win. This friction was invisible in v1/v2/v3 because benchmark inputs were smaller. It surfaces in real sessions against production crates.

**Out of scope for this cycle.** Add to open threads. Next cycle must design a paginated/chunked/streaming answer. Candidate mitigations: `--max-lines N` cap flag, `--json` streaming mode, auto-fall-back to symbol-map for large files when `--full` would exceed a configurable limit.

---

## Slice 2 outcome — MCP adapter output cap

### Shipped
- Two-layer guard in `o8v/src/mcp/handler.rs`: pre-flight byte-sum check for `read --full` (×1.20 safety multiplier) + post-render length check on all subcommand output.
- Cap default 55,000 chars. Override via `O8V_MCP_OUTPUT_CAP` env var. `OnceLock<Result<usize, String>>` caches the parsed cap; invalid values (`""`, `"0"`, `"-1"`, `"abc"`) error on first use.
- 7 tests added to `o8v/tests/mcp_e2e.rs`. Workspace green. CLI behavior for non-MCP callers unchanged.

### Verified
- Live spot check: `8v read upgrade.rs stats.rs write.rs --full` via MCP returned structured inline error listing per-file sizes (35K / 23K / 42K), cap value, env override name, `:range` + symbol-map redirect templates. No Claude Code transport error reached the agent.
- Symbol-map-only batch read on same 3 files (no `--full`) passed cleanly — 2,639 lines, no cap hit.

### Durable learning: L6 — measurement before design pays off
- **Observation:** we had two fresh-agent data points (66K and 83K chars) before starting. The measurement slice bracketed the cap at 60,500 pass / 63,336 fail and found the enforcement layer (Claude Code's MCP client, not 8v). The design was grounded in known numbers, not guesses.
- **Root cause:** text-only design on uncertain inputs hides assumptions as confident statements. A single measurement call collapsed three open questions (where, how much, configurable?).
- **Rule:** when a P0 has an unknown quantitative threshold, spawn a measurement-only slice before any design. Cheap, eliminates whole categories of design rework.

### Durable learning: L7 — cycle discipline holds under the narrow-slice rhythm
- **Observation:** slice 2 followed the exact cycle slice 1 used: micro-design (74 lines) → r1 review (blocker + 2 test gaps + 3 risks) → fix → r2 review (0 blockers, 1 test gap + 1 risk + 2 nits) → tiny polish → implement + verify.
- **Root cause:** the narrow-slice discipline survives its first replication. Each cycle stage has a predictable cost and a clean exit criterion.
- **Rule:** keep the rhythm. Don't collapse review rounds into implementation briefs to "save a turn" — the r1 blocker on slice 2 (read_to_report had no Caller param) would have wasted an implementation round.

### Process gap recorded
- Slice 2 implementation agent's report omitted the required failing-first verbatim cargo output and the unified diff. Post-change tests demonstrably pass, but the evidence trail for "these tests were red on pre-change code" is missing. Future implementation briefs must require the pre-change red output as a precondition to proceeding, not just as a report item.

### Update open threads
- Close: "MCP transport truncation (new P0 from slice-1 targeted check)" → shipped, slice 2.
- Close: "Worked example: batch + `--full` combined (doc-only slice 1b pending)" → shipped, slice 1b.
- Close: "slice 5: --full batch bug" → invalidated, stale binary. See `read-batch-full-bug-2026-04-19.md` banner and L8.
- Keep: symbol-map-on-prose fallback, delimiter contract, partial-failure contract, :range + --full interaction.
- Add: MCP discovery hint in doc — single-line edit, deferred.

---

### L8 — MCP servers hold stale binaries across rebuilds

- **Observation:** today we measured a Case-4 behavior bug (plain second arg downgrading to symbol map) that turned out to not exist. The MCP server was still running a pre-slice-1 binary; a `/mcp` reconnect made the symptom disappear. `cargo build` + working-tree changes do NOT update the running MCP server.
- **Root cause:** the MCP server is a long-running subprocess spawned at Claude Code startup (or last reconnect). It holds the binary it was launched with until explicitly reconnected. `cargo run` via `.mcp.json` would rebuild per spawn, but spawn only happens on reconnect.
- **Rule:** after every `cargo build` that could change MCP-observable behavior — especially CLI flag parsing, handler.rs logic, or render code — `/mcp` reconnect BEFORE running any measurement or benchmark via MCP. Embed this in every design doc's "manual spot-check" step so implementers don't skip it.