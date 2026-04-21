// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for Swift, Ruby, and Erlang stacks.
//!
//! Each test probes actual behavior observed on the host system, so they
//! assert exit-code contracts and JSON structure rather than specific tool
//! output (toolchains may or may not be installed).

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    TempProject::from_fixture(&fixture_path("o8v", name))
}

// ─── Swift ──────────────────────────────────────────────────────────────────

#[test]
fn swift_check_exits_0_or_1() {
    let project = fixture("check-swift");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-swift");
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on swift project must exit 0 or 1, got {code}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn swift_check_json_has_swiftlint_entry() {
    let project = fixture("check-swift");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-swift");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let swift_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("swift"))
        .expect("swift result not found");

    let checks = swift_result["checks"].as_array().expect("checks array");
    let swiftlint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("swiftlint"))
        .expect("swiftlint entry not found");

    let outcome = swiftlint["outcome"].as_str().expect("outcome field");
    assert!(
        outcome == "passed" || outcome == "failed" || outcome == "error",
        "unexpected outcome: {outcome}"
    );
}

#[test]
fn swift_check_json_swiftlint_no_invalid_flag_error() {
    // Regression: swiftlint 0.63.2 has no --exclude flag.
    // If the wrong args are passed, outcome="error" with cause containing "Unknown option".
    let project = fixture("check-swift");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-swift");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let swift_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("swift"))
        .expect("swift result");

    let checks = swift_result["checks"].as_array().expect("checks array");
    let swiftlint = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("swiftlint"))
        .expect("swiftlint entry");

    let cause = swiftlint["cause"].as_str().unwrap_or("");
    assert!(
        !cause.contains("Unknown option"),
        "swiftlint reported unknown flag — invalid CLI args passed: {cause}"
    );
}

#[test]
fn swift_build_exits_0() {
    let project = fixture("check-swift");
    let out = bin()
        .args(["build", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-swift");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(
        parsed["exit_code"].as_i64().unwrap_or(-1),
        0,
        "swift build should succeed on clean project\nstderr: {}",
        parsed["stderr"].as_str().unwrap_or("")
    );
}

#[test]
fn swift_test_json_has_stack_field() {
    let project = fixture("check-swift");
    let out = bin()
        .args(["test", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test --json on check-swift");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(
        parsed["stack"].as_str(),
        Some("swift"),
        "test output must include stack=swift"
    );
    assert!(
        parsed["exit_code"].is_number(),
        "test output must include exit_code"
    );
}

// ─── Ruby ───────────────────────────────────────────────────────────────────

#[test]
fn ruby_check_exits_0_or_1() {
    let project = fixture("check-ruby");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-ruby");
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on ruby project must exit 0 or 1, got {code}"
    );
}

#[test]
fn ruby_check_json_valid() {
    let project = fixture("check-ruby");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-ruby");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(
        !results.is_empty(),
        "ruby check must produce at least one result"
    );

    let ruby_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("ruby"))
        .expect("ruby result not found in check output");

    let checks = ruby_result["checks"].as_array().expect("checks array");
    let rubocop = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("rubocop"))
        .expect("rubocop entry not found");

    // rubocop may not be installed — outcome can be "error" with NotFound cause,
    // or "passed" / "failed" if installed. All are valid.
    assert!(
        rubocop["outcome"].is_string(),
        "rubocop entry must have an outcome field"
    );
}

#[test]
fn ruby_check_json_rubocop_not_found_is_error_not_crash() {
    // When rubocop is absent the tool must NOT crash — it must produce valid JSON
    // with outcome="error" and a human-readable cause containing "NotFound".
    let project = fixture("check-ruby");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-ruby");

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check must not crash when rubocop is absent, got exit {code}"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
        "output must be valid JSON even when rubocop is absent\noutput: {stdout}"
    );
}

// ─── Erlang ─────────────────────────────────────────────────────────────────

#[test]
fn erlang_check_exits_0_or_1() {
    let project = fixture("check-erlang");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-erlang");
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v check on erlang project must exit 0 or 1, got {code}"
    );
}

#[test]
fn erlang_xref_outcome_not_error_on_clean_project() {
    // Regression: rebar3 writes ANSI-colored progress to stdout, which previously
    // caused has_unparseable_lines=true → ParseStatus::Unparsed → outcome="error".
    let project = fixture("check-erlang");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-erlang");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let erlang_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("erlang"))
        .expect("erlang result not found");

    let checks = erlang_result["checks"].as_array().expect("checks array");
    let xref = checks
        .iter()
        .find(|c| c["name"].as_str() == Some("rebar3 xref"))
        .expect("rebar3 xref check not found");

    let outcome = xref["outcome"].as_str().expect("outcome field");
    assert_ne!(
        outcome, "error",
        "rebar3 xref must not report error on a clean project (ANSI strip regression)"
    );
}

#[test]
fn erlang_build_exits_0() {
    let project = fixture("check-erlang");
    let out = bin()
        .args(["build", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on check-erlang");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(
        parsed["exit_code"].as_i64().unwrap_or(-1),
        0,
        "erlang build should succeed on clean project\nstderr: {}",
        parsed["stderr"].as_str().unwrap_or("")
    );
}

#[test]
fn erlang_test_exits_0() {
    let project = fixture("check-erlang");
    let out = bin()
        .args(["test", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v test on check-erlang");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(
        parsed["exit_code"].as_i64().unwrap_or(-1),
        0,
        "erlang test (rebar3 eunit) should exit 0 on clean project\nstdout: {stdout}"
    );
}

#[test]
fn erlang_check_json_has_rebar3_entries() {
    let project = fixture("check-erlang");
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-erlang");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let erlang_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("erlang"))
        .expect("erlang result");

    let checks = erlang_result["checks"].as_array().expect("checks array");
    let names: Vec<&str> = checks.iter().filter_map(|c| c["name"].as_str()).collect();

    assert!(
        names.iter().any(|n| n.starts_with("rebar3")),
        "erlang checks must include at least one rebar3 tool, got: {names:?}"
    );
}

// ─── Ruby violations ─────────────────────────────────────────────────────────
//
// Fixture: check-ruby-violations/
//   broken.rb — 3 rubocop offenses (error severity):
//     line 1  Lint/UselessAssignment   — `x = 1` is never read
//     line 1  Style/FrozenStringLiteralComment — missing magic comment
//     line 2  Style/StringLiterals     — double-quoted string "hello"
//
// These tests require rubocop on PATH.  When rubocop is absent, outcome="error"
// (not "failed") and all diagnostic-count assertions are moot — each test
// guards against that case explicitly.

fn ruby_violations_rubocop_check(project: &TempProject) -> serde_json::Value {
    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-ruby-violations");
    let stdout = String::from_utf8_lossy(&out.stdout);
    match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    }
}

fn ruby_violations_find_rubocop(json: &serde_json::Value) -> &serde_json::Value {
    json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["stack"].as_str() == Some("ruby"))
        .expect("ruby result not found")["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"].as_str() == Some("rubocop"))
        .expect("rubocop check not found")
}

#[test]
fn ruby_violations_exits_1() {
    let project = fixture("check-ruby-violations");
    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-ruby-violations");
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "rubocop violations must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ruby_violations_outcome_failed() {
    let project = fixture("check-ruby-violations");
    let json = ruby_violations_rubocop_check(&project);
    let rubocop = ruby_violations_find_rubocop(&json);
    let outcome = rubocop["outcome"].as_str().unwrap_or("missing");
    // When rubocop is absent the outcome is "error", not "failed".
    // The test is meaningful only when rubocop ran successfully.
    if outcome == "error" {
        let cause = rubocop["cause"].as_str().unwrap_or("");
        assert!(
            cause.contains("NotFound") || cause.contains("not found"),
            "outcome=error but cause is unexpected: {cause}"
        );
        return; // rubocop not installed — skip the behavioral assertion
    }
    assert_eq!(
        outcome, "failed",
        "rubocop must report 'failed' (not 'error') when violations exist: {rubocop}"
    );
}

#[test]
fn ruby_violations_exactly_three_diagnostics() {
    // broken.rb has exactly 3 violations. Catches parser duplication or silent-drop.
    let project = fixture("check-ruby-violations");
    let json = ruby_violations_rubocop_check(&project);
    let rubocop = ruby_violations_find_rubocop(&json);
    if rubocop["outcome"].as_str() == Some("error") {
        return; // rubocop not installed
    }
    let diags = rubocop["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(
        diags.len(),
        3,
        "broken.rb has exactly 3 rubocop violations — got: {diags:?}"
    );
}

#[test]
fn ruby_violations_has_frozen_string_literal_cop() {
    // broken.rb is missing the `# frozen_string_literal: true` magic comment.
    let project = fixture("check-ruby-violations");
    let json = ruby_violations_rubocop_check(&project);
    let rubocop = ruby_violations_find_rubocop(&json);
    if rubocop["outcome"].as_str() == Some("error") {
        return; // rubocop not installed
    }
    let diags = rubocop["diagnostics"].as_array().expect("diagnostics");
    let has_cop = diags.iter().any(|d| {
        d["rule"]
            .as_str()
            .map(|r| r == "Style/FrozenStringLiteralComment")
            .unwrap_or(false)
    });
    assert!(
        has_cop,
        "expected Style/FrozenStringLiteralComment in diagnostics: {diags:?}"
    );
}

#[test]
fn ruby_violations_all_severity_error() {
    // .rubocop.yml sets Severity: error for all three cops.
    let project = fixture("check-ruby-violations");
    let json = ruby_violations_rubocop_check(&project);
    let rubocop = ruby_violations_find_rubocop(&json);
    if rubocop["outcome"].as_str() == Some("error") {
        return; // rubocop not installed
    }
    let diags = rubocop["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        assert_eq!(
            d["severity"].as_str(),
            Some("error"),
            "rubocop diagnostic must be severity=error (cops set to Severity: error): {d}"
        );
    }
}

#[test]
fn ruby_violations_span_has_line() {
    // rubocop provides start_line — the parser must extract it.
    let project = fixture("check-ruby-violations");
    let json = ruby_violations_rubocop_check(&project);
    let rubocop = ruby_violations_find_rubocop(&json);
    if rubocop["outcome"].as_str() == Some("error") {
        return; // rubocop not installed
    }
    let diags = rubocop["diagnostics"].as_array().expect("diagnostics");
    for d in diags {
        let line = d["span"]["line"].as_u64().unwrap_or(0);
        assert!(line > 0, "diagnostic must have span.line > 0: {d}");
    }
}
