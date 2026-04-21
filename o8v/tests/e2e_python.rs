// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on Python projects — test, check, fmt --check.
//!
//! Covers the full command-level pipeline against isolated fixture projects.

use o8v_testkit::TempProject;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── 8v test ────────────────────────────────────────────────────────────────

#[test]
fn python_test_passing_exits_0() {
    let project = fixture("test-python-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on python-pass");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "8v test should exit 0 when all tests pass\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("python"),
        "output should name the stack: {stdout}"
    );
    assert!(
        stdout.contains("tests passed"),
        "output should say tests passed: {stdout}"
    );
}

#[test]
fn python_test_failing_exits_1() {
    let project = fixture("test-python-fail");

    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on python-fail");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v test should exit non-zero when tests fail\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("tests failed"),
        "output should say tests failed: {stdout}"
    );
}

#[test]
fn python_test_json_has_required_fields() {
    let project = fixture("test-python-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v test --json on python-pass");

    assert!(out.status.success(), "should exit 0 for passing tests");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "python", "stack should be python");
    assert_eq!(parsed["exit_code"], 0, "exit_code should be 0");
    assert!(
        parsed["success"].as_bool().unwrap_or(false),
        "success should be true"
    );
    assert!(parsed.get("duration_ms").is_some(), "missing duration_ms");
    assert!(parsed.get("truncated").is_some(), "missing truncated");
}

#[test]
fn python_test_fail_json_exit_code_nonzero() {
    let project = fixture("test-python-fail");

    let out = bin()
        .args(["test", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v test --json on python-fail");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "python", "stack should be python");
    assert_ne!(parsed["exit_code"], 0, "exit_code should be non-zero");
    assert!(
        !parsed["success"].as_bool().unwrap_or(true),
        "success should be false"
    );
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn python_check_clean_exits_0() {
    let project = fixture("check-python-clean");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-python-clean");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "8v check should exit 0 on clean python project\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn python_check_violations_exits_1() {
    let project = fixture("check-python-violations");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-python-violations");

    assert!(
        !out.status.success(),
        "8v check should exit non-zero when ruff finds violations"
    );
}

#[test]
fn python_check_violations_json_has_ruff_failed() {
    let project = fixture("check-python-violations");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-python-violations");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should have at least one result");

    let result = &results[0];
    assert_eq!(result["stack"], "python", "stack should be python");

    let checks = result["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty(), "checks array should not be empty");

    // ruff must be present and failed
    let ruff = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("ruff"))
        .expect("ruff check not found in checks array");
    assert_eq!(
        ruff["outcome"], "failed",
        "ruff outcome should be 'failed': {ruff}"
    );

    // ruff must have diagnostics (unused imports)
    let diagnostics = ruff["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "ruff should have diagnostics for the violations fixture"
    );

    // At least one diagnostic should be F401 (unused import)
    let has_f401 = diagnostics
        .iter()
        .any(|d| d["rule"].as_str() == Some("F401"));
    assert!(
        has_f401,
        "expected F401 (unused import) diagnostic: {diagnostics:?}"
    );
}

#[test]
fn python_check_clean_json_stack_is_python() {
    let project = fixture("check-python-clean");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-python-clean");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();
    assert!(
        stacks.contains(&"python"),
        "expected python in stacks, got: {stacks:?}"
    );

    // All checks must have name and outcome
    let checks = results[0]["checks"].as_array().expect("checks array");
    for check in checks {
        assert!(check["name"].is_string(), "check missing name: {check}");
        assert!(
            check["outcome"].is_string(),
            "check missing outcome: {check}"
        );
    }
}

// ─── 8v fmt --check ─────────────────────────────────────────────────────────

#[test]
fn python_fmt_check_exits_1_on_violations() {
    let project = fixture("check-python-violations");

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on check-python-violations");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v fmt --check must exit non-zero when python files need formatting\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn python_fmt_check_exits_0_on_clean() {
    let project = fixture("check-python-clean");

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on check-python-clean");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "8v fmt --check should exit 0 when python files are already formatted\nstdout: {stdout}\nstderr: {stderr}"
    );
}
