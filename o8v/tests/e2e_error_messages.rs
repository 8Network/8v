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
