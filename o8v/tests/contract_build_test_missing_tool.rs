// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Contract tests for `8v build` and `8v test` when the underlying tool is missing or broken.
//!
//! Phase 2d — binary-boundary failure surface.
//!
//! FAILING-FIRST discipline applied:
//! - Tests that assert the missing-tool name appears in output are `#[ignore]`
//!   because `SpawnError.cause` is currently not surfaced to stdout/stderr.
//! - All other behavioral contracts are active.

use o8v_testkit::TempProject;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── Test 1: build with empty PATH reports non-zero exit ─────────────────────

/// Current behavior: exit 1 + "build failed" in stdout, but cargo NOT named in output.
/// The SpawnError cause ("No such file or directory") is swallowed — never written to
/// stdout/stderr. When that is fixed, remove the #[ignore] and enable the cargo assertion.
#[test]
#[ignore = "FIXME phase-3: SpawnError cause not surfaced — cargo not named in stderr"]
fn build_with_empty_path_reports_missing_tool() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v build");

    assert!(
        !out.status.success(),
        "should exit non-zero when cargo is missing"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("cargo"),
        "output should name the missing tool\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 2: build --json with missing tool emits valid JSON ─────────────────

/// Active part: JSON is valid and exit is non-zero.
/// Ignored part: error envelope fields (`code` + `error`) — current shape is the
/// success-path report with `success: false`, not a separate error envelope.
#[test]
fn build_json_with_missing_tool_exits_nonzero_with_valid_json() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v build --json");

    assert!(
        !out.status.success(),
        "should exit non-zero when cargo is missing"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON on missing-tool build: {e}\noutput: {stdout}"),
    };

    let success = parsed["success"].as_bool().unwrap_or(true);
    assert!(
        !success,
        "JSON success field should be false\nJSON: {parsed}"
    );
}

#[test]
#[ignore = "FIXME phase-3: SpawnError cause not surfaced — JSON error envelope (code+error) not emitted"]
fn build_json_with_missing_tool_emits_error_envelope() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v build --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(
        parsed.get("code").is_some(),
        "JSON error envelope should have 'code' field\nJSON: {parsed}"
    );
    assert!(
        parsed.get("error").is_some(),
        "JSON error envelope should have 'error' field\nJSON: {parsed}"
    );
}

// ─── Test 3: test with empty PATH reports non-zero exit ──────────────────────

/// Same as test 1 — SpawnError cause swallowed; cargo not named in output.
#[test]
#[ignore = "FIXME phase-3: SpawnError cause not surfaced — cargo not named in stderr"]
fn test_with_empty_path_reports_missing_tool() {
    let project = fixture("test-rust-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap()])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v test");

    assert!(
        !out.status.success(),
        "should exit non-zero when cargo is missing"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("cargo"),
        "output should name the missing tool\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 4: test --json with missing tool emits valid JSON ──────────────────

#[test]
fn test_json_with_missing_tool_exits_nonzero_with_valid_json() {
    let project = fixture("test-rust-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap(), "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v test --json");

    assert!(
        !out.status.success(),
        "should exit non-zero when cargo is missing"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON on missing-tool test: {e}\noutput: {stdout}"),
    };

    let success = parsed["success"].as_bool().unwrap_or(true);
    assert!(
        !success,
        "JSON success field should be false\nJSON: {parsed}"
    );
}

#[test]
#[ignore = "FIXME phase-3: SpawnError cause not surfaced — JSON error envelope (code+error) not emitted"]
fn test_json_with_missing_tool_emits_error_envelope() {
    let project = fixture("test-rust-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap(), "--json"])
        .env("PATH", "/nonexistent")
        .output()
        .expect("run 8v test --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    assert!(
        parsed.get("code").is_some(),
        "JSON error envelope should have 'code' field\nJSON: {parsed}"
    );
    assert!(
        parsed.get("error").is_some(),
        "JSON error envelope should have 'error' field\nJSON: {parsed}"
    );
}

// ─── Test 5: build on unsupported stack ──────────────────────────────────────

#[test]
fn build_on_unsupported_stack_reports_no_project_detected() {
    let project = fixture("build-no-tool");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on unsupported stack");

    assert!(
        !out.status.success(),
        "should exit non-zero for unsupported stack"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("has no build step"),
        "stderr should say 'has no build step'\nstderr: {stderr}"
    );
}

// ─── Test 6: build with broken Cargo.toml ────────────────────────────────────

#[test]
fn build_with_broken_manifest_reports_detection_error() {
    let project = fixture("build-rust-broken-toml");

    let out = bin()
        .args(["build", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v build on broken manifest");

    assert!(
        !out.status.success(),
        "should exit non-zero for broken manifest"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no project detected"),
        "stderr should say 'no project detected'\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("detection error"),
        "stderr should include 'detection error'\nstderr: {stderr}"
    );
}

// ─── Test 7: build timeout exceeded ──────────────────────────────────────────

/// Actual process timeout with `--timeout 1` is environment-dependent.
/// On a warm cache the build may finish in <1s; on CI it may hang. Ignoring until
/// a fixture that reliably exceeds 1s is added.
#[test]
#[ignore = "FIXME phase-3: no fixture that reliably exceeds 1s build time across all environments"]
fn build_timeout_exceeded() {
    let project = fixture("build-rust");

    let out = bin()
        .args(["build", project.path().to_str().unwrap(), "--timeout", "1"])
        .output()
        .expect("run 8v build --timeout 1");

    assert!(!out.status.success(), "should exit non-zero on timeout");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.to_lowercase().contains("timeout"),
        "output should mention timeout\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 8: test timeout exceeded ───────────────────────────────────────────

#[test]
#[ignore = "FIXME phase-3: no fixture that reliably exceeds 1s test time across all environments"]
fn test_timeout_exceeded() {
    let project = fixture("test-rust-pass");

    let out = bin()
        .args(["test", project.path().to_str().unwrap(), "--timeout", "1"])
        .output()
        .expect("run 8v test --timeout 1");

    assert!(!out.status.success(), "should exit non-zero on timeout");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.to_lowercase().contains("timeout"),
        "output should mention timeout\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 9: build on directory with no project ───────────────────────────────

#[test]
fn build_with_path_to_directory_with_no_project() {
    use std::fs;
    let dir = tempfile::tempdir().expect("create temp dir");
    // Add a file that cannot be detected as any stack.
    fs::write(dir.path().join("README.txt"), "nothing here").expect("write file");

    // 8v init is required; run it first so the workspace is available.
    let init_out = bin()
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v init");
    let init_stdout = String::from_utf8_lossy(&init_out.stdout);
    let init_stderr = String::from_utf8_lossy(&init_out.stderr);
    // init may succeed or already-exist — either is fine; we just need the workspace.
    let _ = (init_stdout, init_stderr);

    let out = bin()
        .args(["build", dir.path().to_str().unwrap()])
        .current_dir(dir.path())
        .output()
        .expect("run 8v build on empty dir");

    assert!(
        !out.status.success(),
        "should exit non-zero for directory with no project"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no project detected"),
        "stderr should say 'no project detected'\nstderr: {stderr}"
    );
}
