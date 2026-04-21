// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on Kotlin projects — check clean and violations.
//!
//! Uses `check-kotlin` (clean) and `check-kotlin-violations` (tab indentation +
//! wildcard import) to exercise the ktlint pass and fail paths.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn kotlin_check_clean_exits_0() {
    let project = fixture("check-kotlin");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-kotlin");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v check on clean kotlin project should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn kotlin_check_violations_exits_1() {
    let project = fixture("check-kotlin-violations");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-kotlin-violations");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        1,
        "8v check on kotlin with violations should exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn kotlin_check_json_has_ktlint_entry() {
    let project = fixture("check-kotlin");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-kotlin");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let kotlin_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("kotlin"))
        .expect("kotlin result not found");

    let checks = kotlin_result["checks"].as_array().expect("checks array");
    let has_ktlint = checks.iter().any(|c| c["name"].as_str() == Some("ktlint"));
    assert!(
        has_ktlint,
        "expected 'ktlint' entry in checks, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn kotlin_violations_json_has_diagnostics() {
    let project = fixture("check-kotlin-violations");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-kotlin-violations");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let kotlin_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("kotlin"))
        .expect("kotlin result not found");

    let checks = kotlin_result["checks"].as_array().expect("checks array");
    let ktlint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("ktlint"))
        .expect("ktlint check not found");

    assert_eq!(
        ktlint["outcome"], "failed",
        "ktlint should be 'failed' on violations fixture: {ktlint}"
    );

    let diagnostics = ktlint["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "ktlint should report diagnostics for tab/wildcard violations"
    );
}
