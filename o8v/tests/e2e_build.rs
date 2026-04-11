// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v build` — runs the compiled binary against fixture projects.

use o8v_testkit::TempProject;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── Rust builds ────────────────────────────────────────────────────────────

#[test]
fn build_rust_project_succeeds() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v build should exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("cargo build"),
        "should show cargo build command: {stdout}"
    );
    assert!(
        stdout.contains("exit: 0 (success)"),
        "should show success: {stdout}"
    );
    assert!(
        stdout.contains("duration:"),
        "should show duration: {stdout}"
    );
}

#[test]
fn build_rust_json_has_required_fields() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v build --json");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(parsed.get("command").is_some(), "missing command field");
    assert!(parsed.get("exit_code").is_some(), "missing exit_code field");
    assert!(parsed.get("stdout").is_some(), "missing stdout field");
    assert!(parsed.get("stderr").is_some(), "missing stderr field");
    assert!(
        parsed.get("duration_ms").is_some(),
        "missing duration_ms field"
    );
    assert!(parsed.get("truncated").is_some(), "missing truncated field");
    assert!(parsed.get("stack").is_some(), "missing stack field");
    assert!(
        parsed.get("detection_errors").is_some(),
        "missing detection_errors field"
    );

    assert_eq!(parsed["stack"], "rust", "stack should be rust");
    assert_eq!(parsed["exit_code"], 0, "exit_code should be 0");

    let truncated = parsed.get("truncated").unwrap();
    assert!(
        truncated.get("stdout").is_some(),
        "truncated missing stdout"
    );
    assert!(
        truncated.get("stderr").is_some(),
        "truncated missing stderr"
    );
}

// ─── Go builds ──────────────────────────────────────────────────────────────

#[test]
fn build_go_project_succeeds() {
    let project = fixture("build-go");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on go project");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v build should exit 0 for go project\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("go build"),
        "should show go build command: {stdout}"
    );
    assert!(
        stdout.contains("exit: 0 (success)"),
        "should show success: {stdout}"
    );
}

#[test]
fn build_go_json_shows_go_stack() {
    let project = fixture("build-go");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v build --json on go");

    assert!(out.status.success(), "should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "go", "stack should be go");
}

// ─── No build tool ──────────────────────────────────────────────────────────

#[test]
fn build_python_project_errors_no_build_tool() {
    let project = fixture("build-no-tool");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on python");

    assert!(
        !out.status.success(),
        "should fail — python has no build tool"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no build tool"),
        "should say no build tool: {stderr}"
    );
}

// ─── No project ─────────────────────────────────────────────────────────────

#[test]
fn build_empty_dir_errors_no_project() {
    let project = TempProject::empty();

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on empty dir");

    assert!(!out.status.success(), "should fail — no project detected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no project detected"),
        "should say no project: {stderr}"
    );
}

// ─── Invalid path ───────────────────────────────────────────────────────────

#[test]
fn build_nonexistent_path_errors() {
    let out = bin()
        .args(["build", "/nonexistent-path-xyz-123"])
        .output()
        .expect("run 8v build on nonexistent");

    assert!(!out.status.success(), "should fail for nonexistent path");
}

// ─── Broken builds ──────────────────────────────────────────────────────────

#[test]
fn build_rust_broken_fails_with_compile_error() {
    let project = fixture("build-rust-broken");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on broken rust project");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v build should exit non-zero for broken rust project\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("cargo build"),
        "should show cargo build command: {stdout}"
    );
    assert!(
        stdout.contains("exit: 101 (failed)") || stdout.contains("failed"),
        "should show failed exit: {stdout}"
    );
    assert!(
        stdout.contains("mismatched types") || stdout.contains("E0308"),
        "should contain rust type error in output: {stdout}"
    );
}

#[test]
fn build_rust_broken_json_has_nonzero_exit_code() {
    let project = fixture("build-rust-broken");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v build --json on broken rust project");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "rust", "stack should be rust");
    assert_ne!(parsed["exit_code"], 0, "exit_code should be non-zero");
    let stderr_field = parsed["stderr"].as_str().unwrap_or("");
    assert!(
        stderr_field.contains("mismatched types") || stderr_field.contains("E0308"),
        "stderr field should contain compile error: {stderr_field}"
    );
}

#[test]
fn build_go_broken_fails_with_compile_error() {
    let project = fixture("build-go-broken");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on broken go project");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v build should exit non-zero for broken go project\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("go build"),
        "should show go build command: {stdout}"
    );
    assert!(
        stdout.contains("failed"),
        "should show failed exit: {stdout}"
    );
    assert!(
        stdout.contains("invalid operation") || stdout.contains("mismatched types"),
        "should contain go type error in output: {stdout}"
    );
}

#[test]
fn build_go_broken_json_has_nonzero_exit_code() {
    let project = fixture("build-go-broken");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v build --json on broken go project");

    assert!(!out.status.success(), "should exit non-zero");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert_eq!(parsed["stack"], "go", "stack should be go");
    assert_ne!(parsed["exit_code"], 0, "exit_code should be non-zero");
    let stderr_field = parsed["stderr"].as_str().unwrap_or("");
    assert!(
        stderr_field.contains("invalid operation") || stderr_field.contains("mismatched types"),
        "stderr field should contain compile error: {stderr_field}"
    );
}

// ─── Timeout cap ────────────────────────────────────────────────────────────

#[test]
fn build_timeout_cap_enforced() {
    let project = fixture("build-rust");

    let out = bin()
        .args([
            "build",
            project.path().to_str().unwrap(),
            "--timeout",
            "999",
        ])
        .output()
        .expect("run 8v build with excessive timeout");

    assert!(!out.status.success(), "should fail with timeout too large");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exceeds maximum"),
        "should mention max: {stderr}"
    );
}
