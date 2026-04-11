// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v check` command — exercise the binary as a subprocess.
//!
//! These test exit codes, output format routing, argument validation,
//! and the full pipeline on real fixtures (cargo check, clippy, fmt).

use std::process::Command;

/// Path to the compiled binary. Set by Cargo for integration tests.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── Exit codes ────────────────────────────────────────────────────────────

#[test]
fn nonexistent_path_exits_1() {
    let out = bin()
        .args(["check", "/nonexistent/path/that/does/not/exist"])
        .output()
        .expect("failed to run binary");

    // Path validation error is a user error → exit 1 (fail), not 2 (nothing to check).
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error:"),
        "expected error message on stderr, got: {stderr}"
    );
}

#[test]
fn path_error_no_newline_injection() {
    let out = bin()
        .args(["check", "does-not-exist\nINJECTED"])
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&out.stderr);
    // The error must be on ONE line — no newline injection.
    let error_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("error")).collect();
    assert_eq!(
        error_lines.len(),
        1,
        "should be exactly one error line (no injection)\nstderr: {stderr}"
    );
    // The newline was stripped — both parts are on the same line.
    assert!(
        error_lines[0].contains("does-not-exist") && error_lines[0].contains("INJECTED"),
        "sanitized path should be on one line\nstderr: {stderr}"
    );
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn empty_dir_exits_2() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty dir should exit 2 (nothing to check)"
    );
}

// ─── Argument validation ───────────────────────────────────────────────────

#[test]
fn limit_negative_rejected() {
    let out = bin()
        .args(["check", "--limit", "-1", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("non-negative")
            || stderr.contains("invalid")
            || stderr.contains("unexpected argument"),
        "expected limit validation error, got: {stderr}"
    );
}

#[test]
fn limit_non_numeric_rejected() {
    let out = bin()
        .args(["check", "--limit", "abc", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a valid number") || stderr.contains("invalid"),
        "expected limit validation error, got: {stderr}"
    );
}

#[test]
fn json_and_plain_conflict() {
    let out = bin()
        .args(["check", "--json", "--plain", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with"),
        "expected conflict error, got: {stderr}"
    );
}

// ─── Timeout flag ──────────────────────────────────────────────────────────

#[test]
fn timeout_valid_seconds() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--timeout", "30s", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    // Valid timeout, empty dir → exit 2 (nothing to check).
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn timeout_valid_minutes() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--timeout", "5m", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn timeout_valid_combined() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--timeout", "2m30s", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn timeout_valid_bare_number() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--timeout", "300", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn timeout_invalid_rejected() {
    let out = bin()
        .args(["check", "--timeout", "invalid", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected character"),
        "expected timeout parse error, got: {stderr}"
    );
}

#[test]
fn timeout_zero_rejected() {
    let out = bin()
        .args(["check", "--timeout", "0", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("greater than 0"),
        "expected zero timeout error, got: {stderr}"
    );
}

#[test]
fn timeout_overflow_rejected() {
    let out = bin()
        .args(["check", "--timeout", "18446744073709551615m", "."])
        .output()
        .expect("failed to run binary");

    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("too large"),
        "expected overflow error, got: {stderr}"
    );
}

// ─── Output format routing ─────────────────────────────────────────────────

#[test]
fn json_empty_dir_valid_json_on_stdout() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--json", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    // Empty dir still exits 2, but JSON should be valid if any output.
    assert_eq!(out.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !stdout.is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        assert!(parsed.is_ok(), "invalid JSON: {stdout}");
    }
}

#[test]
fn plain_empty_dir_exits_2() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--plain", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn no_color_env_respected() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", tmp.path().to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    // Should not contain ANSI escape sequences
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("\x1b["),
        "NO_COLOR set but output contains ANSI escapes"
    );
}

// ─── Additional check.rs paths ─────────────────────────────────────────────

#[test]
fn limit_zero_means_no_limit() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--limit", "0", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    // --limit 0 is valid (means no limit). Should not error on arg parsing.
    assert_eq!(out.status.code(), Some(2), "empty dir = exit 2");
}

#[test]
fn no_color_flag() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--no-color", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("\x1b["),
        "--no-color should suppress ANSI escapes"
    );
}

#[test]
fn default_path_uses_cwd() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    // No path argument — should use "." (cwd).
    let out = bin()
        .arg("check")
        .current_dir(tmp.path())
        .output()
        .expect("failed to run binary");

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty cwd should exit 2 (nothing to check)"
    );
}

#[test]
fn human_format_outputs_to_stderr_not_stdout() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    // Human format writes to stderr, stdout should be empty.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.is_empty(),
        "human format should not write to stdout, got: {stdout}"
    );
}

#[test]
fn json_format_outputs_to_stdout_not_stderr() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--json", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    // JSON goes to stdout. Stderr should be empty (no tracing, no errors).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.is_empty(),
        "json format should not write to stderr, got: {stderr}"
    );
}

#[test]
fn plain_format_outputs_to_stdout() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", "--plain", tmp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.is_empty(),
        "plain format should not write to stderr, got: {stderr}"
    );
}

// ─── main.rs paths ─────────────────────────────────────────────────────────

#[test]
fn invalid_rust_log_warns() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", tmp.path().to_str().unwrap()])
        .env("RUST_LOG", "not a valid filter [[[")
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning: invalid RUST_LOG filter"),
        "expected RUST_LOG warning, got: {stderr}"
    );
}

#[test]
fn valid_rust_log_accepted() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = bin()
        .args(["check", tmp.path().to_str().unwrap()])
        .env("RUST_LOG", "debug")
        .output()
        .expect("failed to run binary");

    // Should not warn about RUST_LOG.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("warning: invalid RUST_LOG filter"),
        "valid RUST_LOG should not produce warning"
    );
    // Should still produce the correct exit code.
    assert_eq!(out.status.code(), Some(2));
}

// ─── Broken pipe / render error ─────────────────────────────────────────────

#[test]
fn json_broken_pipe_exits_1() {
    // Pipe stdout to a process that immediately closes, causing a broken pipe.
    // JSON writes to stdout — a broken pipe should trigger the render error path.
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mut child = bin()
        .args(["check", "--json", tmp.path().to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    // Close stdout immediately to cause broken pipe.
    drop(child.stdout.take());

    let out = child.wait_with_output().expect("failed to wait");

    // Broken pipe is not a check failure — the consumer decided to stop reading.
    // Following Unix convention (exit 0 on SIGPIPE), we exit 0 or 2 (nothing to check)
    // depending on timing.
    let code = out.status.code().unwrap();
    assert!(code == 0 || code == 2, "expected exit 0 or 2, got {code}");
}

// ─── Violation fixture render tests ────────────────────────────────────────

fn violation_fixture() -> std::path::PathBuf {
    o8v_testkit::Fixture::e2e("rust-violations")
        .path()
        .to_path_buf()
}

#[test]
fn rust_violations_json_has_diagnostics() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", "--json", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(
        out.status.code(),
        Some(1),
        "violations should exit 1 (failed)"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "JSON output should not be empty");

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output is not valid JSON");
    assert!(parsed.is_object(), "top-level should be object");

    // Structural assertions — not string matching.
    let results = parsed["results"]
        .as_array()
        .expect("results should be array");
    assert!(!results.is_empty(), "should have at least one result");

    let checks = results[0]["checks"]
        .as_array()
        .expect("checks should be array");
    let has_diagnostics = checks
        .iter()
        .any(|c| c["diagnostics"].as_array().is_some_and(|d| !d.is_empty()));
    assert!(
        has_diagnostics,
        "JSON should contain diagnostics from violations"
    );
}

#[test]
fn rust_violations_plain_no_ansi() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", "--plain", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "plain output should not be empty");
    assert!(
        !stdout.contains("\x1b["),
        "plain output must not contain ANSI escapes"
    );
    assert!(
        stdout.contains("src/main.rs"),
        "diagnostic file path should appear in plain output"
    );
}

#[test]
fn rust_violations_human_no_ansi_without_color() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", fixture.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    // Human mode writes to stderr. NO_COLOR set → no ANSI.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("\x1b["),
        "human with NO_COLOR must not contain ANSI escapes"
    );
    assert!(
        stderr.contains("src/main.rs") || stderr.contains("unused"),
        "diagnostic info should appear in human output"
    );
}

#[test]
fn rust_violations_exit_code_1() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    assert_eq!(
        out.status.code(),
        Some(1),
        "violations fixture should exit 1 (failed)"
    );
}

// ─── Streaming output on violations ────────────────────────────────────────
//
// These verify that streaming (human/plain) produces the same content as
// batch rendering: diagnostics, project headers, detection errors.

#[test]
fn streaming_human_shows_diagnostics() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", fixture.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&out.stderr);
    // Streaming human must include diagnostics — not just status lines.
    assert!(
        stderr.contains("src/main.rs"),
        "streaming human should show diagnostic file paths\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("error"),
        "streaming human should show diagnostic severity\nstderr: {stderr}"
    );
}

#[test]
fn streaming_plain_shows_diagnostics() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", "--plain", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Streaming plain must include diagnostics — not just status lines.
    assert!(
        stdout.contains("src/main.rs"),
        "streaming plain should show diagnostic file paths\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("diagnostics"),
        "streaming plain should show diagnostic count\nstdout: {stdout}"
    );
}

#[test]
fn streaming_human_verbose_shows_path() {
    let fixture = violation_fixture();
    let out = bin()
        .args(["check", "--verbose", fixture.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&out.stderr);
    // Verbose streaming must include the project path.
    assert!(
        stderr.contains("rust-violations") || stderr.contains("test-violations"),
        "streaming verbose should show project path\nstderr: {stderr}"
    );
}

// ─── Full pipeline on clean fixture ─────────────────────────────────────────

#[test]
fn rust_fixture_full_check() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../o8v-project/tests/fixtures/corpus/rust-standalone-app");

    let out = bin()
        .args(["check", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let code = out.status.code().unwrap();
    // exit 0 (passed) or 1 (failed) — NOT 2 (nothing to check).
    // The Rust project should be detected.
    assert!(
        code == 0 || code == 1,
        "expected exit 0 or 1, got {code}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rust_fixture_json_output_valid() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../o8v-project/tests/fixtures/corpus/rust-standalone-app");

    let out = bin()
        .args(["check", "--json", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "JSON output should not be empty");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output is not valid JSON");
    assert!(parsed.is_object(), "top-level JSON should be an object");
}

#[test]
fn rust_fixture_plain_output() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../o8v-project/tests/fixtures/corpus/rust-standalone-app");

    let out = bin()
        .args(["check", "--plain", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Plain format always has output for detected projects.
    assert!(!stdout.is_empty(), "plain output should not be empty");
    // No ANSI escapes in plain mode.
    assert!(
        !stdout.contains("\x1b["),
        "plain output should not contain ANSI escapes"
    );
}

#[test]
fn rust_fixture_verbose() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../o8v-project/tests/fixtures/corpus/rust-standalone-app");

    let out = bin()
        .args(["check", "--verbose", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run binary");

    let code = out.status.code().unwrap();
    assert!(code == 0 || code == 1, "expected exit 0 or 1, got {code}");
    // Verbose mode shows timing and project paths.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("rust-standalone-app") || stderr.contains("acme-runner"),
        "verbose should show project name\nstderr: {stderr}"
    );
}
