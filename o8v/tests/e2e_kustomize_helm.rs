// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for Kustomize and Helm stacks.
//!
//! Both stacks have no build step or test runner — only check tools.
//! `8v build` and `8v test` must respond with a clear error message, not crash.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    TempProject::from_fixture(&fixture_path("o8v", name))
}

// ─── Kustomize ──────────────────────────────────────────────────────────────

#[test]
fn kustomize_check_clean_exits_0() {
    let project = fixture("check-kustomize");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-kustomize");
    assert_eq!(
        out.status.code().unwrap_or(99),
        0,
        "kustomize build passes on clean fixture\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn kustomize_check_json_valid() {
    let project = fixture("check-kustomize");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-kustomize");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let kust = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("kustomize"))
        .expect("kustomize result not found");

    let checks = kust["checks"].as_array().expect("checks array");
    let kustomize_build = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("kustomize build"))
        .expect("kustomize build check not found");

    assert_eq!(
        kustomize_build["outcome"].as_str(),
        Some("passed"),
        "kustomize build should pass on clean fixture: {kustomize_build}"
    );
}

#[test]
fn kustomize_check_json_checks_have_name_and_outcome() {
    let project = fixture("check-kustomize");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-kustomize");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let kust = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("kustomize"))
        .expect("kustomize result");

    for check in kust["checks"].as_array().expect("checks array") {
        assert!(check["name"].is_string(), "check missing 'name': {check}");
        assert!(
            check["outcome"].is_string(),
            "check missing 'outcome': {check}"
        );
    }
}

#[test]
fn kustomize_build_reports_no_build_step_not_crash() {
    let project = fixture("check-kustomize");
    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-kustomize");

    let code = out.status.code().unwrap_or(99);
    assert_ne!(code, 99, "8v build must not crash on kustomize project");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no build step") || stderr.contains("kustomize"),
        "expected clear message about missing build step, got: {stderr}"
    );
}

#[test]
fn kustomize_test_reports_no_test_concept_not_crash() {
    let project = fixture("check-kustomize");
    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on check-kustomize");

    let code = out.status.code().unwrap_or(99);
    assert_ne!(code, 99, "8v test must not crash on kustomize project");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no test concept") || stderr.contains("kustomize"),
        "expected clear message about missing test runner, got: {stderr}"
    );
}

// ─── Helm ───────────────────────────────────────────────────────────────────

#[test]
fn helm_check_exits_0_or_1() {
    let project = fixture("check-helm");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-helm");
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on helm project must exit 0 or 1, got {code}"
    );
}

#[test]
fn helm_check_json_has_helm_lint_entry() {
    let project = fixture("check-helm");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-helm");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let helm = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("helm"))
        .expect("helm result not found");

    let checks = helm["checks"].as_array().expect("checks array");
    let has_lint = checks
        .iter()
        .any(|c| c["name"].as_str() == Some("helm lint"));
    assert!(
        has_lint,
        "expected 'helm lint' in checks, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn helm_check_json_lint_failed_on_incomplete_chart() {
    // The check-helm fixture has an incomplete chart that fails helm lint.
    let project = fixture("check-helm");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-helm");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let helm = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("helm"))
        .expect("helm result");

    let checks = helm["checks"].as_array().expect("checks array");
    let lint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("helm lint"))
        .expect("helm lint entry");

    assert_eq!(
        lint["outcome"].as_str(),
        Some("failed"),
        "helm lint should fail on incomplete chart: {lint}"
    );
}

#[test]
fn helm_build_reports_no_build_step_not_crash() {
    let project = fixture("check-helm");
    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-helm");

    let code = out.status.code().unwrap_or(99);
    assert_ne!(code, 99, "8v build must not crash on helm project");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no build step") || stderr.contains("helm"),
        "expected clear message about missing build step, got: {stderr}"
    );
}

#[test]
fn helm_test_reports_no_test_concept_not_crash() {
    let project = fixture("check-helm");
    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on check-helm");

    let code = out.status.code().unwrap_or(99);
    assert_ne!(code, 99, "8v test must not crash on helm project");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no test concept") || stderr.contains("helm"),
        "expected clear message about missing test runner, got: {stderr}"
    );
}
