// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on Go projects — check, test, fmt --check, build.
//!
//! Uses the corpus fixture `go-service` for clean-path tests and
//! `o8v/tests/fixtures/` for targeted fixture paths.

use o8v_testkit::{fixture_path, Fixture, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn go_check_clean_exits_0() {
    let f = Fixture::corpus("go-service");

    let out = bin()
        .args(["check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on go-service");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v check on clean go-service should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn go_check_json_go_vet_passed() {
    let f = Fixture::corpus("go-service");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on go-service");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should detect at least one project");

    let go_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("go"))
        .expect("go result not found");

    let checks = go_result["checks"].as_array().expect("checks array");
    let go_vet = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("go vet"))
        .expect("go vet check not found");

    assert_eq!(
        go_vet["outcome"], "passed",
        "go vet should pass on clean go-service: {go_vet}"
    );
}

#[test]
fn go_check_json_staticcheck_present() {
    let f = Fixture::corpus("go-service");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on go-service");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let go_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("go"))
        .expect("go result not found");

    let checks = go_result["checks"].as_array().expect("checks array");
    let has_staticcheck = checks
        .iter()
        .any(|c| c["name"].as_str() == Some("staticcheck"));
    assert!(
        has_staticcheck,
        "staticcheck should be present in go checks: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

// ─── 8v test ────────────────────────────────────────────────────────────────

#[test]
fn go_test_exits_0_when_no_tests() {
    let f = Fixture::corpus("go-service");

    let out = bin()
        .args(["test", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on go-service");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v test on go-service (no test files) should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn go_test_failing_exits_1() {
    let fixture_src = fixture_path("o8v", "agent-benchmark/fix-go");
    let project = TempProject::from_fixture(&fixture_src);

    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on fix-go");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        1,
        "8v test on fix-go (intentional test bug) should exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn go_test_json_has_fields() {
    let f = Fixture::corpus("go-service");

    let out = bin()
        .args(["test", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v test --json on go-service");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(parsed.get("exit_code").is_some(), "missing exit_code");
    assert!(parsed.get("success").is_some(), "missing success");
    assert_eq!(parsed["stack"], "go", "stack should be go");
}

// ─── 8v fmt --check ─────────────────────────────────────────────────────────

#[test]
fn go_fmt_check_clean_exits_0() {
    let fixture_src = fixture_path("o8v", "build-go");
    let project = TempProject::from_fixture(&fixture_src);

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on build-go");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v fmt --check on clean build-go should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── 8v build ───────────────────────────────────────────────────────────────

#[test]
fn go_build_succeeds() {
    let fixture_src = fixture_path("o8v", "build-go");
    let project = TempProject::from_fixture(&fixture_src);

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on build-go");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v build on build-go should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
