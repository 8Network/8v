// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for error message formatting — no doubled prefixes, correct variant text.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Finding 1: `8v ls /nonexistent` must not double the "error: " prefix.
/// Before fix: stderr was `error: error: cannot access path '...'`
/// After fix:  stderr is  `error: cannot access path '...'`
#[test]
fn ls_nonexistent_no_doubled_error_prefix() {
    let out = bin()
        .args(["ls", "/nonexistent-path-that-does-not-exist-8v-test"])
        .output()
        .expect("run 8v ls");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v ls on nonexistent path should fail"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain doubled 'error: error:' prefix\nstderr: {stderr}"
    );
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with single 'error: '\nstderr: {stderr}"
    );
}

/// Finding 1 (search): `8v search` on a nonexistent path must not double the prefix.
#[test]
fn search_nonexistent_no_doubled_error_prefix() {
    let out = bin()
        .args(["search", "pattern", "/nonexistent-path-8v-test"])
        .output()
        .expect("run 8v search");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v search on nonexistent path should fail"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain doubled 'error: error:' prefix\nstderr: {stderr}"
    );
}

/// Finding 4: path-outside-boundary error must say "path escapes project directory",
/// not "symlink escapes project directory" (the old SymlinkEscape message).
///
/// We test this via `8v ls` on a path whose *parent* is outside the project root.
/// The `safe_exists` function in o8v-fs walks ancestors to verify containment;
/// for non-symlink paths outside the root it used to return SymlinkEscape.
///
/// We can't trigger `safe_exists` directly from CLI, but the init command writes
/// files through o8v-fs. The unit-level test below exercises o8v-fs directly.
#[test]
fn containment_violation_error_text() {
    use o8v_fs::{safe_exists, ContainmentRoot};
    use std::path::Path;

    // Create a temp dir as the "root" and try to check a path outside it.
    let root_dir = tempfile::tempdir().expect("create temp dir");
    let root = ContainmentRoot::new(root_dir.path()).expect("create containment root");

    // Path that is clearly outside the root.
    let outside = Path::new("/tmp");
    let result = safe_exists(outside, &root);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                !msg.contains("symlink"),
                "ContainmentViolation should not mention 'symlink'\nerror: {msg}"
            );
            assert!(
                msg.contains("escapes") || msg.contains("containment") || msg.contains("project"),
                "ContainmentViolation should describe the boundary breach\nerror: {msg}"
            );
        }
        Ok(_) => panic!("safe_exists on an outside path should return Err"),
    }
}

/// Finding 4b: `8v read <path-outside-project-root>` must say "path escapes project directory",
/// NOT "symlink escapes project directory".
///
/// Before fix: guarded_read returned FsError::SymlinkEscape for any path outside root,
/// producing "error: symlink escapes project directory: ..." — wrong, no symlink involved.
/// After fix:  returns FsError::ContainmentViolation → "error: path escapes project directory: ..."
///
/// Failing-first test — written BEFORE fix. Run on pre-fix code: MUST FAIL.
/// After fix: MUST PASS.
#[test]
fn read_outside_root_containment_violation_not_symlink_error() {
    use std::io::Write;
    // Write a real, regular file to /tmp so guarded_read reaches the containment check
    // (it canonicalizes first, then checks starts_with(root)).
    let tmp_file = std::env::temp_dir().join("8v_test_outside_root.rs");
    {
        let mut f = std::fs::File::create(&tmp_file).expect("create tmp file");
        writeln!(f, "fn main() {{}}").expect("write content");
    }

    // Run 8v read from inside a real temp project dir so it has a containment root.
    let project_dir = tempfile::tempdir().expect("create project dir");
    // Initialize a minimal 8v project (just needs .8v/ or a Cargo.toml so 8v picks a root).
    std::fs::write(
        project_dir.path().join("Cargo.toml"),
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["read", tmp_file.to_str().unwrap()])
        .current_dir(project_dir.path())
        .output()
        .expect("run 8v read");

    let _ = std::fs::remove_file(&tmp_file);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v read on outside-root path should fail\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("symlink"),
        "error must NOT mention 'symlink' — no symlink is involved\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("escapes") || stderr.contains("project") || stderr.contains("containment"),
        "error must describe the boundary breach\nstderr: {stderr}"
    );
}

/// Write double-prefix bug: `8v write <path>:<line> ""` must emit a single `error: ` prefix,
/// not `error: error: content cannot be empty ...`
///
/// Failing-first test — written before the fix. Run on pre-fix code: MUST FAIL.
/// After fix: MUST PASS.
#[test]
fn write_empty_content_no_doubled_error_prefix() {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("target.txt");
    let mut f = std::fs::File::create(&file).expect("create file");
    writeln!(f, "hello").expect("write line");

    let path_arg = format!("{}:1", file.display());
    let out = bin()
        .args(["write", &path_arg, ""])
        .output()
        .expect("run 8v write");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v write with empty content should fail"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain doubled 'error: error:' prefix
stderr: {stderr}"
    );
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with single 'error: '
stderr: {stderr}"
    );
}

/// Write double-prefix bug: invalid line range must emit a single `error: ` prefix.
#[test]
fn write_invalid_range_no_doubled_error_prefix() {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("target.txt");
    let mut f = std::fs::File::create(&file).expect("create file");
    writeln!(f, "hello").expect("write line");

    // :5-2 is a reversed range — parse_path_line returns Err("error: invalid line range...") before
    // any file existence check, so the outer eprintln!("error: {e}") doubles the prefix.
    let path_arg = format!("{}:5-2", file.display());
    let out = bin()
        .args(["write", &path_arg, "new content"])
        .output()
        .expect("run 8v write");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "8v write with invalid range should fail"
    );
    assert!(
        !stderr.contains("error: error:"),
        "stderr must not contain doubled 'error: error:' prefix
stderr: {stderr}"
    );
    assert!(
        stderr.starts_with("error: "),
        "stderr must start with single 'error: '
stderr: {stderr}"
    );
}
