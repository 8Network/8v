// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial QA tests for the Dockerfile stack — hadolint check pipeline.
//!
//! Each test makes a specific falsifiable claim about the parser or the check
//! pipeline. A test that passes both before and after a regression proves
//! nothing — every assertion here is tied to a concrete fact about the
//! fixture or the parser.
//!
//! Fixtures:
//! - check-dockerfile: clean, hadolint exits 0 with zero diagnostics.
//! - check-dockerfile-violations: four violations —
//!   line 1 DL3007 (warning), line 2 DL3009 (info),
//!   line 3 DL3015 (info), line 3 DL3008 (warning).
//!
//! Known parser surface this file probes:
//! - hadolint emits a bare JSON array (not an object) — parser must handle `[]`.
//! - Severity mapping: hadolint "warning"→Warning, "info"→Info, "style"→Hint.
//! - DL3007 is the canary rule: if hadolint is not running, no diagnostics appear.
//! - `parsed_items` and `diagnostics.len()` must agree — no silent drops.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    TempProject::from_fixture(&fixture_path("o8v", name))
}

fn check_json(project: &TempProject) -> serde_json::Value {
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    }
}

fn hadolint_check(json: &serde_json::Value) -> &serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("dockerfile"))
        .expect("dockerfile result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("hadolint"))
        .expect("hadolint check not found")
}

// ─── Clean project ──────────────────────────────────────────────────────────

#[test]
fn dockerfile_clean_check_exits_0() {
    // If exit-code mapping regresses for the clean case, this catches it.
    let project = fixture("check-dockerfile");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-dockerfile");
    assert_eq!(
        out.status.code().unwrap_or(99),
        0,
        "8v check on clean Dockerfile must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dockerfile_check_json_stack_is_dockerfile() {
    // If stack detection regresses and the project is misidentified, this catches it.
    let project = fixture("check-dockerfile");
    let json = check_json(&project);
    let results = json["results"].as_array().expect("results array");
    assert!(
        results
            .iter()
            .any(|r| r["stack"].as_str() == Some("dockerfile")),
        "dockerfile stack must appear in check results: {results:?}"
    );
}

#[test]
fn dockerfile_check_json_has_hadolint_entry() {
    // hadolint is the only linter for the dockerfile stack.
    // If the check pipeline drops the hadolint entry, this catches it.
    let project = fixture("check-dockerfile");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    assert_eq!(
        check["name"].as_str(),
        Some("hadolint"),
        "hadolint entry must be present in check results: {check}"
    );
}

#[test]
fn dockerfile_clean_outcome_passed() {
    // Ensures "passed" on clean code — inverts the violations tests.
    // If hadolint incorrectly flags clean code, this catches it.
    let project = fixture("check-dockerfile");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("passed"),
        "hadolint must be 'passed' on clean Dockerfile: {check}"
    );
}

#[test]
fn dockerfile_clean_zero_diagnostics() {
    // If the parser emits false-positive diagnostics on clean code, this catches it.
    // `FROM alpine:3.19` with a pinned version must produce no DL3007 hit.
    let project = fixture("check-dockerfile");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let count = check["diagnostics"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "clean Dockerfile must produce zero hadolint diagnostics, got {count}: {check}"
    );
}

// ─── Violations project ─────────────────────────────────────────────────────
//
// Fixture: check-dockerfile-violations/Dockerfile
//   FROM ubuntu:latest           → DL3007 warning  (line 1)
//   RUN apt-get update && ...    → DL3009 info      (line 2)
//   RUN apt-get install -y curl  → DL3015 info      (line 3)
//                                → DL3008 warning   (line 3)
// Total: 4 diagnostics.
//
// hadolint exits 1 when any issue is "error" level; for warnings/info it exits 0.
// But 8v maps any non-empty diagnostics to outcome="failed" and exit 1.

#[test]
fn dockerfile_violations_exits_1() {
    // 8v must exit 1 when any check produces diagnostics.
    // hadolint itself exits 0 for warning/info-only results, but 8v must still
    // map that to exit 1. This test catches a regression where 8v passes through
    // hadolint's raw exit code instead of its own outcome mapping.
    let project = fixture("check-dockerfile-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-dockerfile-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "hadolint warnings must cause 8v to exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dockerfile_violations_outcome_failed() {
    // outcome="error" means the tool couldn't run. outcome="failed" means it ran
    // and found problems. These must not be confused.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("failed"),
        "hadolint warnings must produce outcome=failed, not error: {check}"
    );
}

#[test]
fn dockerfile_violations_exactly_four_diagnostics() {
    // The violations Dockerfile produces exactly 4 hadolint hits.
    // Catches parser duplication (same line emitted twice) or silent-drop regressions.
    // If this fails with 0, hadolint is not running or the JSON array parser broke.
    // If this fails with 8, the JSON array is being parsed twice.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        4,
        "violations Dockerfile has exactly 4 hadolint hits (DL3007+DL3009+DL3015+DL3008).\n\
         If this fails with 0, hadolint is not running or the parser dropped everything.\n\
         If this fails with 8, the array is being parsed twice.\n\
         Got: {diags:?}"
    );
}

#[test]
fn dockerfile_violations_has_dl3007_rule() {
    // DL3007 fires on `FROM ubuntu:latest` — the canary rule.
    // If hadolint is not running at all, this will fail even if the count test passes.
    // The rule name must be exactly "DL3007", not null or renamed.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let has_dl3007 = diags
        .iter()
        .any(|d| d["rule"].as_str().map(|r| r == "DL3007").unwrap_or(false));
    assert!(
        has_dl3007,
        "expected DL3007 (FROM ubuntu:latest pins latest) in diagnostics: {diags:?}"
    );
}

#[test]
fn dockerfile_violations_dl3007_is_on_line_1() {
    // `FROM ubuntu:latest` is line 1 of the Dockerfile.
    // If line extraction regresses (line always 0 or wrong), this catches it.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let dl3007 = diags
        .iter()
        .find(|d| d["rule"].as_str().map(|r| r == "DL3007").unwrap_or(false));
    match dl3007 {
        Some(d) => assert_eq!(
            d["span"]["line"].as_u64(),
            Some(1),
            "DL3007 must be on line 1 (FROM ubuntu:latest): {d}"
        ),
        None => panic!("DL3007 diagnostic not found: {diags:?}"),
    }
}

#[test]
fn dockerfile_violations_dl3007_severity_is_warning() {
    // DL3007 is a "warning" in hadolint — not "error" or "info".
    // If severity mapping regresses (e.g., everything maps to "error"), this catches it.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let dl3007 = diags
        .iter()
        .find(|d| d["rule"].as_str().map(|r| r == "DL3007").unwrap_or(false));
    match dl3007 {
        Some(d) => assert_eq!(
            d["severity"].as_str(),
            Some("warning"),
            "DL3007 must be severity=warning: {d}"
        ),
        None => panic!("DL3007 diagnostic not found: {diags:?}"),
    }
}

#[test]
fn dockerfile_violations_has_dl3008_rule() {
    // DL3008 fires on `apt-get install curl` without pinning a version.
    // Validates that a second warning-severity rule is also parsed correctly.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let has_dl3008 = diags
        .iter()
        .any(|d| d["rule"].as_str().map(|r| r == "DL3008").unwrap_or(false));
    assert!(
        has_dl3008,
        "expected DL3008 (pin apt-get package versions) in diagnostics: {diags:?}"
    );
}

#[test]
fn dockerfile_violations_info_severity_rules_present() {
    // DL3009 and DL3015 fire as "info" severity.
    // This test verifies the "info" severity is parsed — not collapsed into "warning".
    // A regression in the severity mapping would make all info rules appear as warning.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let info_count = diags
        .iter()
        .filter(|d| d["severity"].as_str() == Some("info"))
        .count();
    assert_eq!(
        info_count,
        2,
        "expected exactly 2 info-severity diagnostics (DL3009 + DL3015), got {info_count}: {diags:?}"
    );
}

#[test]
fn dockerfile_violations_span_has_line() {
    // hadolint provides line numbers for every diagnostic.
    // The parser must extract them — line 0 means extraction failed.
    let project = fixture("check-dockerfile-violations");
    let json = check_json(&project);
    let check = hadolint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diags.is_empty(),
        "expected diagnostics to check span fields"
    );
    for d in diags {
        let line = d["span"]["line"].as_u64().unwrap_or(0);
        assert!(
            line > 0,
            "hadolint diagnostic must have line > 0 (hadolint provides it): {d}"
        );
    }
}
