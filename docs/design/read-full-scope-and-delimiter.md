# Design: `read --full` Scope Ambiguity and Batch Delimiter Documentation

**Status:** Draft  
**Phase:** 0 Stabilization  
**Date:** 2026-04-19  
**Scope:** Documentation + test coverage only. No new flags, no new behavior.

---

## 1. Problem

Two coupled gaps surfaced in the v3 instruction-clarity benchmark (Axis 2: output clarity 4.67/10, Axis 3: failure-mode clarity 2.17/10):

**Finding 1 — `--full` scope ambiguity.** Agents who misread `--full` as per-file write `8v read a.rs b.rs --full --full`, expecting full content for each file. Clap rejects this today with "the argument '--full' cannot be used multiple times" — 6/6 v3 benchmark runs hit this error. The instruction surfaces say only "entire file. Last resort." — they do not say the flag applies to all paths uniformly.

**Finding 2 — `=== <path> ===` delimiter undocumented.** Every multi-file `8v read` call produces headers like `=== src/lib.rs ===` before each file's output. Neither instruction surface (`ai_section.txt`, `instructions.txt`) mentions this format. Agents cannot predict it, cannot parse it, and cannot distinguish a batch response from N independent responses.

These are not missing features. The behavior is already correct. The gap is that the behavior is not described anywhere an agent can read before issuing the call.

---

## 2. Root Cause

**Finding 1.** `--full` is a clap boolean flag (`pub full: bool`, `#[arg(long)]`). Clap booleans reject repetition by design. The flag is passed identically to every `read_one()` invocation — its scope is architecturally global. The instructions say nothing about scope.

**Finding 2.** `ReadReport::Multi` renders each entry with `=== {label} ===\n` (line 119 of `o8v-core/src/render/read_report.rs`). The format has never appeared on either instruction surface. Agents encounter it only after issuing the call.

Both gaps are on the documentation layer, not the code layer.

---

## 3. Current Behavior

**Source: `o8v/src/commands/read.rs`**

```
pub struct Args {
    #[arg(required = true)]
    pub paths: Vec<String>,   // one or more, clap rejects zero
    #[arg(long)]
    pub full: bool,            // global boolean — applies to every path
}
```

- `8v read a.rs b.rs` → `ReadReport::Multi` with two entries
- `8v read a.rs b.rs --full` → same, but every entry is `ReadReport::Full`
- `8v read a.rs b.rs --full --full` → clap error before execution (current; §5b changes this)
- `8v read a.rs b.rs --json` → JSON; no `===` delimiters present

**Source: `o8v-core/src/render/read_report.rs`, lines 113–133**

Multi plain-text render:
```
(blank line, only between entries i > 0)
=== {relative_label} ===
{body lines}
```

Per-file errors render inline as `error: {message}` — the batch does not abort.

Single-path calls return the sub-report variant directly (no `Multi` wrapper, no `===` header).

---

## 4. Design Goals

1. **Close the scope gap.** Agents must know that `--full` applies to all paths before issuing the call, not after a clap error.
2. **Close the delimiter gap.** Agents must know the exact `=== <path> ===` format before issuing the call.
3. **No new behavior.** No new flags, no new output formats, no config. Phase 0 freeze is active.
4. **Parity.** Both instruction surfaces (`ai_section.txt` and `instructions.txt`) must receive identical updates at the same time.
5. **Minimal surface area.** Each update adds the minimum bytes required to answer the specific question an agent would ask. No restructuring.

---

## 5. Proposed Behavior

The decision points below govern documentation wording. Exception: §5b requires a one-line clap annotation change to make repeated `--full` a no-op instead of an error.

### 5a. Where to add the `--full` scope annotation

**Options:**
- A. Inline with the existing `--full` line on each surface ("entire file — applies to all paths in the call")
- B. As a new line directly below the existing `--full` line ("Note: `--full` applies to every path in the call.")
- C. In a new "Flags" subsection separate from the read examples

**Recommendation: A.** The scope annotation belongs on the same line as the flag. Agents read the flag description first; a separate note requires them to associate two lines correctly under time pressure. Inline is the lowest-friction option and adds the fewest bytes.

### 5b. Whether to error or silently accept repeated `--full`

**Options:**
- A. Document exact clap error: "passing `--full` twice produces: the argument '--full' cannot be used multiple times"
- B. Document the consequence without quoting clap: "passing `--full` more than once is an error"
- C. Accept repeated `--full` silently as a no-op (idempotent flag)

**Recommendation: C — accept repeated `--full` silently (no-op).** Three reasons:
1. 6/6 v3 benchmark runs hit the clap error because agents naturally write `a --full b --full` when inferring per-arg semantics from `:range` syntax. The error is never the right outcome — every agent who hits it wanted full output for all files.
2. Implementation is a clean clap idiom: `ArgAction::Count` with a `>= 1` check, or `SetTrue` + `.overrides_with_self(true)`. No hack, no compat risk.
3. Fails-open philosophy: forgiving CLIs beat strict ones when the repeated arg carries no contradictory meaning. `--full --full` means the same thing as `--full`; rejecting it is pure friction.

This decision requires a small code change (clap arg annotation only). It is the only §5 decision that touches the code layer.

### 5c. Where to add the `=== path ===` delimiter description

**Options:**
- A. Inline with the batch read example (`8v read a.rs b.rs Cargo.toml`)
- B. As a new line directly below the batch example
- C. In a new "Output format" subsection

**Recommendation: B.** The batch example teaches what to pass; the delimiter description teaches what to expect back. Keeping them adjacent (but on separate lines) preserves the progressive structure. A new subsection (C) adds structural weight that is disproportionate to one sentence.

### 5d. Whether to document `--json` suppressing delimiters

**Options:**
- A. Add a note on the `--full` or batch lines: "use `--json` for structured output without `===` headers"
- B. Document this only in a general `--json` section (not read-specific)
- C. Do not document the interaction explicitly

**Recommendation: A.** Agents who are parsing output will reach for `--json` as an alternative. If the delimiter behavior changes their parsing strategy, they need to know `--json` is the clean exit. One sentence inline with the delimiter description is sufficient.

### 5e. Whether to document single-path-no-`===` behavior

**Options:**
- A. Add: "single-path calls do not produce `===` headers"
- B. Leave implicit — the `===` description covers only the multi-path case
- C. Document with an example showing the difference

**Recommendation: A.** The asymmetry (single path → no header, multiple paths → headers) is the most likely source of agent confusion when a call returns unexpected output. One line stating the negative case explicitly removes the ambiguity. A full example (C) would add disproportionate length.

### 5f. Mixing `:range` + `--full` on the same argument

**Decision:** When an agent writes `8v read a.rs:1-20 b.rs --full`, what wins for `a.rs`?

Current code: `read_one` branches on `range` before checking `full` (lines 122–146 of `read.rs`). The range wins silently for that path; `--full` applies to every other path that has no range.

**Options:**
- A. Document existing behavior: `:range` wins for that arg; `--full` fills in for the remaining args
- B. Treat the combination as an error to avoid silent precedence
- C. Leave undocumented (out of scope)

**Recommendation: A.** The behavior is already correct and useful — per-arg `:range` overrides `--full` for that arg; `--full` fills in elsewhere. The only gap is that agents cannot predict this. Option B would break a valid use case. Option C continues the silent-precedence problem flagged in E1. Document the rule; do not change the code.

### 5g. Partial failure in a batch call

**Decision:** One file is missing in `8v read a.rs missing.rs b.rs`. What happens?

Current code: `MultiResult::Err` entries render inline as `error: {message}` under the `=== path ===` header for that entry. The batch does not abort. Exit code: the CLI exits non-zero if any entry errored.

**Options:**
- A. Surface per-file error inline, proceed with remaining files, exit non-zero
- B. Halt on first error, exit non-zero
- C. Surface per-file error inline, proceed, exit zero

**Recommendation: A.** Halting (B) discards results for valid files — an agent would need to split the call to recover. Exiting zero (C) hides the partial failure from CI and calling processes. Option A gives the agent maximum information and a correct signal. The current implementation already does this; the documentation gap is that agents cannot predict the non-zero exit or the inline error format.

---

## 6. Test Plan

All six tests must be written and confirmed failing on the current instruction surfaces before the doc update is applied. This is the failing-first gate.

**Layer:** Integration (instruction-surface layer, not CLI binary layer). Tests parse the text of both surfaces, not the behavior of the read command itself.

**Test T1 — `--full` scope annotation present on Surface 1**  
Input: contents of `o8v/src/init/ai_section.txt`  
Assert: the `--full` line contains the word "all" or the phrase "every path" or equivalent scope qualifier  
Fails today: the current text is "entire file. Last resort." — no scope qualifier

**Test T2 — `--full` scope annotation present on Surface 2**  
Input: contents of `o8v/src/mcp/instructions.txt`  
Same assertion as T1  
Fails today: same gap

**Test T3 — `--full` repeat-accepted annotation present on Surface 1**  
Input: contents of `o8v/src/init/ai_section.txt`  
Assert: the surface contains text indicating that repeating `--full` is accepted or a no-op (any form: "no-op", "silently", "Repeating", "accepted")  
Fails today: no such text exists

**Test T4 — `--full` repeat-accepted annotation present on Surface 2**  
Same assertion as T3 on `o8v/src/mcp/instructions.txt`  
Fails today: no such text exists

**Test `read_multi_full_flag_accepted` — CLI binary accepts repeated `--full`**  
Layer: CLI binary (not instruction-surface layer).  
Command: `8v read a.rs b.rs --full --full --full`  
Assert: exits 0; output contains the full body of both files separated by `=== ... ===` delimiter.  
Fails today: clap errors with "the argument '--full' cannot be used multiple times" and exits non-zero. This test MUST be run against pre-change code to confirm it fails before the §5b code change is applied.

**Test T5 — `=== path ===` delimiter documented on Surface 1**  
Input: contents of `o8v/src/init/ai_section.txt`  
Assert: the surface contains the literal text `===`  
Fails today: grep finds no `===` in the file

**Test T6 — `=== path ===` delimiter documented on Surface 2**  
Input: contents of `o8v/src/mcp/instructions.txt`  
Same assertion as T5  
Fails today: no `===` present

---

## 7. Doc Updates

Exact bytes to replace on both surfaces. The change is identical on both files; parity is enforced by replacing the same block.

**Current text (both surfaces, read section):**

```
`8v read <path> --full` — entire file. Last resort.
`8v read a.rs b.rs Cargo.toml` — batch any combination of paths and ranges in one call: distinct files, multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`), or a mix. One call beats N sequential calls.
```

**Replacement text:**

```
`8v read <path> --full` — entire file, applied to every path in the call. Last resort. Repeating `--full` is accepted silently (no-op).
`8v read a.rs b.rs Cargo.toml` — batch any combination of paths and ranges in one call: distinct files, multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`), or a mix. Multi-path output uses `=== <path> ===` headers before each file's content; single-path output has no header. Use `--json` for structured output without headers.
```

**Affected files:**

- `o8v/src/init/ai_section.txt` — lines containing `--full` and the batch example
- `o8v/src/mcp/instructions.txt` — same lines

No other files require changes under this design. `CLAUDE.md` and the parent `oss/CLAUDE.md` reproduce the same instruction text; they should be updated in the same commit for consistency, but they are not the authoritative source and do not require separate test coverage.

---

## 8. Edge Cases

**E1 — Range + `--full` on the same path.**  
`8v read a.rs:1-20 --full`. The `parse_path_range` parser extracts the range; `read_one` then branches on `range` first (lines 122–146 of `read.rs`), meaning the range wins over `--full` for that path. This is silent precedence — undocumented. Out of scope for this design (single finding, not compound). Log as a separate P1 for the next design pass.

**E2 — All paths error in a batch call.**  
`8v read missing.rs also_missing.rs`. Both return `MultiResult::Err`; the batch output is two `=== path ===` headers each followed by `error: ...`. The proposed delimiter documentation must not imply the body is always a symbol map or file content. The word "content" in the proposed text covers this implicitly, but reviewers should verify.

**E3 — Single-path call with `--full`.**  
Returns `ReadReport::Full` directly (no `Multi` wrapper). No `===` header. The proposed "single-path output has no header" annotation covers this correctly.

**E4 — `--json` with a batch call.**  
Returns a single JSON object serialized from `ReadReport::Multi`. The `===` headers do not appear. The proposed annotation "Use `--json` for structured output without headers" is accurate.

**E5 — Surface parity drift.**  
If `ai_section.txt` and `instructions.txt` are updated in separate commits, parity can drift. The test plan (T1–T6) enforces parity at the assertion level but does not enforce same-commit discipline. That is a process requirement, not a test requirement.

---

## 9. Acceptance Criteria

1. Tests T1–T6 fail on the current surfaces before the doc update is applied.
2. Tests T1–T6 pass after the doc update is applied.
3. Test `read_multi_full_flag_accepted` fails on pre-change code (clap rejects repeated `--full`).
4. Test `read_multi_full_flag_accepted` passes after the §5b code change is applied (exits 0, full output, delimiter present).
5. No other tests regress.
6. `ai_section.txt` and `instructions.txt` contain identical text for the affected lines.
7. The replacement text adds no new flags, commands, or behavioral promises beyond §5b's no-op acceptance.
8. `8v check .` passes (format + lint) after the file edits and code change.
9. The doc update and code change are committed together in a single atomic commit.

---

## 10. Risks and Review Gates

**Risk R1 — Wording locks in implementation detail.**  
The phrase "applied to every path in the call" is a behavioral description derived from the source. If the implementation changes (e.g., per-path `--full` is added in a future phase), this line becomes wrong. Mitigation: the text describes observable behavior, not the `pub full: bool` implementation. A future per-path flag would not invalidate "applies to every path" for the no-flag case — it would require a separate update.

**Risk R2 — `===` format changes.**  
If the delimiter changes (e.g., to `--- <path> ---`), the documented literal `===` becomes wrong. Mitigation: T5 and T6 search for the literal `===` — if the render code changes but the docs do not, the tests catch the drift in the other direction. The render code should be the source of truth, and docs must track it.

**Risk R3 — E1 (range + `--full` precedence) bleeds into this change.**  
Reviewers may ask about the silent range-wins behavior. This design explicitly does not address it. If a reviewer flags it as a blocker, it should be filed as a separate design, not folded in here.

**Review Gates:**

- Gate 1: Counterexample review. Reviewer must attempt to construct an agent prompt that, after reading the proposed text, still produces either bug. If a counterexample exists, the wording is insufficient.
- Gate 2: Parity check. Both surfaces must be diff-verified to contain the identical replacement text.
- Gate 3: Failing-first verification. Reviewer must confirm that T1–T6 were run against the pre-update surfaces and failed, with output attached to the PR.

---

## 8v Feedback

This design doc was produced by reading source files directly (read.rs, read_report.rs, ai_section.txt, instructions.txt). No tool friction on reads. One friction point: the user specified output path `docs/designs/` (plural) but the existing directory is `docs/design/` (singular). The `8v ls` + `8v read` flow did not surface this discrepancy — it required a manual `ls` to confirm. A future `8v write --path-check` that warns on write to a non-existent directory would have caught this at the moment of the write call rather than requiring a pre-flight check. Logged, not a blocker.
