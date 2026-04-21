// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on .NET projects — build, test, check, fmt --check.
//!
//! Uses the corpus fixture `dotnet-slnx-solution` which has a broken solution file.
//! All build/test/check commands fail with exit 1 (not crash) — these tests verify
//! that the pipeline handles .NET gracefully under real failure conditions.

use o8v_testkit::Fixture;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── 8v build ───────────────────────────────────────────────────────────────

#[test]
fn dotnet_build_does_not_crash() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["build", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on dotnet-slnx-solution");

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v build on dotnet must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dotnet_build_json_has_required_fields() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["build", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v build --json on dotnet-slnx-solution");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "dotnet", "stack should be dotnet");
    assert!(parsed.get("exit_code").is_some(), "missing exit_code");
    assert!(parsed.get("duration_ms").is_some(), "missing duration_ms");
    assert!(parsed.get("truncated").is_some(), "missing truncated");
    assert!(parsed.get("success").is_some(), "missing success");
}

#[test]
fn dotnet_build_json_stack_is_dotnet() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["build", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v build --json on dotnet");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    assert_eq!(parsed["stack"], "dotnet", "stack must be 'dotnet'");
}

// ─── 8v test ────────────────────────────────────────────────────────────────

#[test]
fn dotnet_test_does_not_crash() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["test", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on dotnet-slnx-solution");

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v test on dotnet must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dotnet_test_json_has_required_fields() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["test", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v test --json on dotnet-slnx-solution");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "dotnet", "stack should be dotnet");
    assert!(parsed.get("exit_code").is_some(), "missing exit_code");
    assert!(parsed.get("success").is_some(), "missing success");
    assert!(parsed.get("duration_ms").is_some(), "missing duration_ms");
    assert!(parsed.get("truncated").is_some(), "missing truncated");
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn dotnet_check_does_not_crash() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on dotnet-slnx-solution");

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on dotnet must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dotnet_check_json_valid_and_stack_dotnet() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on dotnet-slnx-solution");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "JSON output should not be empty");

    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should detect at least one project");

    let stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();
    assert!(
        stacks.contains(&"dotnet"),
        "expected 'dotnet' in stacks, got: {stacks:?}"
    );
}

#[test]
fn dotnet_check_json_has_build_check() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["check", "--json", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on dotnet-slnx-solution");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON must be valid");

    let results = parsed["results"].as_array().expect("results array");
    let dotnet_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("dotnet"))
        .expect("dotnet result not found");

    let checks = dotnet_result["checks"].as_array().expect("checks array");
    assert!(
        !checks.is_empty(),
        "dotnet checks array should not be empty"
    );

    // "dotnet build" must be present as a check
    let has_dotnet_build = checks
        .iter()
        .any(|c| c["name"].as_str() == Some("dotnet build"));
    assert!(
        has_dotnet_build,
        "expected 'dotnet build' check entry, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );

    // Every check must have name and outcome
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
fn dotnet_fmt_check_does_not_crash() {
    let f = Fixture::corpus("dotnet-slnx-solution");

    let out = bin()
        .args(["fmt", "--check", f.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on dotnet-slnx-solution");

    let code = out.status.code().unwrap_or(99);
    // Dotnet fmt uses `dotnet format` which may fail (broken slnx) — exit 0 or 1 both valid.
    assert!(
        code == 0 || code == 1,
        "8v fmt --check on dotnet must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
