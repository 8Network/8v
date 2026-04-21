// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on infrastructure stacks: Terraform, Dockerfile, Helm.
//!
//! Terraform: `check-terraform` is clean; `check-terraform-violations` has unused
//!   variable declarations → tflint fails.
//! Dockerfile: `check-dockerfile` is clean; `check-dockerfile-violations` has
//!   DL3007/DL3008 (latest tag, unpinned apt) → hadolint fails.
//! Helm: `check-helm` has incomplete templates → helm lint fails.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── Terraform ──────────────────────────────────────────────────────────────

#[test]
fn terraform_check_unused_var_exits_1() {
    let project = fixture("check-terraform-violations");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-terraform");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        1,
        "8v check on terraform with unused var should exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn terraform_check_json_has_tflint() {
    let project = fixture("check-terraform");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-terraform");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let tf_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("terraform"))
        .expect("terraform result not found");

    let checks = tf_result["checks"].as_array().expect("checks array");
    let has_tflint = checks.iter().any(|c| c["name"].as_str() == Some("tflint"));
    assert!(
        has_tflint,
        "expected 'tflint' entry in checks, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn terraform_check_json_diagnostic_rule() {
    let project = fixture("check-terraform-violations");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-terraform");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let tf_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("terraform"))
        .expect("terraform result not found");

    let checks = tf_result["checks"].as_array().expect("checks array");
    let tflint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("tflint"))
        .expect("tflint check not found");

    let diagnostics = tflint["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "tflint should report diagnostics for unused declarations"
    );

    let has_unused_rule = diagnostics.iter().any(|d| {
        d["rule"]
            .as_str()
            .map(|r| r.contains("terraform_unused_declarations"))
            .unwrap_or(false)
    });
    assert!(
        has_unused_rule,
        "expected terraform_unused_declarations rule in diagnostics: {diagnostics:?}"
    );
}

// ─── Dockerfile (clean) ─────────────────────────────────────────────────────

#[test]
fn dockerfile_check_clean_exits_0() {
    let project = fixture("check-dockerfile");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-dockerfile");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v check on clean dockerfile should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dockerfile_check_json_hadolint_passed() {
    let project = fixture("check-dockerfile");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-dockerfile");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let df_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("dockerfile"))
        .expect("dockerfile result not found");

    let checks = df_result["checks"].as_array().expect("checks array");
    let hadolint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("hadolint"))
        .expect("hadolint check not found");

    assert_eq!(
        hadolint["outcome"], "passed",
        "hadolint should pass on clean dockerfile: {hadolint}"
    );
}

// ─── Dockerfile (violations) ────────────────────────────────────────────────

#[test]
fn dockerfile_violations_check_exits_1() {
    let project = fixture("check-dockerfile-violations");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-dockerfile-violations");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        1,
        "8v check on dockerfile with violations should exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dockerfile_violations_json_has_diagnostics() {
    let project = fixture("check-dockerfile-violations");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-dockerfile-violations");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let df_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("dockerfile"))
        .expect("dockerfile result not found");

    let checks = df_result["checks"].as_array().expect("checks array");
    let hadolint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("hadolint"))
        .expect("hadolint check not found");

    assert_eq!(
        hadolint["outcome"], "failed",
        "hadolint should fail on violations fixture: {hadolint}"
    );

    let diagnostics = hadolint["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "hadolint should report diagnostics for violations (latest tag, unpinned apt)"
    );
}

// ─── Helm ───────────────────────────────────────────────────────────────────

#[test]
fn helm_check_incomplete_exits_1() {
    let project = fixture("check-helm");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-helm");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        1,
        "8v check on incomplete helm chart should exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn helm_check_json_has_error_severity() {
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
    let helm_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("helm"))
        .expect("helm result not found");

    let checks = helm_result["checks"].as_array().expect("checks array");

    // Collect all diagnostics across all checks for this helm result
    let has_error = checks.iter().any(|c| {
        c["diagnostics"]
            .as_array()
            .map(|diags| {
                diags
                    .iter()
                    .any(|d| d["severity"].as_str() == Some("error"))
            })
            .unwrap_or(false)
    });
    assert!(
        has_error,
        "helm check should have at least one diagnostic with severity='error'"
    );
}
