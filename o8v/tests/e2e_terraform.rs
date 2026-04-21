// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial QA tests for the Terraform stack — tflint check pipeline.
//!
//! Each test makes a specific falsifiable claim about the parser or the check
//! pipeline. A test that passes both before and after a regression proves
//! nothing — every assertion here is tied to a concrete fact about the
//! fixture or the parser.
//!
//! Fixtures:
//! - check-terraform: clean, tflint exits 0 with zero issues.
//! - check-terraform-violations: two unused variable declarations
//!   ("region" and "env"), tflint emits 2 warnings, 8v maps to exit 1.
//!
//! Known parser surface this file probes:
//! - tflint emits JSON `{"issues": [...], "errors": []}` to stdout.
//! - The parser must not confuse `errors` (tool-level errors) with `issues` (lint hits).
//! - Severity mapping: tflint "warning" → Severity::Warning, not Error.
//! - Span extraction: tflint provides start+end line+column — parser must extract all four.

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

fn tflint_check(json: &serde_json::Value) -> &serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("terraform"))
        .expect("terraform result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("tflint"))
        .expect("tflint check not found")
}

// ─── Clean project ──────────────────────────────────────────────────────────

#[test]
fn terraform_clean_check_exits_0() {
    // If exit-code mapping regresses for the clean case, this catches it.
    let project = fixture("check-terraform");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-terraform");
    assert_eq!(
        out.status.code().unwrap_or(99),
        0,
        "8v check on clean terraform project must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn terraform_check_json_stack_is_terraform() {
    // If stack detection regresses and the project is misidentified, this catches it.
    let project = fixture("check-terraform");
    let json = check_json(&project);
    let results = json["results"].as_array().expect("results array");
    assert!(
        results
            .iter()
            .any(|r| r["stack"].as_str() == Some("terraform")),
        "terraform stack must appear in check results: {results:?}"
    );
}

#[test]
fn terraform_check_json_has_tflint_entry() {
    // tflint is the only linter for the terraform stack.
    // If the check pipeline drops the tflint entry, this catches it.
    let project = fixture("check-terraform");
    let json = check_json(&project);
    let check = tflint_check(&json);
    assert_eq!(
        check["name"].as_str(),
        Some("tflint"),
        "tflint entry must be present in check results: {check}"
    );
}

#[test]
fn terraform_clean_outcome_passed() {
    // Ensures "passed" on clean code — inverts the violations test.
    // If the parser emits false-positive diagnostics on valid .tf, this catches it.
    let project = fixture("check-terraform");
    let json = check_json(&project);
    let check = tflint_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("passed"),
        "tflint must be 'passed' on clean terraform code: {check}"
    );
}

#[test]
fn terraform_clean_zero_diagnostics() {
    // If the parser emits spurious diagnostics on clean code (false positives),
    // this test fails. Catches regressions in the JSON parser or normalize_path.
    let project = fixture("check-terraform");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let count = check["diagnostics"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        count, 0,
        "clean terraform project must produce zero tflint diagnostics, got {count}: {check}"
    );
}

// ─── Violations project ─────────────────────────────────────────────────────
//
// Fixture: check-terraform-violations/main.tf
//   terraform { required_version = ">= 1.0" }
//   variable "region" { type = string; default = "us-east-1" }  ← unused
//   variable "env"    { type = string; default = "prod" }       ← unused
//
// tflint emits 2 issues (terraform_unused_declarations, severity=warning).
// tflint exits 0 (it does not use nonzero for warnings), but 8v maps
// any diagnostic to exit 1.

#[test]
fn terraform_violations_exits_1() {
    // 8v must exit 1 when any check produces diagnostics, even if the underlying
    // tool exits 0. This tests that the outcome→exit-code mapping is correct.
    let project = fixture("check-terraform-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-terraform-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "tflint warnings must cause 8v to exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn terraform_violations_outcome_failed() {
    // outcome="error" means the tool couldn't run. outcome="failed" means it ran
    // and found problems. These must not be confused.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    assert_eq!(
        check["outcome"].as_str(),
        Some("failed"),
        "tflint warnings must produce outcome=failed, not error: {check}"
    );
}

#[test]
fn terraform_violations_exactly_two_diagnostics() {
    // main.tf has exactly 2 unused variable declarations.
    // Catches parser duplication (same issue emitted twice) or silent-drop.
    // If this fails with 0 diagnostics, the parser is silently dropping issues.
    // If this fails with 4 diagnostics, the `errors` array is being counted as issues.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        2,
        "main.tf has exactly 2 unused variable declarations — parser must not duplicate or drop them.\n\
         If this fails with 4, the `errors` array is being conflated with `issues`.\n\
         Got: {diags:?}"
    );
}

#[test]
fn terraform_violations_has_unused_declarations_rule() {
    // Both violations trigger `terraform_unused_declarations`.
    // If rule name extraction regresses (e.g., rule becomes None), this catches it.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let has_rule = diags.iter().any(|d| {
        d["rule"]
            .as_str()
            .map(|r| r == "terraform_unused_declarations")
            .unwrap_or(false)
    });
    assert!(
        has_rule,
        "expected terraform_unused_declarations rule in diagnostics: {diags:?}"
    );
}

#[test]
fn terraform_violations_all_severity_warning() {
    // tflint emits these as "warning" severity — not "error" or "info".
    // If the severity mapping regresses (e.g., everything maps to error),
    // this test catches it.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("warning"),
            "tflint unused-declaration diagnostic must be severity=warning: {d}"
        );
    }
}

#[test]
fn terraform_violations_span_has_line_and_column() {
    // tflint provides start.line, start.column, end.line, end.column.
    // The parser must extract all four. If start.column is dropped, this catches it.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
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
            "diagnostic span must have column > 0 (tflint provides it): {d}"
        );
    }
}

#[test]
fn terraform_violations_region_variable_on_line_4() {
    // `variable "region"` starts on line 4 of main.tf.
    // If line extraction regresses (e.g., line always 0), this catches it.
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let region_diag = diags.iter().find(|d| {
        d["message"]
            .as_str()
            .map(|m| m.contains("\"region\""))
            .unwrap_or(false)
    });
    match region_diag {
        Some(d) => assert_eq!(
            d["span"]["line"].as_u64(),
            Some(4),
            "variable \"region\" declaration is on line 4: {d}"
        ),
        None => panic!("expected diagnostic mentioning \"region\": {diags:?}"),
    }
}

#[test]
fn terraform_violations_env_variable_on_line_8() {
    // `variable "env"` starts on line 8 of main.tf.
    // Tests that the second issue is parsed independently (not merged with the first).
    let project = fixture("check-terraform-violations");
    let json = check_json(&project);
    let check = tflint_check(&json);
    let diags = check["diagnostics"].as_array().expect("diagnostics array");
    let env_diag = diags.iter().find(|d| {
        d["message"]
            .as_str()
            .map(|m| m.contains("\"env\""))
            .unwrap_or(false)
    });
    match env_diag {
        Some(d) => assert_eq!(
            d["span"]["line"].as_u64(),
            Some(8),
            "variable \"env\" declaration is on line 8: {d}"
        ),
        None => panic!("expected diagnostic mentioning \"env\": {diags:?}"),
    }
}
