// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial violations tests for TypeScript, JavaScript, and .NET.
//!
//! Each test makes a specific falsifiable claim. Tests that pass both before
//! and after a regression prove nothing — every assertion here is tied to a
//! concrete fact about the fixture or the parser.
//!
//! Known bugs caught:
//! - .NET: MSBuild emits each error twice (compile + summary pass).
//!   Parser was producing 4 diagnostics for 2 errors. Fixed by seen-set dedup.

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

fn find_check<'a>(json: &'a serde_json::Value, stack: &str, tool: &str) -> &'a serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some(stack))
        .unwrap_or_else(|| panic!("{stack} result not found"))["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some(tool))
        .unwrap_or_else(|| panic!("{tool} check not found"))
}

// ─── TypeScript violations ───────────────────────────────────────────────────

#[test]
fn ts_violations_exits_1() {
    let project = fixture("check-typescript-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-typescript-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "tsc type errors must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ts_violations_tsc_outcome_failed() {
    let project = fixture("check-typescript-violations");
    let json = check_json(&project);
    let tsc = find_check(&json, "typescript", "tsc");
    assert_eq!(
        tsc["outcome"].as_str(),
        Some("failed"),
        "tsc must be 'failed' (not 'error') when it catches type errors: {tsc}"
    );
}

#[test]
fn ts_violations_exactly_two_diagnostics() {
    // broken.ts has exactly 2 type errors (line 1 and line 2).
    // Catches parser duplication or silent-drop regressions.
    let project = fixture("check-typescript-violations");
    let json = check_json(&project);
    let tsc = find_check(&json, "typescript", "tsc");
    let diags = tsc["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(
        diags.len(),
        2,
        "broken.ts has exactly 2 type errors — got: {diags:?}"
    );
}

#[test]
fn ts_violations_all_severity_error() {
    let project = fixture("check-typescript-violations");
    let json = check_json(&project);
    let tsc = find_check(&json, "typescript", "tsc");
    let diags = tsc["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "tsc diagnostic must be severity=error: {d}"
        );
    }
}

#[test]
fn ts_violations_first_error_line_1_type_mismatch() {
    // broken.ts line 1: `const x: number = "not a number"` — string not assignable to number.
    let project = fixture("check-typescript-violations");
    let json = check_json(&project);
    let tsc = find_check(&json, "typescript", "tsc");
    let diags = tsc["diagnostics"].as_array().expect("diagnostics");
    let first = &diags[0];
    assert_eq!(
        first["span"]["line"].as_u64(),
        Some(1),
        "first type error is on line 1: {first}"
    );
    assert!(
        first["message"]
            .as_str()
            .unwrap_or("")
            .contains("not assignable"),
        "first error must be a type assignment mismatch: {first}"
    );
}

#[test]
fn ts_violations_span_has_column() {
    // tsc reports column numbers — the parser must extract them.
    let project = fixture("check-typescript-violations");
    let json = check_json(&project);
    let tsc = find_check(&json, "typescript", "tsc");
    let diags = tsc["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        let col = d["span"]["column"].as_u64().unwrap_or(0);
        assert!(col > 0, "tsc diagnostic must have column > 0: {d}");
    }
}

// ─── JavaScript violations ───────────────────────────────────────────────────

#[test]
fn js_violations_exits_1() {
    let project = fixture("check-javascript-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-javascript-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "eslint errors must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn js_violations_eslint_outcome_failed() {
    let project = fixture("check-javascript-violations");
    let json = check_json(&project);
    let eslint = find_check(&json, "javascript", "eslint");
    assert_eq!(
        eslint["outcome"].as_str(),
        Some("failed"),
        "eslint must be 'failed' (not 'error') when it catches violations: {eslint}"
    );
}

#[test]
fn js_violations_exactly_three_diagnostics() {
    // broken.js: `unused` on line 1 (no-unused-vars) + `console` and `notDefined`
    // on line 2 (no-undef × 2). Total: 3.
    let project = fixture("check-javascript-violations");
    let json = check_json(&project);
    let eslint = find_check(&json, "javascript", "eslint");
    let diags = eslint["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(
        diags.len(),
        3,
        "broken.js has exactly 3 eslint errors — got: {diags:?}"
    );
}

#[test]
fn js_violations_has_unused_vars_rule() {
    // broken.js line 1: `const unused = 42` — no-unused-vars.
    let project = fixture("check-javascript-violations");
    let json = check_json(&project);
    let eslint = find_check(&json, "javascript", "eslint");
    let diags = eslint["diagnostics"].as_array().expect("diagnostics");
    let has_rule = diags.iter().any(|d| {
        d["rule"]
            .as_str()
            .map(|r| r.contains("no-unused-vars"))
            .unwrap_or(false)
    });
    assert!(
        has_rule,
        "expected no-unused-vars rule in diagnostics: {diags:?}"
    );
}

#[test]
fn js_violations_has_undef_rule() {
    // broken.js line 2: `notDefined` — no-undef.
    let project = fixture("check-javascript-violations");
    let json = check_json(&project);
    let eslint = find_check(&json, "javascript", "eslint");
    let diags = eslint["diagnostics"].as_array().expect("diagnostics");
    let has_rule = diags.iter().any(|d| {
        d["rule"]
            .as_str()
            .map(|r| r.contains("no-undef"))
            .unwrap_or(false)
    });
    assert!(has_rule, "expected no-undef rule in diagnostics: {diags:?}");
}

#[test]
fn js_violations_all_severity_error() {
    let project = fixture("check-javascript-violations");
    let json = check_json(&project);
    let eslint = find_check(&json, "javascript", "eslint");
    let diags = eslint["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "eslint diagnostic must be severity=error (rules set to 'error'): {d}"
        );
    }
}

// ─── .NET violations ─────────────────────────────────────────────────────────

#[test]
fn dotnet_violations_exits_1() {
    let project = fixture("check-dotnet-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-dotnet-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "dotnet build errors must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dotnet_violations_outcome_failed() {
    let project = fixture("check-dotnet-violations");
    let json = check_json(&project);
    let build = find_check(&json, "dotnet", "dotnet build");
    assert_eq!(
        build["outcome"].as_str(),
        Some("failed"),
        "dotnet build must be 'failed' (not 'error') on compile errors: {build}"
    );
}

#[test]
fn dotnet_violations_exactly_two_diagnostics() {
    // BUG REGRESSION: MSBuild emits each error twice (compile pass + summary pass).
    // The parser was producing 4 diagnostics for 2 errors. Dedup fix collapses them.
    let project = fixture("check-dotnet-violations");
    let json = check_json(&project);
    let build = find_check(&json, "dotnet", "dotnet build");
    let diags = build["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(
        diags.len(),
        2,
        "Broken.cs has exactly 2 compile errors — parser must not duplicate them.\n\
         If this fails with 4 diagnostics, the MSBuild dedup fix is missing.\n\
         Got: {diags:?}"
    );
}

#[test]
fn dotnet_violations_cs0029_rule() {
    // Program.cs line 2: `int x = "not an int"` — CS0029 cannot convert string to int.
    let project = fixture("check-dotnet-violations");
    let json = check_json(&project);
    let build = find_check(&json, "dotnet", "dotnet build");
    let diags = build["diagnostics"].as_array().expect("diagnostics");
    let has_cs0029 = diags
        .iter()
        .any(|d| d["rule"].as_str().map(|r| r == "CS0029").unwrap_or(false));
    assert!(
        has_cs0029,
        "expected CS0029 (implicit type conversion error) in diagnostics: {diags:?}"
    );
}

#[test]
fn dotnet_violations_span_has_line_and_column() {
    // MSBuild provides (line,col) — the parser must extract both.
    let project = fixture("check-dotnet-violations");
    let json = check_json(&project);
    let build = find_check(&json, "dotnet", "dotnet build");
    let diags = build["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        let line = d["span"]["line"].as_u64().unwrap_or(0);
        let col = d["span"]["column"].as_u64().unwrap_or(0);
        assert!(line > 0, "diagnostic must have line > 0: {d}");
        assert!(
            col > 0,
            "diagnostic must have column > 0 (MSBuild provides it): {d}"
        );
    }
}

#[test]
fn dotnet_violations_all_severity_error() {
    let project = fixture("check-dotnet-violations");
    let json = check_json(&project);
    let build = find_check(&json, "dotnet", "dotnet build");
    let diags = build["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "MSBuild compile diagnostic must be severity=error: {d}"
        );
    }
}
