// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! F11 regression: no command should echo the real $HOME or /Users/... path in
//! error output.  Every error site must use the user-supplied label string, not
//! the resolved absolute PathBuf.

use std::process::Command;

fn bin_in(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.current_dir(dir);
    cmd
}

fn setup_project(tmp: &tempfile::TempDir) {
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
}

/// Assert that `output` (stdout + stderr combined) contains neither $HOME nor
/// the literal "/Users/" prefix common on macOS.
fn assert_no_home_leak(label: &str, stdout: &[u8], stderr: &[u8]) {
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let home: String = std::env::var_os("HOME")
        .map(|val| val.to_string_lossy().into_owned())
        .unwrap_or_default();

    if !home.is_empty() {
        assert!(
            !combined.contains(&home),
            "[{label}] output leaks $HOME ({home}):\n{combined}"
        );
    }
    // Belt-and-suspenders: also check for the macOS /Users/ prefix even if
    // $HOME happens to be something else.
    assert!(
        !combined.contains("/Users/"),
        "[{label}] output leaks '/Users/' substring:\n{combined}"
    );
}

// ─── write errors ────────────────────────────────────────────────────────────

#[test]
fn write_nonexistent_file_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["write", "nonexistent.txt:1", "content"])
        .output()
        .expect("run");
    assert_no_home_leak("write/nonexistent", &out.stdout, &out.stderr);
}

#[test]
fn write_out_of_range_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("f.txt"), "one\ntwo\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "f.txt:999", "x"])
        .output()
        .expect("run");
    assert_no_home_leak("write/out-of-range", &out.stdout, &out.stderr);
}

#[test]
fn write_file_already_exists_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("existing.txt"), "content\n").unwrap();

    // Attempt to create a file that already exists (no --force).
    let out = bin_in(tmp.path())
        .args(["write", "existing.txt", "new content"])
        .output()
        .expect("run");
    // This is the CreateFile path; file exists without --force → error.
    assert_no_home_leak("write/already-exists", &out.stdout, &out.stderr);
}

#[test]
fn write_find_not_found_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("f.txt"), "hello world\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "f.txt", "--find", "nothere", "--replace", "x"])
        .output()
        .expect("run");
    assert_no_home_leak("write/find-not-found", &out.stdout, &out.stderr);
}

#[test]
fn write_containment_violation_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["write", "../escape.txt:1", "bad"])
        .output()
        .expect("run");
    assert_no_home_leak("write/containment", &out.stdout, &out.stderr);
}

// ─── read errors ─────────────────────────────────────────────────────────────

#[test]
fn read_nonexistent_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["read", "nonexistent.txt"])
        .output()
        .expect("run");
    assert_no_home_leak("read/nonexistent", &out.stdout, &out.stderr);
}

#[test]
fn read_out_of_range_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    std::fs::write(tmp.path().join("f.txt"), "one\ntwo\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["read", "f.txt:999-1000"])
        .output()
        .expect("run");
    assert_no_home_leak("read/out-of-range", &out.stdout, &out.stderr);
}

// ─── ls errors ───────────────────────────────────────────────────────────────

#[test]
fn ls_nonexistent_path_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["ls", "nonexistent_dir"])
        .output()
        .expect("run");
    assert_no_home_leak("ls/nonexistent", &out.stdout, &out.stderr);
}

// ─── search errors ───────────────────────────────────────────────────────────

#[test]
fn search_nonexistent_path_no_home_leak() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["search", "pattern", "nonexistent_dir"])
        .output()
        .expect("run");
    assert_no_home_leak("search/nonexistent", &out.stdout, &out.stderr);
}

// ─── find-replace escape sequences ─────────────────────────────────────────

#[test]
fn write_find_replace_unescapes_newline() {
    // `--replace "a\nb"` must write two lines, not the literal \n.
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("f.txt");
    std::fs::write(&file, "foo\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "f.txt", "--find", "foo", "--replace", r"a\nb"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "should succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read_to_string(&file).unwrap();
    assert!(
        result.contains('\n') && result.contains('a') && result.contains('b'),
        "replace must produce two lines; got: {result:?}"
    );
    assert!(
        !result.contains("\\n"),
        "result must not contain literal \\n; got: {result:?}"
    );
    // Exact check: "foo" → "a\nb" (real newline)
    assert_eq!(result, "a\nb\n", "expected 'a\\nb\\n', got: {result:?}");
}
