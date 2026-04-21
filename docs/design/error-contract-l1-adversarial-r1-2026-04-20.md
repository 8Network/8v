# Adversarial Review — Round 1 — 2026-04-20

Reviewer role: adversary. Goal is to find what the author missed, not to validate.
Scope: six L1 design drafts. Context-only: error-contract.md, qa-sweep-register-2026-04-20.md.
Hard constraint: no source files touched. End with git status.

---

## §1 error-routing-decomposition.md

### Blockers

**B1.** "B2a first — lowest risk; every subsequent slice depends on stderr discipline." The dependency claim is stated but not argued. B2c (exit code unification) does not obviously depend on stderr discipline — a command can emit wrong exit codes while correctly routing to stderr. If this ordering constraint is wrong, the sequencing rationale collapses. Either prove the dependency or drop the claim.

**B2.** "B2d absorbs the capital-E follow-up doc." The capital-E doc contains an open A/B/C option selection: "Pick A, B, or C. Implementation agent only starts after founder picks." B2d absorption does not transfer or resolve that decision. The decision is now homeless — neither the capital-E doc nor the B2d description in this decomp records who owns the option choice or where the answer lives. Level 2 for B2d cannot start.

**B3.** No failing-first test is named for any of the four sub-slices. A decomposition that creates four new work units without specifying the entry-condition (failing test per slice) violates the failing-first rule. This is not a Level 2 concern — Level 1 must bound what "done" means per slice.

### Scope creep attempts

None detected. The decomposition is a structural decision, not a feature add.

### Contract drift

Not directly applicable — this doc restructures work, does not define behavior.

### Missing failing-first tests

See Blocker B3 above. All four sub-slices lack a named entry test.

### Untestable claims

"Every subsequent slice depends on stderr discipline." No observable behavior test can verify an ordering constraint between design docs.

### Mutation audit plan gaps

No mutations listed. Decomposition doc — acceptable if sub-slice docs own mutations. Verified: B2a-CE doc lists mutations. B2b, B2c, B2d slice docs not provided for review — gap cannot be confirmed closed.

---

## §2 stderr-channel-counterexamples.md

### Blockers

**B1.** A6 is in the gate list ("A2, A6, A7, A8 have concrete answers before Level 2 starts") but the body says: "Level 2 must trace this path." A gate requirement that points to Level 2 for its own resolution is not a gate — it is a deferred decision dressed as a gate. Either remove A6 from the gate list or provide the concrete answer in this doc.

**B2.** A2 resolution is stated in the CE doc: "B2a MUST preserve current stdout-on-json behavior for the path that will be formalized in B2b." This is labeled "Resolution needed" — not resolved. The decomp spec (error-routing-decomposition.md) does not carry this resolution either. A2 is the doc's own highest-priority open tension, and neither doc resolves it. Level 2 cannot start until A2 has a definitive answer, not a statement of constraint.

**B3.** A7 and A8 are listed in the gate as requiring concrete answers but neither is addressed in the resolution section. A7 and A8 are not even summarized in the doc as provided. The gate lists four requirements; the doc provides resolution for fewer than four. Gate is unmet on its own terms.

### Scope creep attempts

None detected. The CE doc correctly stays within B2a scope.

### Contract drift

A2's constraint ("preserve stdout-on-json behavior") is the right instinct but it creates a dependency on B2b's contract before B2b's contract is written. This is not drift from error-contract.md, but it is drift risk: if B2a ships before B2b defines the JSON contract, B2a's "preserve" clause has no stable anchor.

### Missing failing-first tests

No test names appear in this CE doc. A CE doc is not required to list tests, but A2's constraint ("B2a MUST preserve current stdout-on-json behavior") implies at least one test: "when --json is passed and an error occurs, stdout receives JSON, stderr is empty." That test does not appear in B2a's test list (this doc) or the decomp doc.

### Untestable claims

"A2: B2a MUST preserve current stdout-on-json behavior." If the current behavior is itself wrong (which B2b is meant to fix), "preserve current behavior" is a moving target. The claim is only testable if the current behavior is snapshotted — no snapshot or fixture is named.

### Mutation audit plan gaps

No mutations in this doc (CE doc, not slice spec — acceptable). Mutations belong in the slice spec. B2a slice spec is not provided for review — cannot confirm coverage.

---

## §3 search-silent-failure-l1.md

### Blockers

**B1.** Register §4 coverage map (qa-sweep-register-2026-04-20.md) claims B3 closes BR-39 (`8v search .` treats `.` as regex). B3 slice doc §4 explicitly lists BR-39 as out of scope: "BR-39 (dot-as-regex): regex engine behavior, not a silent failure — out of scope." One of these documents is wrong. The register is a shared reference; if it is wrong, every coverage claim built on it is suspect. Which document is authoritative, and what corrects the other?

**B2.** `files_skipped_by_reason` is a new JSON output field. The doc defines its schema but provides no backward-compatibility note. If any consumer (benchmark harness, CI parser, test fixture) parses `--json` output by field name today, adding a new field silently changes the contract. The doc must either: (a) declare this is a purely additive change with no current consumers, or (b) identify the consumers and confirm they tolerate unknown fields.

### Scope creep attempts

BR-39 (regex behavior) is correctly excluded. No scope creep detected.

### Contract drift

error-contract.md §2.5: "search special case: emit `error: permission denied: <path>` to stderr and exit 1." error-contract.md §7 CE-2 overrides: harvest, not fail-fast. B3 correctly follows §7. No drift.

CE-2 discriminant: B3 correctly implements `exit 1 + stderr empty` = clean no-match vs. `exit 1 + stderr non-empty` = partial failure. M5 regression test covers this. No drift.

### Missing failing-first tests

Tests T1–T6 are listed. Verify each exists as a failing test before code change — the doc does not confirm current binary behavior for each. T4 (`--limit 0` accepted silently) is the weakest: current behavior must be confirmed as silently accepting before T4 is written as failing. If current binary already errors on `--limit 0`, T4 is not a failing-first test.

### Untestable claims

"`files_skipped_by_reason` accurately reports all skip reasons." Completeness of a map is not directly testable — only specific reasons can be tested case by case. The doc lists three reasons (permission, binary, unreadable) but does not claim exhaustiveness. This is acceptable if the claim is not made. Confirm no "all reasons" claim exists in the doc.

### Mutation audit plan gaps

M5 (CE-2 discriminant) is the strongest mutation. M1–M4 are adequate. No gaps beyond the BR-39 coverage map contradiction (Blocker B1).

---

## §4 slice-c1-init-hooks-correctness.md

### Blockers

**B1.** H-1: "fail-closed default: treat malformed input as 'block' (exit 1). Empty input is a programming error and should exit 2." error-contract.md §2.1 exit code table: "2 = Invocation error — invalid flag, missing required argument (clap parse failure)." Empty stdin at runtime is not a clap parse failure. Assigning exit 2 to a runtime condition violates the error-contract. This is not a Level 2 detail — the exit code is a contract commitment made at Level 1.

**B2.** I-3 (hooks PATH anchor) is listed in scope but no failing-first test is named for it. The doc lists 6 tests; none covers PATH validation behavior. A scope item without a test entry condition means Level 2 has no anchor for "done."

**B3.** BR-28 (hooks `--json` rejected) appears in the register's C1 proposal description ("BR-28 partially covered by B2 but hooks is separate subsystem") but is not in C1's scope, not in C1's test list, and is not claimed by B2 either. BR-28 is orphaned. Either C1 claims it or the register's C1 proposal is inaccurate — either way, one document must be corrected before Level 2.

### Scope creep attempts

H-3 (Co-Authored-By stripping) is listed in scope. This is a correctness fix for an existing hook behavior. Not scope creep. Confirm it does not add a new flag or command (freeze rule: no new commands/flags since 2026-04-14).

### Contract drift

H-1 exit 2 for empty stdin: see Blocker B1. This is a direct contract drift — the doc assigns exit 2 to a condition the error-contract reserves for clap failures only.

### Missing failing-first tests

I-3 PATH anchor: no test. See Blocker B2.
H-3 Co-Authored-By stripping: one test listed (`hooks_co_authored_by_in_body_is_stripped`). Confirm this test currently passes on the pre-fix binary (i.e., current binary strips when it should not — otherwise the test is not failing-first).

### Untestable claims

"fail-closed default: treat malformed input as 'block'." "Malformed" is not defined. What is the test boundary between malformed and valid? UTF-8 error? Incomplete line? Null byte? Without a definition, the claim cannot be fully tested.

### Mutation audit plan gaps

No explicit mutation list in C1 doc. The doc lists tests but not their inversions. Missing: invert H-1 to pass malformed input and confirm exit is 1 (not 0, not 2). Invert I-2 skip message to use wrong filename and confirm test fails.

---

## §5 slice-c2-upgrade-contract.md

### Blockers

**B1.** "Either a new value (`'status':'current'`) or `upgraded: false` + `current: true`. Pick one in Level 2." The JSON shape of the upgrade response is a contract commitment, not an implementation choice. L1 decides what (including what the output looks like); L2 decides how (implementation). Deferring the shape to Level 2 means Level 2 is doing L1 work. Any consumer of `8v upgrade --json` is blocked until L2 resolves this. The shape must be decided in this doc.

**B2.** The doc mentions "field absent when errored" as a scope item but the test list does not include a test for this. A scope item without a test is an untested contract.

**B3.** Offline cache behavior: "default: no (fail)" — the exit code for this case is not specified. error-contract.md requires that every failure path have an assigned exit code. Is this exit 1 (runtime error) or exit 2 (invocation error)? The doc does not say.

### Scope creep attempts

None detected. C2 is correctly scoped to U-1 and U-2.

### Contract drift

U-2 "exit 0 on network failure": fixing this to exit 1 is correct per error-contract §2.1. No drift. Confirm the fix does not change the human-readable message format in a way that breaks any existing test fixture.

### Missing failing-first tests

"Field absent when errored" — no test. See Blocker B2.
"Offline / network failure → exit 1" — test named? Confirm it currently fails (current binary exits 0 on network failure per U-2).

### Untestable claims

"The JSON shape communicates upgrade success unambiguously." Ambiguity is subjective. Untestable as stated. Replace with a concrete structural assertion: "the field `upgraded` is present and boolean in all success paths."

### Mutation audit plan gaps

No mutation list. For U-1: invert the "already current" check to always emit `upgraded: true` — confirm test fails. For U-2: change exit code back to 0 on network error — confirm test fails. Neither is documented.

---

## §6 slice-c3-write-semantics.md

### Blockers

**B1.** CE-1 escape analysis is wrong. The doc states: "\\n in the shell call → \n after expansion → matches literal backslash-n." Shell processes one backslash layer. To deliver `\n` (backslash-n) to 8v's expander, the agent must pass `\\n` (double backslash) in the shell command, which shell reduces to `\n` before 8v sees it. If the agent wants 8v to match a literal backslash-n in file content, and 8v's expander converts `\n` → newline, the agent must pass `\\n` to get `\n` through to 8v. The doc's claimed escape sequence is off by one shell layer. This directly affects the fix for AF-4 — if the implementation is based on a wrong escape model, the fix will be wrong.

**B2.** AF-1 scope: "Decide in Level 2: update help text OR add a doc note." Test `write_help_text_for_force_describes_overwrite_semantics` asserts on help text. This test makes the L2 decision now (update help text, not doc note). Either remove the test from L1 scope or make the decision here. L1 cannot simultaneously defer a decision and write a test that presupposes one branch.

**B3.** BR-38 (symlink error text) is listed in the QA register's C3 proposal (§5) but is not in C3's scope, not in C3's test list, and no other slice claims it. BR-38 is orphaned from C3. Either C3 claims it or the register's C3 proposal must be corrected.

### Scope creep attempts

Line number `o8v/src/commands/write.rs:101` is referenced directly in the doc. Line numbers in design docs rot immediately when code changes. This is documentation scope creep into implementation detail — design docs must reference behavior, not source line numbers. Remove all line number citations.

### Contract drift

AF-4 fix (expand `\n` in `--find` argument): this is a behavioral change to the `--find` flag. error-contract.md does not address flag argument expansion. No drift from the contract. But: the expansion must be consistent with how `--replace` and positional content arguments handle `\n` — the doc does not confirm this consistency.

### Missing failing-first tests

I-3 equivalent is not present here, but: no test for "\\n in `--find` with current binary fails to match literal newline in file." This is the failing-first test for AF-4. Without it, we cannot confirm the bug exists as stated before patching.

### Mutation audit plan gaps

No mutation list. For AF-4: invert the `\n` expansion (do not expand) — confirm test fails. For AF-1: revert help text — confirm test fails. Neither is documented.

---

## §7 write-capital-e-prefix-superseded.md (findings/)

### Blockers

**B1.** "Pick A, B, or C. Implementation agent only starts after founder picks." No decision is recorded in this doc or in the B2d description in the decomp doc. The doc is in scope for review and the gate is blocked. This is not a Level 2 detail — it is an L1 architectural decision (where does prefix normalization happen: at source, at router, or in a shared wrapper). The decision must be recorded before B2d's Level 2 can begin.

**B2.** B2d absorbs this doc (per error-routing-decomposition.md) but neither doc records what happens to the A/B/C decision during absorption. Does B2d inherit the open decision? Is the capital-E doc deprecated? Who is authoritative? The handoff is undocumented. Level 2 for B2d cannot start with two conflicting or ambiguous parent docs.

### Scope creep attempts

Option C description: "strip prefix at source, let router add canonical prefix." If the router is not yet built (B2a–B2c are prerequisites), Option C depends on infrastructure that does not exist. This is not scope creep in the doc itself, but it is an ordering constraint that is not acknowledged.

### Contract drift

error-contract.md §3: single `error:` prefix per message. The capital-E bug produces double-prefix (`error: ERROR: <message>`). All three options fix the double-prefix — no drift. But: Option B (wrapper) risks introducing a different form of drift if the wrapper is applied inconsistently.

### Missing failing-first tests

M1 is a behavioral mutation (capitalize a message to `Error:` — confirm double-prefix appears). This correctly describes the current broken behavior and confirms the test would fail post-fix. Adequate.

### Untestable claims

M3: "Capitalize a different message to `ERROR:` — guard still single-prefixes? Measure." "Measure" is not a test outcome. This is an open question masquerading as a mutation. Either convert to a deterministic assertion ("guard still single-prefixes → test passes") or remove it from the mutation plan.

### Mutation audit plan gaps

M3 is not a mutation plan item (see Untestable claims above). M1 and M2 are adequate for the scope. M3 must either be sharpened or removed.

---

## §8 Cross-Draft Analysis

### Inter-draft conflicts

**XD-1 (B2a spec vs. JSON mode).** B2a's decomp description says "move human-formatted errors to stderr." B2a-CE doc A2 says B2a must preserve stdout-on-json behavior. The B2a slice spec (not the CE doc) does not mention the JSON mode exception. A reader of the slice spec alone would implement stderr routing for all error paths including --json. The exception is only in the CE doc. One of these must be the authoritative behavioral spec, and it must be complete on its own.

**XD-2 (C1 exit-2 vs. error-contract §2.1).** C1 assigns exit 2 to empty stdin (runtime condition). error-contract.md §2.1 reserves exit 2 for clap parse failures. Direct conflict. C1 must resolve this before Level 2.

**XD-3 (B3 scope vs. register coverage map on BR-39).** B3 out-of-scope list excludes BR-39. Register §4 claims B3 closes BR-39. One is wrong. If the register is authoritative, B3 must claim BR-39 or a new slice must. If the B3 slice doc is authoritative, the register must be corrected. The register being wrong means every coverage gap analysis built on it is suspect.

**XD-4 (B2d absorption of capital-E without decision transfer).** B2d absorbs the capital-E follow-up doc (per decomp doc). The capital-E doc has an open A/B/C decision. B2d does not carry the decision or reference the open gate. The capital-E doc and B2d are now in conflict: one says "pick A/B/C before starting," the other implies the work is absorbed and ready for Level 2.

**XD-5 (C2 shape decision vs. L1 responsibility).** C2 defers the JSON output shape to Level 2. Every other slice doc (B3, C1, C3) defines its output shape at L1. C2 is inconsistent with its siblings and with the two-level design rule.

### Ordering constraints

**OC-1.** "B2a → B2c → B2b → B2d" recommended in decomp doc. B2c depends on stderr discipline (claimed but not argued — see decomp Blocker B1). If B2c can ship independently of B2a, the ordering is wrong and slices can be parallelized. This matters for throughput.

**OC-2.** Capital-E follow-up Option C requires a router (from B2a/B2c). If Option C is chosen, the capital-E fix cannot ship before B2a and B2c are complete. This ordering constraint is not documented in either the capital-E doc or the decomp doc.

**OC-3.** C1's H-1 (malformed stdin → exit 2) conflicts with error-contract. If error-contract is amended to allow exit 2 for this case, the amendment must happen before C1's Level 2 — not during. C1 cannot ship with an unresolved contract conflict.

---

## §9 Gate Recommendations

| Draft | Gate | Required before Level 2 |
|---|---|---|
| error-routing-decomposition.md | REVISE | Prove B2a→B2c dependency or drop claim. Document B2d/capital-E option handoff. Name one failing-first test per sub-slice. |
| stderr-channel-counterexamples.md | REVISE | Resolve A2 concretely (not "resolution needed"). Provide concrete answers for A6, A7, A8 or remove them from gate list. Add --json/stderr test. |
| search-silent-failure-l1.md | REVISE | Resolve BR-39 conflict between register and slice doc. Add backward-compat note for `files_skipped_by_reason`. |
| slice-c1-init-hooks-correctness.md | REVISE | Fix H-1 exit code (exit 1, not 2, for runtime empty-stdin). Add failing-first test for I-3. Resolve BR-28 orphan. |
| slice-c2-upgrade-contract.md | REVISE | Decide JSON shape at L1 (not deferred). Add test for "field absent when errored." Specify exit code for offline failure. |
| slice-c3-write-semantics.md | REVISE | Fix CE-1 escape depth analysis. Resolve AF-1 L2 decision conflict with test. Resolve BR-38 orphan. Remove line number citations. |
| write-capital-e-prefix-superseded.md (findings/) | REVISE | Record A/B/C decision. Document handoff into B2d. Convert M3 from open question to deterministic assertion. |

**READY: 0. REVISE: 7. REJECT: 0.**

---

## §10 8v Feedback (Dogfood)

Friction observed during this review session:

1. **Deferred tool schema is the single largest friction point.** Before any file read, `ToolSearch` with `select:mcp__8v-debug__8v` was required. The agent cannot call 8v until this step completes. Every session cold-starts with this overhead. The schema should be eagerly loaded or the deferred-tool mechanism should be opt-in per tool, not opt-out.

2. **`8v read` batch call with 9 files in parallel worked correctly.** No friction. Batch reads across different crate directories and docs directories resolved without path errors. This is the intended usage pattern working as designed.

3. **No `8v search` was needed for this review** (all file paths were provided upfront). In a discovery session this would be the first command; the review format bypassed it. No feedback on search in this session.

4. **No `8v write` was needed.** Output file created with Write tool per guardrail (no source files). No feedback on write in this session.

---

## Summary Counts

| Draft | Blockers | Gate |
|---|---|---|
| error-routing-decomposition.md | 3 | REVISE |
| stderr-channel-counterexamples.md | 3 | REVISE |
| search-silent-failure-l1.md | 2 | REVISE |
| slice-c1-init-hooks-correctness.md | 3 | REVISE |
| slice-c2-upgrade-contract.md | 3 | REVISE |
| slice-c3-write-semantics.md | 3 | REVISE |
| write-capital-e-prefix-superseded.md (findings/) | 2 | REVISE |

**Total blockers: 19. READY: 0. REVISE: 7. REJECT: 0.**

All seven drafts have unresolved blockers. No draft is ready for Level 2.
