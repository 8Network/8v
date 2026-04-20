// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary E2E harness — spawns the compiled `8v` binary and asserts on stdout/stderr.
//!
//! Every test uses `run_8v` to keep boilerplate minimal. Fixtures live in
//! `tests/fixtures/`. Each test covers one command × output mode × path.

use std::process::Command;

use o8v_testkit::TempProject;

// ── Helper ───────────────────────────────────────────────────────────────────

/// Spawn `8v` with the given arguments, return `(stdout, stderr, success)`.
fn run_8v(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(args)
        .output()
        .expect("failed to spawn 8v");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ── version / help ───────────────────────────────────────────────────────────

#[test]
fn version_exits_0() {
    let (stdout, stderr, ok) = run_8v(&["--version"]);
    assert!(ok, "expected exit 0\nstdout:{stdout}\nstderr:{stderr}");
    assert!(stdout.contains("8v"), "should contain '8v': {stdout}");
}

#[test]
fn help_exits_0() {
    let (stdout, stderr, ok) = run_8v(&["--help"]);
    assert!(ok, "expected exit 0\nstdout:{stdout}\nstderr:{stderr}");
    assert!(stdout.contains("Usage"), "should contain 'Usage': {stdout}");
}

// ── ls ───────────────────────────────────────────────────────────────────────

#[test]
fn ls_plain_rust_project() {
    let project = fixture("ls-rust-project");
    let (stdout, stderr, ok) = run_8v(&["ls", project.path().to_str().unwrap()]);
    assert!(ok, "ls should exit 0\nstdout:{stdout}\nstderr:{stderr}");
    assert!(
        stdout.contains("rust"),
        "should detect rust stack: {stdout}"
    );
}

#[test]
fn ls_json_has_projects_field() {
    let project = fixture("ls-rust-project");
    let (stdout, stderr, ok) = run_8v(&["ls", project.path().to_str().unwrap(), "--json"]);
    assert!(
        ok,
        "ls --json should exit 0\nstdout:{stdout}\nstderr:{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(
        v.get("projects").is_some(),
        "missing 'projects' field: {stdout}"
    );
}

#[test]
fn ls_nonexistent_path_fails() {
    let (stdout, stderr, ok) = run_8v(&["ls", "/nonexistent/path/xyz"]);
    assert!(
        !ok,
        "should fail on bad path\nstdout:{stdout}\nstderr:{stderr}"
    );
}

// ── read ─────────────────────────────────────────────────────────────────────

#[test]
fn read_rust_file_shows_symbols() {
    // Use the fixture path directly — safe_read enforces containment within the
    // git repo, and TempProject copies to /tmp which is outside the repo.
    let fixture_dir = o8v_testkit::fixture_path("o8v", "build-rust");
    let main = fixture_dir.join("src").join("main.rs");
    let (stdout, stderr, ok) = run_8v(&["read", main.to_str().unwrap()]);
    assert!(ok, "read should exit 0\nstdout:{stdout}\nstderr:{stderr}");
    // Symbol map should show function names
    assert!(!stdout.is_empty(), "should produce symbol output: {stdout}");
}

#[test]
fn read_nonexistent_file_fails() {
    let (stdout, stderr, ok) = run_8v(&["read", "/nonexistent/file.rs"]);
    assert!(
        !ok,
        "should fail on missing file\nstdout:{stdout}\nstderr:{stderr}"
    );
}

// ── search ───────────────────────────────────────────────────────────────────
//
// NOTE: search uses safe_read which enforces containment within the workspace
// (git repo root). TempProject copies to /tmp which is outside the repo, so
// safe_read skips every file → files_skipped > 0 → nonzero exit.
// These tests use the fixture path directly (within the git repo) to avoid this.

#[test]
fn search_finds_pattern_in_fixture() {
    let path = o8v_testkit::fixture_path("o8v", "build-rust");
    let (stdout, stderr, ok) = run_8v(&["search", "fn main", path.to_str().unwrap()]);
    assert!(ok, "search should exit 0\nstdout:{stdout}\nstderr:{stderr}");
    assert!(stdout.contains("main"), "should find pattern: {stdout}");
}

#[test]
fn search_json_is_valid() {
    let path = o8v_testkit::fixture_path("o8v", "build-rust");
    let (stdout, stderr, ok) = run_8v(&["search", "fn main", path.to_str().unwrap(), "--json"]);
    assert!(
        ok,
        "search --json should exit 0\nstdout:{stdout}\nstderr:{stderr}"
    );
    let _: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
}

#[test]
fn search_no_match_exits_1_with_empty_stderr() {
    // Per error-contract §7 (CE-2 resolution): no matches + no I/O errors = exit 1
    // with stderr empty. Agents distinguish clean no-match (stderr empty) from
    // partial I/O failure (stderr non-empty).
    let path = o8v_testkit::fixture_path("o8v", "build-rust");
    let (stdout, stderr, ok) = run_8v(&["search", "ZZZNOMATCHZZZ", path.to_str().unwrap()]);
    assert!(
        !ok,
        "search must exit non-zero on no-match\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stderr.is_empty(),
        "clean no-match must have empty stderr; got:{stderr}"
    );
    assert!(
        stdout.contains("no matches") || stdout.is_empty() || stdout.contains("0"),
        "stdout should indicate no match found\nstdout:{stdout}"
    );
}

// ── build ─────────────────────────────────────────────────────────────────────

#[test]
fn build_rust_succeeds_plain() {
    let project = fixture("build-rust");
    let (stdout, stderr, ok) = run_8v(&["build", project.path().to_str().unwrap()]);
    assert!(ok, "build should exit 0\nstdout:{stdout}\nstderr:{stderr}");
    assert!(stdout.contains("rust"), "should show stack: {stdout}");
}

#[test]
fn build_rust_succeeds_json() {
    let project = fixture("build-rust");
    let (stdout, stderr, ok) = run_8v(&["build", project.path().to_str().unwrap(), "--json"]);
    assert!(
        ok,
        "build --json should exit 0\nstdout:{stdout}\nstderr:{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(v["success"], serde_json::Value::Bool(true));
}

#[test]
fn build_rust_broken_fails() {
    let project = fixture("build-rust-broken");
    let (stdout, stderr, ok) = run_8v(&["build", project.path().to_str().unwrap()]);
    assert!(
        !ok,
        "broken build should fail\nstdout:{stdout}\nstderr:{stderr}"
    );
}

// ── test (F9 regression + F10 structured output) ─────────────────────────────

/// F9 REGRESSION — `8v test` must not error with "Option 'format' given more than once".
/// On unfixed HEAD this test will fail because the command exits nonzero with that error.
#[test]
fn test_rust_pass_exits_0() {
    let project = fixture("test-rust-pass");
    let (stdout, stderr, ok) = run_8v(&["test", project.path().to_str().unwrap()]);
    assert!(
        ok,
        "8v test should exit 0 on passing tests\nstdout:{stdout}\nstderr:{stderr}"
    );
    // Must NOT contain the duplicate-format error (F9).
    assert!(
        !stderr.contains("given more than once"),
        "F9: duplicate --format flag detected\nstderr:{stderr}"
    );
    assert!(
        !stdout.contains("given more than once"),
        "F9: duplicate --format flag in stdout\nstdout:{stdout}"
    );
}

/// F9 REGRESSION — JSON mode must also work without the duplicate-flag error.
#[test]
fn test_rust_pass_json_valid() {
    let project = fixture("test-rust-pass");
    let (stdout, stderr, ok) = run_8v(&["test", project.path().to_str().unwrap(), "--json"]);
    assert!(
        ok,
        "8v test --json should exit 0\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        !stderr.contains("given more than once"),
        "F9: duplicate --format flag\nstderr:{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(
        v["success"],
        serde_json::Value::Bool(true),
        "success field should be true: {stdout}"
    );
}

/// F10 REGRESSION — failing tests must produce structured output, not a raw stderr dump.
/// Line like "tests N passed M failed" must appear.
#[test]
fn test_rust_fail_shows_structured_output() {
    let project = fixture("test-rust-fail");
    let (stdout, stderr, ok) = run_8v(&["test", project.path().to_str().unwrap()]);
    assert!(
        !ok,
        "8v test should exit nonzero on failing tests\nstdout:{stdout}\nstderr:{stderr}"
    );
    // F9 fix: no duplicate-format error
    assert!(
        !stderr.contains("given more than once"),
        "F9: duplicate --format flag\nstderr:{stderr}"
    );
    // F10: structured summary line must appear
    assert!(
        stdout.contains("passed") || stdout.contains("failed"),
        "F10: structured test count line expected\nstdout:{stdout}\nstderr:{stderr}"
    );
    // The degenerate fallback just does "<project> rust\ntests failed <ms>\n<raw stderr>".
    // Structured output includes "passed" counts. If only "tests failed" with no "passed" appears,
    // the render is still degenerate.
    assert!(
        stdout.contains("passed"),
        "F10: 'passed' count missing — degenerate render detected\nstdout:{stdout}\nstderr:{stderr}"
    );
}

/// F10 — JSON output on failing tests must have structured fields.
#[test]
fn test_rust_fail_json_has_counts() {
    let project = fixture("test-rust-fail");
    let (stdout, stderr, ok) = run_8v(&["test", project.path().to_str().unwrap(), "--json"]);
    assert!(!ok, "should exit nonzero\nstdout:{stdout}\nstderr:{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(
        v["success"],
        serde_json::Value::Bool(false),
        "success should be false: {stdout}"
    );
}

// ── check ─────────────────────────────────────────────────────────────────────

#[test]
fn check_rust_project_plain() {
    let project = fixture("build-rust");
    let (stdout, stderr, _ok) = run_8v(&["check", project.path().to_str().unwrap()]);
    // check may exit nonzero if warnings exist; just confirm it runs without panic
    assert!(
        stdout.contains("rust") || stderr.contains("rust") || stdout.contains("check"),
        "should produce check output\nstdout:{stdout}\nstderr:{stderr}"
    );
}

#[test]
fn check_rust_project_json() {
    let project = fixture("build-rust");
    let (stdout, _stderr, _ok) = run_8v(&["check", project.path().to_str().unwrap(), "--json"]);
    let _: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
}

// ── fmt ───────────────────────────────────────────────────────────────────────

#[test]
fn fmt_rust_project_exits_0() {
    let project = fixture("build-rust");
    let (stdout, stderr, ok) = run_8v(&["fmt", project.path().to_str().unwrap()]);
    assert!(ok, "fmt should exit 0\nstdout:{stdout}\nstderr:{stderr}");
}

// ── init ──────────────────────────────────────────────────────────────────────

#[test]
fn init_without_tty_prints_error() {
    // `8v init` requires an interactive terminal; in CI/tests it should exit
    // nonzero with a clear error message, not panic.
    let project = fixture("build-rust");
    let (stdout, stderr, ok) = run_8v(&["init", project.path().to_str().unwrap()]);
    assert!(
        !ok,
        "init without tty should fail gracefully\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stderr.contains("interactive") || stderr.contains("terminal"),
        "should explain TTY requirement\nstderr:{stderr}"
    );
}

// ── error paths ───────────────────────────────────────────────────────────────

#[test]
fn unknown_subcommand_fails() {
    let (stdout, stderr, ok) = run_8v(&["notacommand"]);
    assert!(
        !ok,
        "unknown command should fail\nstdout:{stdout}\nstderr:{stderr}"
    );
}

#[test]
fn test_no_project_fails() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let (stdout, stderr, ok) = run_8v(&["test", dir.path().to_str().unwrap()]);
    assert!(
        !ok,
        "test with no project should fail\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stdout.contains("no project")
            || stderr.contains("no project")
            || stderr.contains("no workspace"),
        "should report no project detected\nstdout:{stdout}\nstderr:{stderr}"
    );
}
