// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for the Java stack — designed to catch real bugs.
//!
//! Each test makes a specific, falsifiable claim about the parser or the
//! check pipeline. A test that passes both before and after a bug fix proves
//! nothing; every test here was written to FAIL on a known defect.
//!
//! Known bugs caught by this file:
//! - Maven repeats each error in its failure footer, producing duplicate
//!   diagnostics (4 instead of 2). Fixed by stopping at "Failed to execute".

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

fn java_checks(json: &serde_json::Value) -> &Vec<serde_json::Value> {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("java"))
        .expect("java result not found")["checks"]
        .as_array()
        .expect("checks array")
}

fn mvn_compile_check(json: &serde_json::Value) -> &serde_json::Value {
    java_checks(json)
        .iter()
        .find(|c| c["name"].as_str() == Some("mvn compile"))
        .expect("mvn compile check not found")
}

// ─── Clean project ──────────────────────────────────────────────────────────

#[test]
fn java_clean_check_exits_0() {
    let project = fixture("check-java");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-java");
    assert_eq!(
        out.status.code().unwrap_or(99),
        0,
        "8v check on clean Java project must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn java_clean_mvn_compile_outcome_passed() {
    let project = fixture("check-java");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("passed"),
        "mvn compile must pass on clean code: {check}"
    );
}

#[test]
fn java_clean_zero_diagnostics() {
    // If the parser emits false positives on clean code, this test catches it.
    let project = fixture("check-java");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let count = check["diagnostics"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "clean Java project must produce zero diagnostics, got {count}: {check}"
    );
}

// ─── Violations project ─────────────────────────────────────────────────────

#[test]
fn java_violations_check_exits_1() {
    let project = fixture("check-java-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-java-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "8v check with compile errors must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn java_violations_outcome_is_failed_not_error() {
    // outcome="error" means the tool couldn't run. outcome="failed" means it ran
    // and found problems. These must not be confused.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("failed"),
        "compile errors must produce outcome=failed, not error: {check}"
    );
}

#[test]
fn java_violations_exactly_two_diagnostics() {
    // BUG REGRESSION: Maven repeats each error twice (compilation section +
    // failure footer). The parser must stop at "Failed to execute" and not
    // produce 4 diagnostics for 2 errors.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        2,
        "Broken.java has exactly 2 errors — parser must not duplicate them.\n\
         If this fails with 4 diagnostics, the failure-footer dedup fix is missing.\n\
         Got diagnostics: {diags:?}"
    );
}

#[test]
fn java_violations_all_diagnostics_are_errors() {
    // Both violations in Broken.java are compile errors, not warnings.
    // If severity mapping regresses, this catches it.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "all Broken.java diagnostics must be severity=error: {d}"
        );
    }
}

#[test]
fn java_violations_span_has_line_and_column() {
    // Maven format provides [line,column] — the parser must extract both.
    // Javac-only format only gets line. This test verifies Maven column capture.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diags.is_empty(),
        "expected diagnostics to check span fields"
    );
    for d in diags {
        let span = &d["span"];
        let line = span["line"].as_u64().unwrap_or(0);
        let col = span["column"].as_u64().unwrap_or(0);
        assert!(line > 0, "diagnostic span must have line > 0: {d}");
        assert!(
            col > 0,
            "diagnostic span must have column > 0 (Maven provides it): {d}"
        );
    }
}

#[test]
fn java_violations_first_error_is_type_mismatch() {
    // Broken.java line 5: `int x = "not an int"` — incompatible types.
    // If message extraction regresses, this catches it.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let first = &diags[0];
    assert!(
        first["message"]
            .as_str()
            .unwrap_or("")
            .contains("incompatible types"),
        "first diagnostic must be the incompatible types error: {first}"
    );
    assert_eq!(
        first["span"]["line"].as_u64(),
        Some(5),
        "type mismatch is on line 5: {first}"
    );
}

#[test]
fn java_violations_second_error_is_undefined_symbol() {
    // Broken.java line 6: `undefinedVar` — cannot find symbol.
    let project = fixture("check-java-violations");
    let json = check_json(&project);
    let check = mvn_compile_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let second = &diags[1];
    assert!(
        second["message"]
            .as_str()
            .unwrap_or("")
            .contains("cannot find symbol"),
        "second diagnostic must be the undefined symbol error: {second}"
    );
    assert_eq!(
        second["span"]["line"].as_u64(),
        Some(6),
        "undefined symbol is on line 6: {second}"
    );
}

// ─── Build and test commands ─────────────────────────────────────────────────

#[test]
fn java_build_clean_exits_0() {
    let project = fixture("check-java");
    let out = bin()
        .args(["build", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-java");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };
    assert_eq!(
        json["exit_code"].as_i64().unwrap_or(-1),
        0,
        "mvn package on clean Java project must exit 0\nstderr: {}",
        json["stderr"].as_str().unwrap_or("")
    );
}

#[test]
fn java_build_violations_exits_nonzero() {
    // If the build pipeline stops surfacing compile failures, this catches it.
    let project = fixture("check-java-violations");
    let out = bin()
        .args(["build", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-java-violations");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };
    assert_ne!(
        json["exit_code"].as_i64().unwrap_or(0),
        0,
        "mvn package on broken Java project must exit non-zero"
    );
}

#[test]
fn java_check_json_stack_field_is_java() {
    let project = fixture("check-java");
    let json = check_json(&project);
    let results = json["results"].as_array().expect("results array");
    assert!(
        results.iter().any(|r| r["stack"].as_str() == Some("java")),
        "java stack must appear in check results: {results:?}"
    );
}
