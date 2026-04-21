# QA Audit: 8v Verify Commands (check, fmt, test, build)

**Date:** 2026-04-20
**8v version:** 0.1.0 (binary `~/.8v/bin/8v`, commit caa7736 dirty)
**Scope:** All four verification commands × all project states × all output forms
**Constraint:** No code changes. All scratch work in `/tmp/`. 8v repo git state unchanged.

---

## 1. Form × Project-State Matrix

### Legend
- PASS — command produces correct output and exit code
- FAIL — wrong output or wrong exit code
- PARTIAL — output is present but incomplete or misleading
- N/A — not applicable

### Fixtures used

| Fixture | Path | Description |
|---|---|---|
| clean-8v | `oss/8v/` | 8v repo itself (19 pre-existing modifications, not QA-introduced) |
| compile-error | `/tmp/rust-qa-compile-error/` | `let x: i32 = "this is not a number";` |
| clippy-warn | `/tmp/rust-qa-clippy/` | `v.iter().map(\|x\| x+1).collect::<Vec<_>>()` without use |
| fmt-dirty | `/tmp/rust-qa-fmt/` | `fn main(){let x=42;println!("{}",x);}` |
| failing-test | `/tmp/rust-qa-failing-test/` | 2 tests: 1 passes, 1 asserts `add(1,2) == 99` |
| empty-dir | `/tmp/empty-test-qa/` | completely empty directory |
| markdown-only | `/tmp/markdown-only-qa/` | README.md + notes.md only |
| polyglot | `o8v-testkit/tests/fixtures/polyglot-violated/` | rust + python + dockerfile stacks |

---

### `8v check`

| Project state | plain | --json | --help |
|---|---|---|---|
| clean-8v | PASS (exits 0) | PASS | PASS |
| compile-error | PARTIAL (Bug 5, Bug 6) | PARTIAL (Bug 5, Bug 6) | PASS |
| clippy-warn | PASS | PASS | PASS |
| fmt-dirty | PASS (fmt not a check concern) | PASS | PASS |
| failing-test | PASS (reports test file issues) | PASS | PASS |
| empty-dir | PASS (exits 2, "no project detected") | PASS | PASS |
| markdown-only | PASS (exits 2, "no project detected") | PASS | PASS |
| polyglot | FAIL — all rust checks fail with workspace collision error | FAIL | PASS |

**Reproducible commands:**
```bash
# compile-error
8v check /tmp/rust-qa-compile-error

# polyglot workspace collision
8v check /Users/soheilalizadeh/8/products/vast/oss/8v/o8v-testkit/tests/fixtures/polyglot-violated
```

---

### `8v fmt`

| Project state | plain | --json | --check (plain) | --check --json | --help |
|---|---|---|---|---|---|
| clean (after fmt) | PARTIAL (Bug 4) | PARTIAL (Bug 4, Bug 7) | PARTIAL (Bug 4) | PARTIAL (Bug 4, Bug 7) | PASS |
| fmt-dirty | PARTIAL (Bug 3) | PARTIAL (Bug 3, Bug 7) | PASS (exits 1) | PASS (exits 1) | PASS |
| empty-dir | PASS (exits 2) | PASS | PASS | PASS | PASS |
| markdown-only | PASS (exits 2) | PASS | PASS | PASS | PASS |

**Reproducible commands:**
```bash
# dirty file — grammar bug
8v fmt /tmp/rust-qa-fmt

# clean file — misleading message
cp /tmp/rust-qa-fmt-clean.rs /tmp/rust-qa-fmt/src/main.rs  # restore clean
8v fmt /tmp/rust-qa-fmt

# check mode on clean file — misleading message
8v fmt /tmp/rust-qa-fmt --check
```

---

### `8v test`

| Project state | plain | --json | --help |
|---|---|---|---|
| clean-8v | PARTIAL (4 MCP e2e tests fail due to O8V_MCP_OUTPUT_CAP=5 env) | PARTIAL | PASS |
| compile-error | PASS (exits nonzero, reports compilation failure) | PARTIAL (Bug 2, passed/failed are null not 0) | PASS |
| failing-test | PASS (exits nonzero, reports failure) | PARTIAL (Bug 2) | PASS |
| empty-dir | PASS (exits 1, "no project detected") | PASS | PASS |
| markdown-only | PASS (exits 1, "no project detected") | PASS | PASS |

**Reproducible commands:**
```bash
# failing test — location.type bug
8v test /tmp/rust-qa-failing-test --json | python3 -c "import json,sys; d=json.load(sys.stdin); [print(e) for e in d.get('errors',[])]"

# compile error — null counts
8v test /tmp/rust-qa-compile-error --json | python3 -c "import json,sys; d=json.load(sys.stdin); print('passed:', d.get('passed'), 'failed:', d.get('failed'))"
```

---

### `8v build`

| Project state | plain | --json | --help |
|---|---|---|---|
| clean (8v repo) | PASS | PASS | PASS |
| compile-error | PASS (exits 1) | FAIL (Bug 1 — exits 0) | PASS |
| empty-dir | PASS (exits 1) | PASS | PASS |
| markdown-only | PASS (exits 1) | PASS | PASS |

**Reproducible commands:**
```bash
# exit code bug
8v build /tmp/rust-qa-compile-error --json; echo "exit: $?"
# Expected: exit 1
# Actual:   exit 0
```

---

## 2. Pass-Through vs. Structured Output Analysis

### `check`
- **Human output:** Structured. File paths, line numbers, rule IDs, severity colors. Not raw tool output.
- **JSON output:** Deeply structured. `results[] → checks[] → diagnostics[]` with full location/span/rule/severity/suggestions. Machine-applicable fix edits included when available.
- **Assessment:** Best JSON shape of the four commands. Delta tracking (`new/fixed/unchanged`) adds value. Two issues: duplicate diagnostics (Bug 6) and phantom empty-path entries (Bug 5).

### `fmt`
- **Human output:** Pass-through summary only. Reports "N stacks formatted/dirty" but no file-level detail.
- **JSON output:** `{stacks: [{name, status, timing_ms, tool}]}` — stack-level summary only. No file paths. No diff. Not actionable for agents.
- **Assessment:** Weakest structured output. An agent receiving `fmt --json` cannot determine which files changed or what changed in them. Agents must re-read all files after `fmt` to detect changes.

### `test`
- **Human output:** Structured summary (pass/fail counts, test names). Not raw cargo test output.
- **JSON output:** Flat single-level object. Diverges from `check` JSON shape (nested results array). When compilation fails, `passed` and `failed` are `null` not `0`.
- **Assessment:** Functional but inconsistent with `check` shape. The `location.type: "Absolute"` for test names (Bug 2) is a semantic mismatch — test names are not file paths.

### `build`
- **Human output:** Structured. Reports errors with file/line/column. Exits nonzero on failure.
- **JSON output:** Same diagnostic shape as `check` at the error level. But process exit code is 0 regardless of build success (Bug 1). The JSON `exit_code` field correctly records the tool's exit, but the 8v process itself exits 0.
- **Assessment:** The exit code bug makes `--json` mode unreliable for any script or agent that tests `$?`. Agent must parse JSON and check `success` field — cannot rely on standard shell convention.

---

## 3. Top 5 Issues Agents Will Hit

### Issue 1 — `build --json` exits 0 on failure (Bug 1)
**Impact:** Critical. Any agent or CI script using `8v build --json` and checking `$?` will believe the build succeeded when it failed.

**Reproduction:**
```bash
8v build /tmp/rust-qa-compile-error --json; echo "exit: $?"
# Prints: exit 0  (build failed)
```

**Workaround:** Agent must parse the JSON `success` field explicitly. `jq '.success'` must return `true` before trusting exit code.

---

### Issue 2 — Duplicate diagnostics from check + clippy (Bug 6)
**Impact:** High. Agents counting errors or deduplicating to decide "fixed vs. not" will overcounts by 2× for every compile error. Every compilation failure emits the same diagnostic twice.

**Reproduction:**
```bash
8v check /tmp/rust-qa-compile-error --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d['results']:
    for c in r['checks']:
        print(c['name'], len(c['diagnostics']), 'diagnostics')
"
# cargo_check  2 diagnostics
# clippy       2 diagnostics  ← same error, duplicated
```

**Workaround:** Agent must deduplicate by `(path, line, column, rule)` before acting on diagnostics.

---

### Issue 3 — `fmt --json` has no file-level detail (Bug 7)
**Impact:** High. An agent that formats a project cannot learn which files changed. It must re-read every source file after `fmt` to detect modifications. Token cost scales with project size.

**Reproduction:**
```bash
8v fmt /tmp/rust-qa-fmt --json
# {"stacks":[{"name":"rust","status":"dirty","timing_ms":111,"tool":"cargo"}]}
# No file paths. No diff content.
```

**Workaround:** Run `git diff` after `fmt` to identify changed files. Or re-read entire source tree.

---

### Issue 4 — `test --json` location type mismatch (Bug 2)
**Impact:** Medium. Test failure locations have `"type": "Absolute"` with a test module path like `tests::test_add_wrong`. This is not a file path. Agents that try to open the file at `location.path` will fail.

**Reproduction:**
```bash
8v test /tmp/rust-qa-failing-test --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d['errors'][0]['location'])
"
# {'path': 'tests::test_add_wrong', 'type': 'Absolute'}
```

**Workaround:** Agent must check whether `location.path` is a filesystem path before attempting to read it. Test names containing `::` are module paths, not file paths.

---

### Issue 5 — Phantom empty-path diagnostics in `check` (Bug 5)
**Impact:** Medium. Compile errors emit follow-up `failure-note` lines (e.g., "For more information about this error, try `rustc --explain E0308`") as diagnostics with `"path": ""`. Agents iterating diagnostics and opening `location.path` will attempt to open an empty path.

**Reproduction:**
```bash
8v check /tmp/rust-qa-compile-error --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d['results']:
    for c in r['checks']:
        for diag in c['diagnostics']:
            if diag['location']['path'] == '':
                print('PHANTOM:', diag['message'][:60])
"
# PHANTOM: For more information about this error, try \`rustc --ex...
```

**Workaround:** Agent must filter diagnostics where `location.path == ""` before attempting file operations.

---

## 4. JSON Shape Quality Per Command

### `check --json` — Grade: A−

```json
{
  "results": [{
    "project": "<name>",
    "stack": "rust",
    "path": "<absolute-path>",
    "checks": [{
      "name": "clippy",
      "outcome": "failed",
      "ms": 495,
      "parse_status": "parsed",
      "diagnostics": [{
        "location": {"type": "File", "path": "<abs-path>"},
        "span": {"line": 2, "column": 18, "end_line": 2, "end_column": 40},
        "rule": "clippy::manual_ok_err",
        "severity": "error",
        "raw_severity": "error",
        "message": "...",
        "related": [],
        "notes": [],
        "suggestions": [{
          "message": "replace with",
          "applicability": "MachineApplicable",
          "edits": [{"span": {...}, "new_text": "..."}]
        }],
        "snippet": "...",
        "tool": "clippy",
        "stack": "rust"
      }]
    }]
  }],
  "detection_errors": [],
  "summary": {"success": false, "passed": 2, "failed": 2, "errors": 0, "detection_errors": 0, "ms": 1304},
  "delta": {"new": 0, "fixed": 0, "unchanged": 3}
}
```

**Strengths:** Machine-applicable suggestions with edit spans. Delta tracking. Full location + span. Rule IDs.
**Weaknesses:** Duplicate diagnostics (Bug 6). Phantom empty-path entries (Bug 5). No deduplication across checks.

---

### `fmt --json` — Grade: D

```json
{"stacks": [{"name": "rust", "status": "dirty", "timing_ms": 111, "tool": "cargo"}]}
```

**Strengths:** Fast. Correct status field (`dirty`/`clean`/`formatted`).
**Weaknesses:** No file paths. No diff. No line numbers. Single stack-level summary is not actionable. Agent cannot determine what needs fixing without re-reading the entire project.

---

### `test --json` — Grade: B−

```json
{
  "command": "cargo test --workspace -- -Z unstable-options --format=json --report-time",
  "detection_errors": [],
  "duration_ms": 30329,
  "errors": [{
    "location": {"path": "tests::test_add_wrong", "type": "Absolute"},
    "message": "test `tests::test_add_wrong` failed...",
    "severity": "error",
    "rule": null,
    "span": null,
    "stack": "rust",
    "tool": "cargo test"
  }],
  "exit_code": 101,
  "failed": 1,
  "ignored": 0,
  "name": "<project>",
  "passed": 1,
  "stack": "rust",
  "success": false,
  "truncated": {"stderr": false, "stdout": false}
}
```

**Strengths:** Includes command string. Pass/fail counts. Truncation flags. Individual test failure list.
**Weaknesses:** Flat shape (vs. nested `check`). `location.type: "Absolute"` for test names (Bug 2). `passed`/`failed` are `null` (not `0`) on compile failure. No top-level `results[]` wrapper — inconsistent with `check`.

---

### `build --json` — Grade: C+

```json
{
  "command": "cargo build --message-format=json",
  "detection_errors": [],
  "duration_ms": 178,
  "errors": [{
    "location": {"path": "src/main.rs", "type": "File"},
    "span": {"column": 18, "end_column": 40, "end_line": 2, "line": 2},
    "message": "mismatched types",
    "rule": "E0308",
    "severity": "error",
    "suggestions": [],
    "stack": "rust",
    "tool": "cargo build"
  }],
  "exit_code": 101,
  "name": "<project>",
  "stack": "rust",
  "success": false,
  "truncated": {"stderr": false, "stdout": false}
}
```

**Strengths:** Full location + span. Rule codes. `exit_code` field present. Same diagnostic shape as `check` at error level.
**Weaknesses:** Process exits 0 even when `success: false` (Bug 1). No `delta` tracking (unlike `check`). Flat shape, no `results[]` wrapper.

---

## 5. Missing Detections

### Polyglot fixture — rust stack unusable (workspace collision)

**Symptom:** Running `8v check` on the polyglot fixture at `o8v-testkit/tests/fixtures/polyglot-violated/` causes all three rust checks (cargo_check, clippy, rustfmt_check) to fail with:

```
current package believes it's in a workspace when it's not:
current:   .../polyglot-violated/Cargo.toml
workspace: .../oss/8v/Cargo.toml
```

The fixture's `Cargo.toml` is nested inside the 8v workspace. Cargo resolves to the parent workspace, and since the fixture is not listed as a member, the check fails entirely. No actual rust diagnostics from the fixture are produced.

**Impact:** The `polyglot-violated` fixture cannot be used to test 8v's rust diagnostic detection. Python and Dockerfile detections still work (they are not workspace-aware).

**Verified python detection:**
```bash
8v check o8v-testkit/tests/fixtures/polyglot-violated --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d['results']:
    print(r['stack'], ':', sum(len(c['diagnostics']) for c in r['checks']), 'diagnostics')
"
# python : 3 diagnostics (ruff violations, actual fixture errors)
# dockerfile : 2 diagnostics (hadolint violations)
# rust : 0 diagnostics (workspace collision — all checks error out)
```

### Shell script checking — not present

No shell stack detected in any of the tested projects. `shellcheck` support exists in the documentation but the polyglot fixture `scripts/` directory does not trigger shell detection.

**Reproduction:**
```bash
ls o8v-testkit/tests/fixtures/polyglot-violated/scripts/
# Contains shell scripts
8v check o8v-testkit/tests/fixtures/polyglot-violated --json | python3 -c "
import json, sys
d = json.load(sys.stdin)
print([r['stack'] for r in d['results']])
"
# ['rust', 'python', 'dockerfile']   ← no 'shell'
```

### `fmt` — no check for non-rust stacks

`8v fmt` only detected the rust stack in all tested multi-stack scenarios. No python (ruff format), no shell (shfmt), no dockerfile formatting was reported.

---

## 6. Proposed Next Slice Candidates

Priority ordered by agent impact:

### P0 — Fix `build --json` exit code (Bug 1)
`8v build --json` must exit nonzero when `success: false`. This is the most critical issue: it breaks every script/agent that uses standard shell convention. One-line fix in the CLI dispatch for build.

### P0 — Deduplicate check diagnostics (Bug 6)
Same diagnostic emitted by cargo_check and clippy. Deduplication by `(path, line, column, message)` before emitting results. Otherwise agent error counts are 2× actual for all compile errors.

### P1 — `fmt --json` add file-level detail (Bug 7)
Capture which files were modified by `cargo fmt`. Expose as `"files": [{"path": "...", "status": "formatted"}]` per stack entry. Agents cannot act on stack-level summaries only.

### P1 — Fix phantom empty-path diagnostics (Bug 5)
`failure-note` lines with no source location should be attached to the parent diagnostic as `notes[]`, not emitted as standalone diagnostics with `path: ""`. The `notes` field already exists on the diagnostic type.

### P2 — Fix `test --json` location type for test names (Bug 2)
`location.type` for test module paths should be `"TestName"` or `"Module"`, not `"Absolute"`. File path operations on `tests::foo::bar` will fail silently.

### P2 — Fix `fmt` human output grammar (Bug 3) + check-mode wording (Bug 4)
- "1 stacks formatted" → "1 stack formatted"
- `--check` mode on clean file: "1 stacks formatted" → "1 stack clean" (nothing was formatted)

### P3 — Fix rust detection in nested workspace fixtures
The polyglot fixture cannot be used as a rust test target because it sits inside the 8v workspace. Either: (a) add fixture to workspace members (changes the workspace itself), or (b) set `CARGO_TARGET_DIR` and a temporary workspace root when running checks on nested paths. This affects real users who run `8v check` on a subdirectory of a larger workspace.

### P3 — Align `test --json` and `build --json` shape with `check --json`
Add a `results[]` wrapper to `test` and `build` JSON output for consistency. Makes programmatic handling uniform across all verify commands.

---

## Dogfood Friction

**Friction reported from this QA session using 8v as the primary tool:**

### 1. `8v read` on 272KB JSON output — tool times out / produces unusable output
Running `8v check . --json` on the 8v repo produces a 272KB JSON blob. Attempting to read this through 8v tools caused timeouts. Workaround was piping to `python3 -c` via Bash. **Friction: medium.** `8v check --json` should offer a `--summary-only` flag that emits just the summary and delta without the full diagnostics array.

### 2. `8v search` for patterns in JSON output — not possible
`8v search` operates on source files. There is no way to search within the output of another 8v command without shelling out. When investigating a specific diagnostic field across a large JSON output, the only option was Bash + python3. **Friction: low.** Not a blocking issue, just an expected CLI limitation.

### 3. No way to diff `8v check` results between two runs without the event store
The delta tracking (`new/fixed/unchanged`) is tied to `~/.8v/events.ndjson`. In a fresh `/tmp/` project, first run always shows `delta: {new: N, fixed: 0, unchanged: 0}` — there is no baseline. Makes it impossible to test delta behavior in a hermetic fixture without seeding the event store. **Friction: medium.** Affects fixture design for test authors.

### 4. `8v build` vs `8v check` — unclear which to use when
Both commands detect compilation errors. `build` produces a flatter output and misses clippy; `check` runs three tools. Documentation does not explain when an agent should use `build` vs `check`. In practice, `check` is strictly more informative. **Friction: low.** A short note in `--help` output distinguishing the two would eliminate confusion.

### 5. Exit code inconsistency between command families
`check`/`fmt` exit 2 on no-project; `build`/`test` exit 1. This means a script checking `$? -ge 1` treats a missing project as a build failure. **Friction: medium for CI scripts.** A single documented exit code contract across all verify commands would fix this.

---

## Summary Table

| Bug | Command | Severity | Affects Agents |
|---|---|---|---|
| Bug 1 — build --json exits 0 on failure | build | Critical | Yes — breaks $? check |
| Bug 6 — duplicate diagnostics | check | High | Yes — 2× error count |
| Bug 7 — fmt JSON no file detail | fmt | High | Yes — blind after format |
| Bug 5 — phantom empty-path diag | check | Medium | Yes — empty path open attempt |
| Bug 2 — test location.type Absolute | test | Medium | Yes — bad path open attempt |
| Bug 3 — "1 stacks" grammar | fmt | Low | No |
| Bug 4 — "formatted" in check mode | fmt | Low | No |
| Finding — exit code inconsistency | all | Medium | CI scripts |
| Finding — polyglot workspace collision | check | High | Fixture unusable |
| Finding — shell stack missing | check | Medium | Missing detection |
