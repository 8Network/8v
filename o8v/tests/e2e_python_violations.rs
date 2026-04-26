// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial violations tests for Python (ruff).
//!
//! Each test makes a specific falsifiable claim. Tests that pass both before
//! and after a regression prove nothing — every assertion here is tied to a
//! concrete fact about the fixture or the parser.
//!
//! Fixture: o8v/tests/fixtures/check-python-violations/src/bad.py
//!   Line 1: `import os`   — F401 (`os` imported but unused)
//!   Line 2: `import sys`  — F401 (`sys` imported but unused)
//!   Total: exactly 2 ruff diagnostics.
//!
//! Clean fixture: o8v/tests/fixtures/check-python-clean/
//!   All checks pass, zero diagnostics.

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

fn ruff_check(json: &serde_json::Value) -> &serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("python"))
        .expect("python result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("ruff"))
        .expect("ruff check not found")
}

// ─── violations: exit code ───────────────────────────────────────────────────

#[test]
fn python_violations_exits_1() {
    // ruff finds errors → process must exit 1.
    // Catches outcome-to-exit-code mapping failures.
    let project = fixture("check-python-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-python-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "ruff violations must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── violations: outcome field ───────────────────────────────────────────────

#[test]
fn python_violations_ruff_outcome_failed() {
    // outcome must be "failed" (tool ran and caught errors), not "error" (tool
    // crashed) and not "passed" (no violations found). The distinction matters
    // for downstream consumers of the JSON envelope.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    assert_eq!(
        ruff["outcome"].as_str(),
        Some("failed"),
        "ruff outcome must be 'failed' when violations exist, not 'error' or 'passed': {ruff}"
    );
}

// ─── violations: exact diagnostic count ─────────────────────────────────────

#[test]
fn python_violations_exactly_two_diagnostics() {
    // bad.py has exactly 2 F401 violations (lines 1 and 2).
    // This test catches both duplication (parser emitting each error twice)
    // and silent-drop regressions (parser swallowing one of the errors).
    // If this fails with 4, the ruff parser is duplicating output.
    // If this fails with 1, the parser is silently dropping one diagnostic.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        2,
        "bad.py has exactly 2 ruff violations — parser must not duplicate or drop them.\n\
         If 4: parser is duplicating (each JSON entry emitted twice).\n\
         If 1: parser is silently dropping one diagnostic.\n\
         Got: {diags:?}"
    );
}

// ─── violations: rule code ───────────────────────────────────────────────────

#[test]
fn python_violations_has_f401_rule() {
    // Both violations in bad.py are F401 (unused import).
    // If this fails, the parser is not extracting the ruff rule code from
    // the JSON "code" field.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    let has_f401 = diags.iter().any(|d| d["rule"].as_str() == Some("F401"));
    assert!(
        has_f401,
        "expected at least one F401 (unused import) diagnostic: {diags:?}"
    );
}

// ─── violations: severity ────────────────────────────────────────────────────

#[test]
fn python_violations_all_severity_error() {
    // ruff emits F401 as severity "error". The parser must not downgrade them
    // to "warning" or leave severity empty.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "ruff F401 diagnostic must be severity=error: {d}"
        );
    }
}

// ─── violations: span ────────────────────────────────────────────────────────

#[test]
fn python_violations_span_has_line() {
    // ruff JSON output includes location.row — the parser must map it to
    // span.line. A zero line number means the parser failed to extract
    // the location.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    for d in diags {
        let line = d["span"]["line"].as_u64().unwrap_or(0);
        assert!(
            line > 0,
            "ruff diagnostic must have span.line > 0 (ruff provides location.row): {d}"
        );
    }
}

// ─── violations: first diagnostic is on line 1 ───────────────────────────────

#[test]
fn python_violations_first_diagnostic_line_1() {
    // bad.py line 1: `import os` — first F401.
    // Validates that the span is preserved end-to-end from ruff JSON to
    // the 8v diagnostic model.
    let project = fixture("check-python-violations");
    let json = check_json(&project);
    let ruff = ruff_check(&json);
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    let first = &diags[0];
    assert_eq!(
        first["span"]["line"].as_u64(),
        Some(1),
        "first F401 violation is on line 1 (import os): {first}"
    );
}

// ─── clean project: exit code ────────────────────────────────────────────────

#[test]
fn python_clean_exits_0() {
    // A clean Python project must exit 0.
    // Catches false-positive regressions where ruff or mypy flags clean code.
    let project = fixture("check-python-clean");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-python-clean");
    assert_eq!(
        out.status.code().unwrap_or(99),
        0,
        "clean python project must exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── clean project: zero diagnostics ─────────────────────────────────────────

#[test]
fn python_clean_zero_diagnostics() {
    // ruff on the clean fixture must produce zero diagnostics.
    // Catches false-positive bugs where clean code is flagged.
    let project = fixture("check-python-clean");
    let json = check_json(&project);
    let ruff = json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("python"))
        .expect("python result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("ruff"))
        .expect("ruff check not found");
    let diags = ruff["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(
        diags.len(),
        0,
        "clean python project must have zero ruff diagnostics: {diags:?}"
    );
}
