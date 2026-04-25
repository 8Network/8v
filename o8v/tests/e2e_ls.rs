// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v ls` — runs the compiled binary against fixture directories.

use o8v_testkit::TempProject;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = o8v_testkit::fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

/// `8v ls` on a Rust project shows the project with stack label
#[test]
fn ls_default_shows_rust_project() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v ls");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v ls should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("rust"),
        "Expected 'rust' in output, got: {stdout}"
    );
}

/// `8v ls --tree` shows file hierarchy
#[test]
fn ls_tree_shows_files() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--tree"])
        .output()
        .expect("run 8v ls --tree");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "8v ls --tree should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("main.rs"),
        "Expected 'main.rs' in tree output, got: {stdout}"
    );
    assert!(
        stdout.contains("src/"),
        "Expected 'src/' in tree output, got: {stdout}"
    );
}

/// `8v ls --files` shows flat listing
#[test]
fn ls_files_flat_output() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--files"])
        .output()
        .expect("run 8v ls --files");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("src/main.rs"),
        "Expected src/main.rs, got: {stdout}"
    );
    assert!(
        stdout.contains("src/lib.rs"),
        "Expected src/lib.rs, got: {stdout}"
    );
}

/// `8v ls --json` produces valid JSON with expected structure
#[test]
fn ls_json_valid() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v ls --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output must be valid JSON");
    assert!(parsed["projects"].is_array(), "Expected projects array");
    assert!(parsed["total_files"].is_number(), "Expected total_files");
    assert!(parsed["truncated"].is_boolean(), "Expected truncated");
    assert!(
        parsed["files_filtered"].is_number(),
        "Expected files_filtered"
    );
    assert!(
        parsed["files_skipped_gitignore"].is_number(),
        "Expected files_skipped_gitignore"
    );
}

/// `8v ls --loc` shows line counts next to files
#[test]
fn ls_loc_shows_line_counts() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--files", "--loc"])
        .output()
        .expect("run 8v ls --files --loc");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // main.rs has 3 lines — should show a number
    let has_number = stdout
        .lines()
        .any(|l| l.contains("main.rs") && l.chars().any(|c| c.is_ascii_digit()));
    assert!(
        has_number,
        "Expected line count next to main.rs, got: {stdout}"
    );
}

/// `8v ls --loc --json` includes loc field in file objects
#[test]
fn ls_loc_json_has_field() {
    let project = fixture("ls-rust-project");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--json", "--loc"])
        .output()
        .expect("run 8v ls --json --loc");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let projects = parsed["projects"].as_array().unwrap();
    assert!(!projects.is_empty());
    let files = projects[0]["files"].as_array().unwrap();
    assert!(!files.is_empty());
    assert!(
        files[0]["loc"].is_number(),
        "Expected loc field in file object with --loc, got: {:?}",
        files[0]
    );
}

/// `8v ls --depth 1 --tree` limits tree depth
#[test]
fn ls_depth_limits_output() {
    let project = fixture("ls-deep-tree");

    let out = bin()
        .args([
            "ls",
            project.path().to_str().unwrap(),
            "--tree",
            "--depth",
            "1",
        ])
        .output()
        .expect("run 8v ls --tree --depth 1");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("src/"),
        "Expected src/ at depth 1, got: {stdout}"
    );
    // nested/ should not appear at depth 1
    assert!(
        !stdout.contains("nested/"),
        "nested/ should not appear at depth 1, got: {stdout}"
    );
}

/// `8v ls -e rs` filters to only .rs files
#[test]
fn ls_extension_filter() {
    let project = fixture("ls-multi-ext");

    let out = bin()
        .args([
            "ls",
            project.path().to_str().unwrap(),
            "--files",
            "-e",
            "rs",
        ])
        .output()
        .expect("run 8v ls --files -e rs");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("main.rs"),
        "Expected main.rs, got: {stdout}"
    );
    assert!(
        !stdout.contains("data.json"),
        "data.json should be filtered, got: {stdout}"
    );
    assert!(
        !stdout.contains("readme.txt"),
        "readme.txt should be filtered, got: {stdout}"
    );
}

/// `8v ls --match "*_test*"` filters files by glob pattern
#[test]
fn ls_match_pattern() {
    let project = fixture("ls-multi-ext");

    let out = bin()
        .args([
            "ls",
            project.path().to_str().unwrap(),
            "--files",
            "--match",
            "*_test*",
        ])
        .output()
        .expect("run 8v ls --files --match");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("handler_test.rs"),
        "Expected handler_test.rs, got: {stdout}"
    );
    assert!(
        !stdout.contains("main.rs"),
        "main.rs should be filtered, got: {stdout}"
    );
}

/// `8v ls` respects .gitignore — target/ and *.log are hidden
#[test]
fn ls_gitignore_respected() {
    let project = fixture("ls-with-gitignore");

    let out = bin()
        .args(["ls", project.path().to_str().unwrap(), "--files"])
        .output()
        .expect("run 8v ls --files");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("main.rs"),
        "main.rs should be visible, got: {stdout}"
    );
    assert!(
        !stdout.contains("target/"),
        "target/ should be gitignored, got: {stdout}"
    );
    assert!(
        !stdout.contains("app.log"),
        "app.log should be gitignored, got: {stdout}"
    );
}

/// `8v ls` on nonexistent path fails with error
#[test]
fn ls_nonexistent_path_errors() {
    let out = bin()
        .args(["ls", "/nonexistent/path/that/does/not/exist"])
        .output()
        .expect("run 8v ls on nonexistent path");

    assert!(!out.status.success(), "Should fail on nonexistent path");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("cannot"),
        "Expected error message, got: {stderr}"
    );
}

/// `8v ls` on empty directory succeeds (not an error)
#[test]
fn ls_empty_directory() {
    let project = TempProject::empty();

    let out = bin()
        .args(["ls", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v ls on empty dir");

    assert!(
        out.status.success(),
        "8v ls on empty dir should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `8v ls --match <glob>` must show matched files at non-root depth.
///
/// Previously, `--match *.rs` without `--files` or `--tree` rendered only
/// project headers (Projects mode), making the output appear as "no files".
/// When `--match` is given alone, the command must switch to Files mode so
/// matched files are actually visible.
#[test]
fn ls_match_glob_shows_nested_files_without_files_flag() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Create a nested file: a/b/c/foo.rs
    std::fs::create_dir_all(dir.join("a/b/c")).expect("create dirs");
    std::fs::write(dir.join("a/b/c/foo.rs"), "fn main() {}").expect("write foo.rs");

    // Run `8v ls <dir> --match "*.rs"` (no --files, no --tree)
    let out = bin()
        .args(["ls", dir.to_str().unwrap(), "--match", "*.rs"])
        .output()
        .expect("run 8v ls --match");

    assert!(
        out.status.success(),
        "exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("foo.rs"),
        "Expected foo.rs in output when --match *.rs is given without --files/--tree\nGot: {stdout}"
    );
}

#[test]
fn ls_stack_filter_no_match_returns_empty_not_unknown() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("main.py"), "print('hello')").expect("write");
    let out = bin()
        .args(["ls", tmp.path().to_str().unwrap(), "--stack", "rust"])
        .output()
        .expect("run 8v ls --stack rust");
    assert!(
        out.status.success(),
        "exit 0
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("unknown"),
        "stack filter with no matches must not output 'unknown'
Got: {stdout}"
    );
}

#[test]
fn ls_recurses_into_workspace_members() {
    // Bug A: 8v ls returns only 1 project for a workspace with multiple crate members.
    // The root Cargo.toml (workspace) is detected, but detect_all stops because
    // projects.is_empty() is false — the shallow subdir scan never triggers.
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Workspace-level Cargo.toml
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]
members = [\"crate-a\", \"crate-b\"]
resolver = \"2\"
",
    )
    .expect("write workspace Cargo.toml");

    // crate-a
    std::fs::create_dir(root.join("crate-a")).expect("mkdir crate-a");
    std::fs::write(
        root.join("crate-a/Cargo.toml"),
        "[package]
name = \"crate-a\"
version = \"0.1.0\"
edition = \"2021\"
",
    )
    .expect("write crate-a Cargo.toml");
    std::fs::create_dir(root.join("crate-a/src")).expect("mkdir src");
    std::fs::write(root.join("crate-a/src/lib.rs"), "pub fn a() {}").expect("write lib.rs");

    // crate-b
    std::fs::create_dir(root.join("crate-b")).expect("mkdir crate-b");
    std::fs::write(
        root.join("crate-b/Cargo.toml"),
        "[package]
name = \"crate-b\"
version = \"0.1.0\"
edition = \"2021\"
",
    )
    .expect("write crate-b Cargo.toml");
    std::fs::create_dir(root.join("crate-b/src")).expect("mkdir src");
    std::fs::write(root.join("crate-b/src/lib.rs"), "pub fn b() {}").expect("write lib.rs");

    let out = bin()
        .args(["ls", root.to_str().unwrap()])
        .output()
        .expect("run 8v ls");

    assert!(
        out.status.success(),
        "exit 0
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("crate-a") && stdout.contains("crate-b"),
        "8v ls must surface all workspace member crates, not just the root workspace entry
Got:
{stdout}"
    );
}

#[test]
fn ls_tree_unknown_dir_does_not_show_unknown_label() {
    // Bug C: 8v ls --tree on a dir with no detectable project shows `./  [unknown]`.
    // The synthetic "unknown" label is useless noise — the user already supplied the path.
    let tmp = tempfile::tempdir().expect("tempdir");
    // Write a plain file — no project manifest of any kind.
    std::fs::write(tmp.path().join("hello.txt"), "world").expect("write");
    let out = bin()
        .args(["ls", tmp.path().to_str().unwrap(), "--tree"])
        .output()
        .expect("run 8v ls --tree");
    assert!(
        out.status.success(),
        "exit 0
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("unknown"),
        "8v ls --tree on a non-project dir must not output 'unknown'
Got: {stdout}"
    );
}

#[test]
fn ls_projects_unknown_dir_does_not_show_unknown_label() {
    // Bug C: 8v ls (default Projects mode) on a dir with no detectable project shows
    // `<dirname>  unknown  .` — confusing synthetic entry with misleading stack label.
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("hello.txt"), "world").expect("write");
    let out = bin()
        .args(["ls", tmp.path().to_str().unwrap()])
        .output()
        .expect("run 8v ls");
    assert!(
        out.status.success(),
        "exit 0
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("unknown"),
        "8v ls on a non-project dir must not output 'unknown'
Got: {stdout}"
    );
}

/// BUG STACK-1 regression: unknown --stack value must exit non-zero with error.
#[test]
fn ls_unknown_stack_rejected() {
    let out = bin()
        .args(["ls", ".", "--stack", "xyzzy_invalid"])
        .output()
        .expect("run 8v ls --stack xyzzy_invalid");
    assert!(
        !out.status.success(),
        "8v ls with unknown stack must exit non-zero, got exit 0"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("xyzzy_invalid")
            || combined.to_lowercase().contains("unknown")
            || combined.to_lowercase().contains("invalid"),
        "error must mention the invalid stack name or 'unknown'/'invalid', got: {combined}"
    );
}
