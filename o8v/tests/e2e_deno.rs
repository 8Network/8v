// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on Deno projects — check, fmt --check.
//!
//! Uses the `check-deno` fixture: a minimal deno.json + main.ts that type-checks clean.
//! The broken `deno-workspace` corpus fixture is intentionally avoided here.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = fixture_path("o8v", name);
    TempProject::from_fixture(&path)
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

fn deno_check_entry(json: &serde_json::Value) -> &serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("deno"))
        .expect("deno result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("deno check"))
        .expect("deno check entry not found")
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn deno_check_clean_exits_0() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-deno");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v check on clean deno project should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn deno_check_stack_detected() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-deno");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should detect at least one project");

    let stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();
    assert!(
        stacks.contains(&"deno"),
        "expected 'deno' in stacks, got: {stacks:?}"
    );
}

#[test]
fn deno_check_json_has_deno_check_entry() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-deno");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let deno_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("deno"))
        .expect("deno result not found");

    let checks = deno_result["checks"].as_array().expect("checks array");
    let has_deno_check = checks
        .iter()
        .any(|c| c["name"].as_str() == Some("deno check"));
    assert!(
        has_deno_check,
        "expected 'deno check' entry in checks, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

// ─── 8v fmt --check ─────────────────────────────────────────────────────────

#[test]
fn deno_fmt_check_runs() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on check-deno");

    // exit 0 (already formatted) or 1 (needs formatting) — must not crash
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v fmt --check on deno must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── Violations project ──────────────────────────────────────────────────────
//
// Fixture: check-deno-violations/
//   deno.json — minimal project manifest
//   broken.ts — two TS2322 type assignment errors:
//     line 1: `const x: number = "not a number"` — string not assignable to number
//     line 2: `const y: boolean = 42`             — number not assignable to boolean
//
// Adversarial design: every test must FAIL on a specific known defect, not
// just pass on correct behavior. Comments document which parser bug each test
// would catch.

#[test]
fn deno_violations_exits_1() {
    // If deno check exit-code mapping regresses (outcome="failed" but code=0),
    // this test catches it.
    let project = fixture("check-deno-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-deno-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "deno type errors must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn deno_violations_outcome_failed() {
    // outcome="error" means the tool couldn't run. outcome="failed" means it ran
    // and found problems. These must not be confused.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("failed"),
        "deno type errors must produce outcome=failed, not error: {check}"
    );
}

#[test]
fn deno_violations_exactly_two_diagnostics() {
    // broken.ts has exactly 2 type errors. Catches parser duplication (e.g., the
    // "Found 2 errors." summary line being parsed as an extra diagnostic) or
    // silent-drop regressions.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        2,
        "broken.ts has exactly 2 type errors — parser must not duplicate or drop them.\n\
         Got: {diags:?}"
    );
}

#[test]
fn deno_violations_has_ts2322_rule() {
    // Both errors in broken.ts are TS2322 (type assignment mismatch).
    // If rule extraction regresses (e.g., rule becomes None or wrong code), this catches it.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let has_rule = diags
        .iter()
        .any(|d| d["rule"].as_str().map(|r| r == "TS2322").unwrap_or(false));
    assert!(
        has_rule,
        "expected TS2322 rule in at least one diagnostic: {diags:?}"
    );
}

#[test]
fn deno_violations_all_severity_error() {
    // Both violations are type errors — severity must be "error", not "warning".
    // If severity mapping regresses (e.g., [ERROR] → warning), this catches it.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "all broken.ts diagnostics must be severity=error: {d}"
        );
    }
}

#[test]
fn deno_violations_span_has_line_and_column() {
    // deno check reports file:line:col — the parser must extract both.
    // If the "at file:///..." location parser misses the column, this catches it.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diags.is_empty(),
        "expected diagnostics to check span fields"
    );
    for d in diags {
        let line = d["span"]["line"].as_u64().unwrap_or(0);
        let col = d["span"]["column"].as_u64().unwrap_or(0);
        assert!(line > 0, "diagnostic span must have line > 0: {d}");
        assert!(
            col > 0,
            "diagnostic span must have column > 0 (deno provides it): {d}"
        );
    }
}

#[test]
fn deno_violations_first_error_line_1_string_not_number() {
    // broken.ts line 1: `const x: number = "not a number"` — string not assignable to number.
    // If message extraction or line extraction regresses, this catches it.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let first = &diags[0];
    assert_eq!(
        first["span"]["line"].as_u64(),
        Some(1),
        "first type error is on line 1: {first}"
    );
    assert!(
        first["message"]
            .as_str()
            .unwrap_or("")
            .contains("not assignable"),
        "first error message must describe a type assignment failure: {first}"
    );
}

#[test]
fn deno_violations_second_error_line_2_number_not_boolean() {
    // broken.ts line 2: `const y: boolean = 42` — number not assignable to boolean.
    // Tests that the second block is parsed correctly and not merged with the first.
    let project = fixture("check-deno-violations");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        diags.len() >= 2,
        "expected at least 2 diagnostics: {diags:?}"
    );
    let second = &diags[1];
    assert_eq!(
        second["span"]["line"].as_u64(),
        Some(2),
        "second type error is on line 2: {second}"
    );
    assert!(
        second["message"]
            .as_str()
            .unwrap_or("")
            .contains("not assignable"),
        "second error message must describe a type assignment failure: {second}"
    );
}

#[test]
fn deno_clean_zero_diagnostics() {
    // If the parser emits false positives on clean code, this test catches it.
    // Inverts the violations tests: deno check must exit 0 with zero diagnostics
    // on well-typed code.
    let project = fixture("check-deno");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    let count = check["diagnostics"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "clean deno project must produce zero diagnostics, got {count}: {check}"
    );
}

#[test]
fn deno_clean_outcome_passed() {
    // Ensures the outcome field is "passed" (not "failed" or "error") on clean code.
    // If the parser incorrectly maps a zero-diagnostic result to "failed", this catches it.
    let project = fixture("check-deno");
    let json = check_json(&project);
    let check = deno_check_entry(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("passed"),
        "deno check must be 'passed' on clean code: {check}"
    );
}
