# Instruction-Clarity Benchmark

Measures how well 8v's instruction surfaces teach correct tool usage to a blind agent
(no file access, no repo context). Drives the Phase 0 stabilization cycle.

---

## Problem

AI agents fail at 8v tasks in ways that are not model failures: they misread the
instructions. Every wrong default, every unnecessary retry, every fallback to Bash is
a comprehension failure that the text caused.

## Why the problem exists

Both instruction surfaces (CLAUDE.md block, MCP tool description) are dense on
invocation syntax and silent on output contracts. An agent can memorize every flag
and still not know: what does `8v read` actually return, and what happens when it
fails? That gap is not fixable by rewording what is already there. It requires adding
facts that are absent.

## What we measure

| Metric | Definition |
|--------|------------|
| Mean clarity score | Average Q23 composite across all 6 runs (1-10) |
| Scenario confidence mean | Average across 15 Q12 scenario ratings (1-5) |
| Ambiguity count | Mean ambiguity items listed per run (Q6) |
| Fallback trigger count | Scenarios where agent chose Bash over 8v (Q12) |
| Output-clarity sub-score | Q23 axis 2 — new in v3, split from composite |
| Failure-mode sub-score | Q23 axis 3 — new in v3, split from composite |

Consensus threshold: >=4/6 runs = universal finding; 2-3/6 = split-verdict.

## How we run it

1. Open `prompt-template-v2.md` (or `prompt-template-v1.md` for baseline comparisons).
2. Replace `{{SURFACE_1_CLAUDEMD}}` with the current content of `o8v/src/init/ai_section.txt`.
3. Replace `{{SURFACE_2_INSTRUCTIONS_TXT}}` with the current content of `o8v/src/mcp/instructions.txt`.
4. For each of 6 runs, set `{{MODEL_RUN_ID}}` to e.g. `Sonnet, run 1`.
5. Spawn agent with `subagent_type: general-purpose`, the filled prompt, no tools.
6. Collect all 6 outputs. Aggregate scores. Compare to prior baseline.

**Do not edit the question set between runs of the same template version.** Changing
questions invalidates the comparison. Create a new numbered template and re-baseline.

## v1 -> v2 results

| Metric | v1 (2026-04-18) | v2 (2026-04-19) | Delta |
|--------|-----------------|-----------------|-------|
| Mean clarity score | 6.50 | 6.17 | -0.33 |
| Sonnet mean | 6.33 | 6.33 | 0.00 |
| Opus mean | 6.67 | 6.00 | -0.67 |
| Edits applied | — | 9 | — |
| Edits fully resolved | — | 4/9 | — |

Regression: Opus dropped after surface edits; Sonnet held flat. Five of nine edits did
not close the gaps they targeted. A new universal gap emerged post-edit: output/error
contract (6/6 runs) — not present in v1 top-5.

## The ceiling hypothesis

The current surfaces are structurally capped. They teach invocation syntax well enough
that re-wording what is already there cannot raise the score. The remaining
comprehension gaps are all facts that are not present in the text:

- What does a successful `8v read` response look like? (format, per-file labeling)
- What exit code does `8v check` return on failure?
- Where does error output go — stderr, stdout, or `--json`?
- What is the `--json` schema for each command?
- What exactly does `8v check` scope to vs `8v test`?

To test this hypothesis, v3 splits Q23 into three independent axes. If output-clarity
and failure-mode sub-scores are consistently lower than input-clarity, the structural
explanation is confirmed with data.

## Taxonomy of 6 edit mistake types (v1 -> v2)

| Type | Description | v1 example | v2 status |
|------|-------------|------------|-----------|
| T1 — Absent fact | Information gap: the fact was never stated | range indexing never stated | resolved |
| T2 — Undefined term | A term used without definition | "symbol map" output format undefined | resolved |
| T3 — Enum gap | An option list truncated or absent on one surface | --stack values absent from MCP surface | partially resolved |
| T4 — Contract silence | Output format or error behavior not specified | --json schema unspecified, exit codes absent | still open |
| T5 — Scope blur | Boundary of a command is unclear | "most Bash" boundary, verify scope | partially resolved |
| T6 — Cross-surface drift | Surface 1 and Surface 2 describe the same thing differently | CS-1 through CS-5 | 3/5 closed |

T4 (contract silence) is the dominant unresolved category in v2. All six runs cited it.

## v3 strategy

1. **Add output contracts** — for each command, specify: return format on success,
   exit code, where error text appears (stderr vs --json), what `--json` looks like.
2. **Resolve T6 remainder** — align Surface 1 and Surface 2 on search output format
   and verify scope wording.
3. **Add Q25-Q33** to `prompt-template-v2.md` — nine questions that directly probe
   output contracts, behavioral dry-run, tool-gap surfacing, and memorability.
4. **Split Q23 into 3 axes** — measure input-clarity, output-clarity, failure-mode
   clarity separately; composite mean = (axis1 + axis2 + axis3) / 3.
5. **Target**: composite mean >= 7.5; output-clarity axis >= 6.5 (currently untested).

## Roadmap

| Stage | Gate |
|-------|------|
| v3 surface edits | T4 contract facts added to both surfaces |
| v3 run | 6 runs with prompt-template-v2.md |
| v3 analysis | Per-axis Q23 breakdown; ceiling hypothesis confirmed or refuted |
| v4 planning | Ceiling confirmed: structural redesign of MCP description. Refuted: targeted rewording. |

---

## Links

- v1 findings: `docs/findings/instruction-clarity-test-2026-04-18.md`
- v2 findings: `docs/findings/instruction-clarity-test-2026-04-19.md`
- v1 prompt (baseline, do not edit): `docs/benchmarks/instruction-clarity/prompt-template-v1.md`
- v2 prompt: `docs/benchmarks/instruction-clarity/prompt-template-v2.md`
- Surface 1: `o8v/src/init/ai_section.txt`
- Surface 2: `o8v/src/mcp/instructions.txt`
