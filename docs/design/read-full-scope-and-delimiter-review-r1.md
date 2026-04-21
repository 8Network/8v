# Adversarial Review R1: `read-full-scope-and-delimiter.md`

**Reviewer:** Claude (Sonnet 4.6)  
**Date:** 2026-04-19  
**Design doc:** `docs/design/read-full-scope-and-delimiter.md`  
**Status:** Reject — two blockers must be resolved before implementation can proceed  

Ground-truth sources read:
- `o8v/src/commands/read.rs` (full)
- `o8v-core/src/render/read_report.rs` (full)
- `o8v/src/init/ai_section.txt` (full)
- `o8v/src/mcp/instructions.txt` (full)
- `o8v/tests/e2e_cli.rs` (full)

---

## Finding Counts

| Category    | Count |
|-------------|-------|
| Blocker     | 2     |
| Design gap  | 5     |
| Test gap    | 4     |
| Risk        | 4     |
| Style nit   | 2     |
| **Total**   | **17** |

---

## Blockers

### B1 — §5f and §8 E1 directly contradict each other

**[Blocker]**

§5f (§5 decisions, "Mixing `:range` + `--full` on the same argument") makes a recommendation:

> Option A — Document existing behavior: `:range` wins for that arg; `--full` fills in for the remaining args

§5f's conclusion:

> "Document the rule; do not change the code."

§8 E1 ("Range + `--full` on the same path") says:

> "Out of scope for this design (single finding, not compound). Log as a separate P1 for the next design pass."

Both statements refer to the same behavior (`a.rs:1-20 b.rs --full` → range wins for `a.rs`, `--full` applies to `b.rs`). §5f says document it in this design. §8 E1 says it is out of scope.

An implementer following this doc cannot tell whether to include the `:range + --full` interaction note in §7's replacement text. §7 does not include it — which implies §8 E1 won. But §5f's recommendation column says "Document the rule." Both are authoritative sections of the same doc.

**Resolution required:** One section must be struck or reconciled. Either add the `:range + --full` precedence note to §7's replacement bytes (matching §5f's recommendation) or change §5f's recommendation to "out of scope" (matching §8 E1). The current state is internally inconsistent and blocks safe implementation.

---

### B2 — §5f decision has no artifact in §7

**[Blocker]**

Even if the reader interprets §5f's recommendation ("Document the rule") as in-scope, §7 contains no implementation of that decision. The replacement text in §7:

```
`8v read a.rs b.rs Cargo.toml` — batch any combination of paths and ranges in one call: distinct files, multiple ranges of the same file (`a.rs:1-200 a.rs:200-400`), or a mix. Multi-path output uses `=== <path> ===` headers before each file's content; single-path output has no header. Use `--json` for structured output without headers.
```

There is no mention of `:range` overriding `--full` for that arg, nor of `--full` filling in for the remaining args. §7 is the only section that contains "exact bytes to replace on both surfaces." If §5f's decision is in-scope, §7 is incomplete. If §5f's decision is out of scope, §5f's recommendation text is wrong.

This is a direct consequence of B1 — they are two facets of the same contradiction — but they are separate blockers because B2 would still exist even if B1 were resolved by clarifying scope: fixing B1 without updating §7 still leaves the doc without an implementation artifact for the §5f decision.

**Resolution required:** Either add the `:range + --full` precedence note to §7's replacement bytes, or change §5f recommendation to "C — leave undocumented (out of scope)" and add a rationale.

---

## Design Gaps

### DG1 — `===` header includes `:N-M` range suffix; §7 shows `=== <path> ===`

**[Design gap]**

The actual delimiter produced by `read_report.rs` line 119:

```rust
output.push_str(&format!("=== {} ===\n", entry.label));
```

`entry.label` is set in `read_to_report` (multi-path path, lines 222–225 of `read.rs`):

```rust
let display_label = match abs_canonical.strip_prefix(workspace.as_path()) {
    Ok(rel) => format!("{}{}", rel.to_string_lossy(), range_suffix),
    Err(_) => label.clone(),
};
```

where `range_suffix` is `":N-M"` when the arg includes a range. For `8v read a.rs:1-20 b.rs`, the first header is `=== a.rs:1-20 ===`, not `=== a.rs ===`.

§7's replacement text uses `=== <path> ===` as the format description — which implies the header contains only the path, not the range suffix. An agent reading this documentation and then seeing `=== a.rs:1-20 ===` will find it inconsistent with the documented format.

**Resolution:** Change the format in §7 to `=== <path>[:range] ===` or `=== <label> ===` where `<label>` includes any `:N-M` suffix. Test T5/T6 verify only that `===` is present; they would not catch this misrepresentation. A test verifying the full header string including range suffix is needed (see TG2 below).

---

### DG2 — §7 rationale dismisses CLAUDE.md as "not authoritative"; this misrepresents operational reality

**[Design gap]**

§7 says:

> "`CLAUDE.md` and the parent `oss/CLAUDE.md` reproduce the same instruction text; they should be updated in the same commit for consistency, but they are not the authoritative source and do not require separate test coverage."

In practice, `CLAUDE.md` is injected into the agent's context at the start of every session. It is the first and often only surface agents read. Calling it "not authoritative" understates its operational weight. If `CLAUDE.md` is updated in the same commit (as recommended) it should be treated as a parity-required surface, not an optional "for consistency" update.

The design doc's test plan (T1–T6) tests only `ai_section.txt` and `instructions.txt`. No test verifies that `CLAUDE.md` received the identical update. Given that `CLAUDE.md` is read first by agents in practice, this coverage gap is a reliability risk.

**Resolution:** Either (a) add T7/T8 testing CLAUDE.md, or (b) explicitly state that CLAUDE.md is derived from the canonical surfaces and the commit discipline is the enforcement mechanism — and acknowledge the trade-off.

---

### DG3 — §5b's clap change may already be a no-op

**[Design gap]**

§5b recommends "accept repeated `--full` silently as a no-op" and states the implementation is "a clean clap idiom: `ArgAction::Count` with a `>= 1` check, or `SetTrue` + `.overrides_with_self(true)`."

The current clap annotation is `#[arg(long)]` with `pub full: bool`. Clap's default for a `bool` flag is `ArgAction::SetTrue`. According to clap documentation, `SetTrue` with `overrides_with_self = false` (the default) does reject `--full --full`. So the change is not a no-op today — §5b's code change is real.

However, §5b's rationale says "Implementation is a clean clap idiom" — it does not verify that the current behavior is actually the error. The benchmark finding says "6/6 v3 runs hit the clap error" but the doc does not cite the exact clap version or confirm the annotation in the current `read.rs`. The existing `read.rs` confirms `pub full: bool` with `#[arg(long)]` and no `overrides_with_self` annotation — so the error is real today.

The gap: §5b presents two implementation options (`ArgAction::Count` vs `SetTrue + overrides_with_self`) but does not specify which is recommended. Both work but have different semantics: `Count` makes `full` an integer (requiring a cast), while `overrides_with_self(true)` keeps it a `bool`. For a flag that is always boolean in meaning, `overrides_with_self(true)` is the correct choice. The doc leaves this ambiguous.

**Resolution:** Pick one implementation option in §5b and state why. Both options are not equivalent from a type-safety standpoint.

---

### DG4 — Exit code semantics for batch partial failure not described in §7

**[Design gap]**

§5g correctly identifies that the CLI exits non-zero if any entry errors. §5g's recommendation (A) is correct and the current implementation matches. But §7's replacement text does not document exit code behavior.

An agent parsing the batch output needs to know: if I get a non-zero exit code, does that mean all files failed, or could it mean one file failed while others succeeded? The inline `error: {message}` format (documented in §3) is what distinguishes partial from total failure. Neither the current nor proposed instruction text tells agents about exit codes in the batch case.

**Resolution:** Add one sentence to §7's replacement text covering the exit-code semantics for partial batch failure, or explicitly defer it to a follow-up design (and note the deferral in §8 or §9).

---

### DG5 — `instructions.txt` already has parity drift with `ai_section.txt`; this is not acknowledged

**[Design gap]**

Ground-truth check: `ai_section.txt` ends its batch example line with "One call beats N sequential calls." `instructions.txt` omits this phrase entirely. This drift exists today, before this design is implemented.

The design doc commits to parity (§4 goal 4: "Both instruction surfaces must receive identical updates at the same time") and §9 acceptance criterion 6: "ai_section.txt and instructions.txt contain identical text for the affected lines."

But the doc's §7 replacement text only covers the `--full` line and the batch example. If the replacement is applied verbatim to both files, the "One call beats N sequential calls." sentence present in `ai_section.txt` but absent from `instructions.txt` will still differ on the lines preceding the replacement range.

**Resolution:** §7 must include the full replacement context, not just the new text, or explicitly identify the surrounding lines that must match. The "exact bytes" framing suggests strict replacement — but the surrounding context mismatch means the pre/post state of the two files will still differ.

---

## Test Gaps

### TG1 — T5/T6 assertions are too broad; a stray `===` anywhere passes the test

**[Test gap]**

T5 assertion:

> "Assert: the surface contains the literal text `===`"

Any line in `ai_section.txt` or `instructions.txt` containing `===` for any reason (a code example, a separator, a comment) would satisfy this test. The test does not verify that `===` appears in the correct context (the read section, the batch example, describing the output delimiter).

This is particularly risky because `ai_section.txt` today contains lines with `--` in code examples. If a future reviewer adds `===` to a code example in a different section, T5/T6 continue passing while the batch delimiter remains undocumented.

**Resolution:** Tighten the assertion to check that `===` appears within N lines of the batch example or on a line that also contains "batch" or "Multi-path" or similar contextual anchor.

---

### TG2 — No test verifies the `=== label ===` header includes the `:N-M` range suffix

**[Test gap]**

The `read_report.rs` delimiter uses `entry.label` which includes the `:N-M` suffix for range reads. No existing test in `e2e_cli.rs` verifies this. The existing tests `read_absolute_path_renders_relative_in_header` and `read_range_absolute_path_renders_relative_in_header` test the F1 regression (absolute path → relative display) but do not test the delimiter content in batch mode.

A test that reads two files with one as a range (`8v read a.rs:1-5 b.rs`) and asserts the output contains `=== a.rs:1-5 ===` would catch any future regression where the range suffix is stripped from the delimiter.

**Resolution:** Add a test in the test plan (or reference existing tests that cover this). The current test plan has no such test.

---

### TG3 — `read_batch_partial_errors_exits_nonzero` is fragile; uses relative path without workspace

**[Test gap]**

`e2e_cli.rs` line 139: `read_batch_partial_errors_exits_nonzero` uses `"Cargo.toml"` as a valid-file argument without `bin_in()` setting the working directory. The test's correctness depends on the CWD at test time. If `cargo test` runs from a directory that does not contain `Cargo.toml`, both files will error and the test passes for the wrong reason (all-errors case, not partial-error case).

This is a pre-existing bug, not introduced by this design, but the design doc's test plan includes `read_batch_all_errors_exits_nonzero` as an acceptance criterion without acknowledging this fragility.

**Resolution:** Flag this existing test as fragile in the test plan; specify that `read_batch_partial_errors_exits_nonzero` must use `bin_in()` with an explicit fixture directory or a temp-dir setup similar to `read_absolute_path_renders_relative_in_header`.

---

### TG4 — `read_batch_all_errors_exits_nonzero` may exit non-zero for the wrong reason

**[Test gap]**

`e2e_cli.rs` line 113: `read_batch_all_errors_exits_nonzero` uses `bin()` without workspace setup (no `setup_project`, no temp dir, no `8v init`). If `WorkspaceRoot` resolution fails before the read command executes, the process exits non-zero at workspace initialization, not at the batch error-propagation path.

The test comment says it tests "Bug #20 batch exit code" — but if the exit is from workspace init failure rather than from `MultiResult::Err` propagation, the test never exercises the code path it claims to cover. The bug fix may still be untested.

**Resolution:** The test plan for this design should specify that `read_batch_all_errors_exits_nonzero` must be rewritten with proper workspace setup (using `setup_project` + `bin_in()`) so the non-zero exit is demonstrably from the batch error path.

---

## Risks

### R1 — Path containing `===` produces ambiguous delimiter

**[Risk]**

A file at a path like `src/===separator===.rs` would produce:

```
=== src/===separator===.rs ===
```

A downstream parser splitting on `=== ... ===` would misidentify the inner `===` as section boundaries. The design doc's §10 Risk R2 acknowledges that the delimiter format could change, but does not acknowledge that the current delimiter is already ambiguous for paths containing `===`.

This is unlikely in practice (most paths do not contain `===`) but is a real failure mode for any agent that builds a regex parser on top of the batch output.

---

### R2 — File content lines containing `===` are ambiguous in batch output

**[Risk]**

If a file's content contains a line like `=== section header ===`, the batch output:

```
=== src/foo.rs ===
... some content ...
=== section header ===
... more content ...
=== src/bar.rs ===
```

is ambiguous to a line-by-line parser. A parser splitting on `^=== .+ ===$` cannot distinguish file-level delimiters from content lines that happen to match the delimiter pattern.

The design doc's E2 ("All paths error") acknowledges inline errors but does not acknowledge that content can match the delimiter. The proposed documentation ("Multi-path output uses `=== <path> ===` headers") does not warn agents about this ambiguity. Adding a note that `--json` avoids this problem entirely would be more actionable than the current generic "Use `--json` for structured output without headers."

---

### R3 — T3/T4 keyword assertions are too narrow and could be satisfied trivially

**[Risk]**

T3/T4 assert that the surface contains one of: "no-op", "silently", "Repeating", "accepted". A line like "Errors are accepted silently" in a different section would satisfy the assertion without the `--full` repeat behavior being documented.

The same keyword `silently` appears in the proposed replacement text for a different purpose. If the order of additions to the file is wrong (e.g., the parity-drift text "silently" from another section is present but the `--full` line is missing), T3/T4 pass while the actual coverage is absent.

---

### R4 — `instructions.txt` parity drift already exists today; no test enforces same-commit discipline

**[Risk]**

As noted in DG5, `instructions.txt` already differs from `ai_section.txt` on the "One call beats N sequential calls." sentence. The existing T5/T6 tests in the current design verify only the proposed new text. No test enforces that the two files are identical on the lines not covered by this design.

§8 E5 acknowledges that "separate commits" can cause drift, and says "that is a process requirement, not a test requirement." This is an explicit acceptance of drift risk. The design should acknowledge that this acceptance is a deliberate trade-off, and should specify the process gate (e.g., "PR description must include a diff of both files side by side").

---

## Style Nits

### SN1 — §7 replacement bytes do not specify encoding

**[Style nit]**

§7 says "exact bytes to replace on both surfaces" but does not specify encoding. UTF-8 is assumed but not stated. Not a blocker, but the "exact bytes" framing implies precision that the doc does not deliver without an encoding declaration.

---

### SN2 — `entry.label` vs "path" naming inconsistency

**[Style nit]**

In `read_report.rs`, the struct field is `MultiEntry { label: String, ... }`. In the design doc's delimiter description, the placeholder is `=== <path> ===`. The code calls it `label`; the doc calls it `path`. The two are not synonymous: `label` includes the `:N-M` suffix, `path` implies a pure filesystem path. This naming inconsistency is a source of confusion for implementers reading both the doc and the source.

---

## Counterexample Verification for T1–T6

Verified against current source (ground-truth reads completed):

| Test | Assertion | Current state | Genuinely fails today? |
|------|-----------|---------------|------------------------|
| T1 | `ai_section.txt --full` line contains "all" or "every path" | Line is "entire file. Last resort." | YES |
| T2 | `instructions.txt --full` line contains scope qualifier | Same text | YES |
| T3 | `ai_section.txt` contains "no-op" / "silently" / "Repeating" / "accepted" | No such text | YES |
| T4 | `instructions.txt` contains same | No such text | YES |
| T5 | `ai_section.txt` contains literal `===` | Not present | YES |
| T6 | `instructions.txt` contains literal `===` | Not present | YES |

All six tests genuinely fail today. The failing-first gate is satisfiable.

---

## Edge Case Adversarial Walk

### §5a edge case — scope annotation inline vs. adjacent

The inline option (A) appends scope text to the existing `--full` line. Current `ai_section.txt` line (bullet format):
```
- `8v read <path> --full` — entire file. Last resort.
```

After §7 replacement:
```
`8v read <path> --full` — entire file, applied to every path in the call. Last resort. Repeating `--full` is accepted silently (no-op).
```

Note: `ai_section.txt` uses `- ` prefix (bullet list); `instructions.txt` uses no prefix. The §7 replacement text shows no `- ` prefix. If applied literally to `ai_section.txt`, it would silently remove the bullet formatting. §7 says "exact bytes" but does not show the full line including the bullet character.

**Surprising output:** After the replacement, `ai_section.txt` would have a formatting inconsistency — one line without a bullet in a bullet-list section. This is a concrete implementation error waiting to happen.

### §5b edge case — `ArgAction::Count` cast path

If the implementer chooses `ArgAction::Count` (one of the two options in §5b), `pub full: bool` must become `pub full: u8` (or similar). This is not a doc-only change — it requires updating callers of `args.full` in `read.rs`. The doc says "a small code change (clap arg annotation only)" but `ArgAction::Count` is not annotation-only. `overrides_with_self(true)` is annotation-only and keeps the bool type. The doc must not present both as equivalent annotation-only changes.

### §5c edge case — delimiter description placement

§5c recommends placing the delimiter description "directly below the batch example" (option B). The batch example is currently the last line in the `## Read` section on both surfaces. "Directly below" means appending, not inserting. The §7 replacement shows the delimiter note appended to the same batch-example line (not as a new line below). This contradicts §5c's recommendation of "adjacent (but on separate lines)."

**Surprising output:** An implementer following §5c would put the delimiter description on a new line after the batch example. An implementer following §7's exact replacement bytes would inline it. These produce different outputs. §5c and §7 disagree.

### §5d edge case — `--json` suppresses delimiters

The proposed text says "Use `--json` for structured output without headers." This is accurate for the plain-text render path. However, `--json` also changes the entire output structure (from multi-line text to a single JSON object). Telling agents to "use `--json` for structured output" may prompt them to use JSON when they do not need structured output — just because they want to avoid parsing `===` delimiters. A tighter statement would be: "Add `--json` to get a single JSON object without headers."

### §5e edge case — single-path no-header rule

The proposed text says "single-path output has no header." An agent parsing this rule correctly would expect: `8v read src/main.rs` → no `===` header, just content or symbol map. Edge case: what if the agent passes one path that expands to an error? The output is `error: ...` with no header (since single-path returns `ReadReport::Err` as a direct `Err(String)` from `read_one`, rendered before the `Multi` wrapper). This is consistent with "single-path output has no header" — but the error format is not documented anywhere on either surface.

### §5g edge case — exit code documentation

§5g says to document the exit-code behavior for partial failure. §7's replacement text does not include this. As noted in DG4, this decision has no implementation artifact in §7.

---

## Most Important Finding

**B1 (§5f vs §8 E1 contradiction).** This is the most important finding because it is an internal contradiction that makes the doc impossible to implement correctly without author clarification. Every other finding is a gap or risk that an implementer can navigate with judgment. B1 requires the author to decide: does this design document the `:range + --full` precedence rule or not? Until that decision is recorded, the implementer cannot know which version of the doc is normative.

---

## Recommendation

**Reject — approve with changes after author resolves B1 and B2.**

B1 and B2 are not implementation risks — they are design-doc inconsistencies that block safe implementation. Once B1 is resolved (§5f and §8 E1 reconciled) and B2 is resolved (§7 updated to match), a second review pass can clear the design gaps and test gaps, most of which are refinements rather than blockers.

Suggested priority order for the revision:
1. Resolve B1/B2 (§5f/§8 E1 reconcile + §7 update).
2. Fix §7 to include bullet prefixes for `ai_section.txt` (from §5a edge case).
3. Clarify §5b implementation choice (DG3): pick `overrides_with_self(true)`, not `ArgAction::Count`.
4. Fix DG1: change `=== <path> ===` to `=== <label> ===` (or `=== <path>[:range] ===`) throughout.
5. Address TG1: tighten T5/T6 assertions to require contextual proximity.
6. Address DG5: specify how the pre-existing `ai_section.txt` / `instructions.txt` drift will be fixed in the same commit.

Items 4–6 and remaining findings (DG4, TG2–TG4, R1–R4, SN1–SN2) can be addressed in a single revision pass without a new review round if the author agrees with the findings.

---

## 8v Dogfood Friction

Batch-reading `read.rs`, `read_report.rs`, `ai_section.txt`, `instructions.txt`, and `e2e_cli.rs` as five separate calls (each time the session resumed after compaction) rather than one batched call was the main friction point. After two compactions, the MCP session state was reset, so the batch context was not preserved across turns. The `8v read a.rs b.rs c.rs` batching principle saves turns within a session, but across compaction boundaries each file must be re-read individually — there is no "re-read last batch" recovery path. This is a session-continuity friction, not a command-design friction.
