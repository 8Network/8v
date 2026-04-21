// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on TypeScript projects.
//!
//! Uses the `typescript-workspace` corpus fixture. tsc is globally installed;
//! eslint is not (no node_modules). Assertions reflect this host configuration.

use o8v_testkit::Fixture;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn ts() -> Fixture {
    Fixture::corpus("typescript-workspace")
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn typescript_check_exits_0_or_1() {
    let f = ts();
    let out = bin()
        .args(["check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on typescript-workspace");
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on typescript project must exit 0 or 1, got {code}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn typescript_check_json_is_valid() {
    let f = ts();
    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
        "8v check --json must produce valid JSON\noutput: {stdout}"
    );
}

#[test]
fn typescript_check_json_stack_is_typescript() {
    let f = ts();
    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(
        results
            .iter()
            .any(|r| r["stack"].as_str() == Some("typescript")),
        "typescript not detected in results: {results:?}"
    );
}

#[test]
fn typescript_check_json_has_tsc_and_eslint() {
    let f = ts();
    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let ts_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("typescript"))
        .expect("typescript result");

    let checks = ts_result["checks"].as_array().expect("checks array");
    let names: Vec<&str> = checks.iter().filter_map(|c| c["name"].as_str()).collect();

    assert!(
        names.contains(&"tsc"),
        "expected tsc in checks, got: {names:?}"
    );
    assert!(
        names.contains(&"eslint"),
        "expected eslint in checks, got: {names:?}"
    );
}

#[test]
fn typescript_check_json_optional_tools_not_error_when_absent() {
    // prettier, biome, oxlint are optional — when not installed via node_modules,
    // they must report outcome="passed" (skipped), not "error".
    let f = ts();
    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let ts_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("typescript"))
        .expect("typescript result");

    let checks = ts_result["checks"].as_array().expect("checks array");
    for tool in ["prettier", "biome", "oxlint"] {
        if let Some(entry) = checks.iter().find(|c| c["name"].as_str() == Some(tool)) {
            let outcome = entry["outcome"].as_str().unwrap_or("");
            assert_ne!(
                outcome, "error",
                "{tool} is optional — must not be 'error' when absent, got: {entry}"
            );
        }
    }
}

#[test]
fn typescript_check_json_checks_have_name_and_outcome() {
    let f = ts();
    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let ts_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("typescript"))
        .expect("typescript result");

    let checks = ts_result["checks"].as_array().expect("checks array");
    assert!(
        !checks.is_empty(),
        "typescript must have at least one check"
    );

    for check in checks {
        assert!(
            check["name"].is_string(),
            "check entry missing 'name': {check}"
        );
        assert!(
            check["outcome"].is_string(),
            "check entry missing 'outcome': {check}"
        );
    }
}

// ─── 8v build ───────────────────────────────────────────────────────────────

#[test]
fn typescript_build_json_is_valid() {
    let f = ts();
    let out = bin()
        .args(["build", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v build --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(
        parsed["exit_code"].is_number(),
        "build output must have exit_code"
    );
    assert_eq!(
        parsed["stack"].as_str(),
        Some("typescript"),
        "build output must have stack=typescript"
    );
}

#[test]
fn typescript_build_succeeds_with_tsc_installed() {
    // tsc is globally installed — build should exit 0 on the clean workspace fixture.
    let f = ts();
    let out = bin()
        .args(["build", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v build --json on typescript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(
        parsed["exit_code"].as_i64().unwrap_or(-1),
        0,
        "typescript build should succeed when tsc is installed\nstderr: {}",
        parsed["stderr"].as_str().unwrap_or("")
    );
}

// ─── 8v test ────────────────────────────────────────────────────────────────

#[test]
fn typescript_test_no_runner_exits_nonzero_not_crash() {
    // The corpus fixture has no test script — 8v reports a clear error and exits non-zero.
    // It must NOT produce a panic or unstructured crash output.
    let f = ts();
    let out = bin()
        .args(["test", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on typescript-workspace");

    let code = out.status.code().unwrap_or(99);
    assert_ne!(
        code, 99,
        "8v test must not crash (got signal/unexpected exit)"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("test runner") || stderr.contains("no test"),
        "expected a clear diagnostic about missing test runner, got stderr: {stderr}"
    );
}

// ─── 8v fmt ─────────────────────────────────────────────────────────────────

#[test]
fn typescript_fmt_check_does_not_crash() {
    let f = ts();
    let out = bin()
        .args(["fmt", "--check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on typescript-workspace");
    let code = out.status.code().unwrap_or(99);
    assert_ne!(
        code, 99,
        "8v fmt --check must not crash on typescript project"
    );
}
