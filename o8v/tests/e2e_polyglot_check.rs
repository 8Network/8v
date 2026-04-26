// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Polyglot E2E coverage — one test per stack.
//!
//! Contract: `8v check --json <fixture>` must:
//! 1. Exit 0 (pass) or 1 (fail) — never crash or return unexpected exit code
//! 2. Produce valid JSON
//! 3. Have at least one entry in `results` with the expected stack label

use o8v_testkit::{fixture_path, Fixture, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn check_stack_detected(path: &std::path::Path, expected_stack: &str) {
    let out = match bin()
        .args(["check", "--json", path.to_str().unwrap()])
        .output()
    {
        Ok(o) => o,
        Err(e) => panic!("failed to spawn 8v check for {expected_stack}: {e}"),
    };

    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "{expected_stack}: expected exit 0 or 1, got {code}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("{expected_stack}: invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = match json["results"].as_array() {
        Some(r) => r,
        None => panic!("{expected_stack}: missing results array\njson: {json}"),
    };

    assert!(
        !results.is_empty(),
        "{expected_stack}: project not detected (results is empty)\nstdout: {stdout}"
    );

    let detected_stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();

    assert!(
        detected_stacks.contains(&expected_stack),
        "{expected_stack}: stack not in results (found: {detected_stacks:?})\nstdout: {stdout}"
    );
}

// ─── Corpus fixtures ────────────────────────────────────────────────────────

#[test]
fn check_go_stack_detected() {
    let f = Fixture::corpus("go-service");
    check_stack_detected(f.path(), "go");
}

#[test]
fn check_python_stack_detected() {
    let f = Fixture::corpus("python-uv-workspace");
    check_stack_detected(f.path(), "python");
}

#[test]
fn check_typescript_stack_detected() {
    let f = Fixture::corpus("typescript-workspace");
    check_stack_detected(f.path(), "typescript");
}

#[test]
fn check_javascript_stack_detected() {
    let f = Fixture::corpus("javascript-workspace");
    check_stack_detected(f.path(), "javascript");
}

#[test]
fn check_deno_stack_detected() {
    let f = Fixture::corpus("deno-workspace");
    check_stack_detected(f.path(), "deno");
}

#[test]
fn check_dotnet_stack_detected() {
    let f = Fixture::corpus("dotnet-slnx-solution");
    check_stack_detected(f.path(), "dotnet");
}

// ─── Isolated fixtures ──────────────────────────────────────────────────────

#[test]
fn check_kotlin_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-kotlin"));
    check_stack_detected(project.path(), "kotlin");
}

#[test]
fn check_swift_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-swift"));
    check_stack_detected(project.path(), "swift");
}

#[test]
fn check_ruby_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-ruby"));
    check_stack_detected(project.path(), "ruby");
}

#[test]
fn check_erlang_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-erlang"));
    check_stack_detected(project.path(), "erlang");
}

#[test]
fn check_terraform_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-terraform"));
    check_stack_detected(project.path(), "terraform");
}

#[test]
fn check_dockerfile_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-dockerfile"));
    check_stack_detected(project.path(), "dockerfile");
}

#[test]
fn check_helm_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-helm"));
    check_stack_detected(project.path(), "helm");
}

#[test]
fn check_kustomize_stack_detected() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-kustomize"));
    check_stack_detected(project.path(), "kustomize");
}

// ─── JSON contract for each detected stack ──────────────────────────────────
//
// Verify the `checks` array is present and each check has at least
// `name` and `outcome` fields.

fn check_json_structure(path: &std::path::Path, expected_stack: &str) {
    let out = match bin()
        .args(["check", "--json", path.to_str().unwrap()])
        .output()
    {
        Ok(o) => o,
        Err(e) => panic!("spawn failed for {expected_stack}: {e}"),
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("{expected_stack}: invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = json["results"].as_array().expect("results array");
    let result = match results
        .iter()
        .find(|r| r["stack"].as_str() == Some(expected_stack))
    {
        Some(r) => r,
        None => panic!("{expected_stack} not in results"),
    };

    let checks = match result["checks"].as_array() {
        Some(c) => c,
        None => panic!("{expected_stack}: checks array missing"),
    };

    assert!(
        !checks.is_empty(),
        "{expected_stack}: checks array is empty — no tools ran"
    );

    for check in checks {
        assert!(
            check["name"].is_string(),
            "{expected_stack}: check entry missing 'name' field: {check}"
        );
        assert!(
            check["outcome"].is_string(),
            "{expected_stack}: check entry missing 'outcome' field: {check}"
        );
    }
}

#[test]
fn go_json_checks_structure() {
    let f = Fixture::corpus("go-service");
    check_json_structure(f.path(), "go");
}

#[test]
fn python_json_checks_structure() {
    let f = Fixture::corpus("python-uv-workspace");
    check_json_structure(f.path(), "python");
}

#[test]
fn typescript_json_checks_structure() {
    let f = Fixture::corpus("typescript-workspace");
    check_json_structure(f.path(), "typescript");
}

#[test]
fn dotnet_json_checks_structure() {
    let f = Fixture::corpus("dotnet-slnx-solution");
    check_json_structure(f.path(), "dotnet");
}

#[test]
fn kotlin_json_checks_structure() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-kotlin"));
    check_json_structure(project.path(), "kotlin");
}

#[test]
fn terraform_json_checks_structure() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-terraform"));
    check_json_structure(project.path(), "terraform");
}

#[test]
fn dockerfile_json_checks_structure() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-dockerfile"));
    check_json_structure(project.path(), "dockerfile");
}

#[test]
fn helm_json_checks_structure() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-helm"));
    check_json_structure(project.path(), "helm");
}

#[test]
fn kustomize_json_checks_structure() {
    let project = TempProject::from_fixture(&fixture_path("o8v", "check-kustomize"));
    check_json_structure(project.path(), "kustomize");
}
