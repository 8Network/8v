// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on JavaScript projects — check, fmt --check.
//!
//! Key behavior: `eslint` is required but not locally installed in the corpus fixture,
//! so its outcome is "error" (not optional). Prettier, biome, and oxlint are optional —
//! when not found, outcome is "passed" (skip). This produces overall exit 1 (error present).

use o8v_testkit::Fixture;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn javascript_check_does_not_crash() {
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on javascript-workspace");

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on javascript must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn javascript_check_json_is_valid() {
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "JSON output should not be empty");

    let _: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };
}

#[test]
fn javascript_check_json_stack_is_javascript() {
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should detect at least one project");

    let stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();
    assert!(
        stacks.contains(&"javascript"),
        "expected 'javascript' in stacks, got: {stacks:?}"
    );
}

#[test]
fn javascript_check_eslint_outcome_is_error() {
    // eslint is not installed locally — the corpus fixture has no node_modules.
    // The expected outcome is "error" (not optional, tool not found).
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    // eslint error causes overall exit 1
    assert!(
        !out.status.success(),
        "8v check should exit non-zero when eslint is not installed"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let results = parsed["results"].as_array().expect("results array");
    let js_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("javascript"))
        .expect("javascript result not found");

    let checks = js_result["checks"].as_array().expect("checks array");

    let eslint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("eslint"))
        .expect("eslint check not found in checks array");

    assert_eq!(
        eslint["outcome"], "error",
        "eslint outcome should be 'error' when not installed: {eslint}"
    );

    // cause field must explain why it errored
    let cause = eslint["cause"].as_str().unwrap_or("");
    assert!(
        !cause.is_empty(),
        "eslint error should have a cause field explaining why: {eslint}"
    );
    assert!(
        cause.contains("eslint") || cause.contains("npm"),
        "cause should mention eslint or npm: {cause}"
    );
}

#[test]
fn javascript_check_optional_tools_outcome_is_passed() {
    // prettier, biome, oxlint are optional — when not installed locally,
    // they are skipped with outcome "passed" (not an error).
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let results = parsed["results"].as_array().expect("results array");
    let js_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("javascript"))
        .expect("javascript result not found");

    let checks = js_result["checks"].as_array().expect("checks array");

    // Optional tools that skip when not installed must not produce "error"
    for optional_tool in ["prettier", "biome", "oxlint"] {
        if let Some(check) = checks
            .iter()
            .find(|c| c["name"].as_str() == Some(optional_tool))
        {
            let outcome = check["outcome"].as_str().unwrap_or("unknown");
            assert_ne!(
                outcome, "error",
                "{optional_tool} is optional — should not be 'error' when not installed, got '{outcome}'"
            );
        }
    }
}

#[test]
fn javascript_check_json_summary_shows_errors() {
    // eslint missing = 1 error in summary
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let summary = parsed["summary"].as_object().expect("summary object");
    assert!(
        !summary
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        "summary.success should be false when eslint errors"
    );
    let errors = summary.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
    assert!(
        errors >= 1,
        "summary.errors should be >= 1 (eslint not installed): {summary:?}"
    );
}

#[test]
fn javascript_check_json_checks_have_name_and_outcome() {
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on javascript-workspace");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let results = parsed["results"].as_array().expect("results array");
    let js_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("javascript"))
        .expect("javascript result not found");

    let checks = js_result["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty(), "checks array should not be empty");

    for check in checks {
        assert!(check["name"].is_string(), "check missing 'name': {check}");
        assert!(
            check["outcome"].is_string(),
            "check missing 'outcome': {check}"
        );
    }
}

// ─── 8v fmt --check ─────────────────────────────────────────────────────────

#[test]
fn javascript_fmt_check_does_not_crash() {
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["fmt", "--check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on javascript-workspace");

    let code = out.status.code().unwrap_or(99);
    // prettier not found → exit 1 (error), or 0 if all optional. Either is valid, no crash.
    assert!(
        code == 0 || code == 1,
        "8v fmt --check on javascript must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn javascript_fmt_check_fails_when_prettier_not_installed() {
    // The corpus fixture has no node_modules — prettier is not available.
    // 8v fmt --check for JS uses prettier; missing prettier → error → exit 1.
    let f = Fixture::corpus("javascript-workspace");

    let out = bin()
        .args(["fmt", "--check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on javascript-workspace");

    assert!(
        !out.status.success(),
        "8v fmt --check should exit non-zero when prettier is not installed"
    );
}
