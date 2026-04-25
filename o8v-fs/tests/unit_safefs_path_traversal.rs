// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Path traversal security tests — dotdot attacks, containment boundary tests,
//! and bug confirmation tests (confirm_* regression tests).

use o8v_fs::*;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fs_default(root: &std::path::Path) -> SafeFs {
    SafeFs::new(root, FsConfig::default()).expect("SafeFs::new failed")
}

// ---------------------------------------------------------------------------
// Path traversal attacks
// ---------------------------------------------------------------------------

#[test]
fn security_path_traversal_dotdot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("safe.txt"), "safe").unwrap();

    let fs = fs_default(dir.path());

    let traversal_path = dir.path().join("../../../etc/passwd");
    let err = fs.read_file(&traversal_path).unwrap_err();
    let msg = format!("{err}");

    assert!(
        msg.contains("not found")
            || msg.contains("symlink escapes")
            || msg.contains("permission denied"),
        "expected containment block on path traversal, got: {msg}"
    );
}

#[test]
fn security_path_traversal_from_subdir_allowed() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(dir.path().join("outside.txt"), "outside").unwrap();

    let fs = fs_default(dir.path());

    let traversal = subdir.join("../outside.txt");
    let result = fs.read_file(&traversal);
    assert!(
        result.is_ok(),
        "reading ../outside.txt from subdir (resolves within root) should succeed"
    );
    assert_eq!(result.unwrap().content(), "outside");
}

#[test]
fn security_path_traversal_many_dotdot() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("safe.txt"), "safe").unwrap();

    let fs = fs_default(dir.path());

    let mut traversal = dir.path().to_path_buf();
    for _ in 0..20 {
        traversal.push("..");
    }
    traversal.push("etc");
    traversal.push("passwd");

    let err = fs.read_file(&traversal).unwrap_err();
    let msg = format!("{err}");

    assert!(
        msg.contains("not found")
            || msg.contains("symlink escapes")
            || msg.contains("path escapes")
            || msg.contains("permission denied"),
        "expected containment block on excessive traversal, got: {msg}"
    );
}

// ===========================================================================
// Bug confirmation tests — each test FAILS while the bug exists,
// PASSES once the bug is fixed. Names match the review doc issue IDs.
// ===========================================================================

#[cfg(unix)]
#[test]
fn confirm_critical_1_dangling_symlink_write_bypass() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_canonical = std::fs::canonicalize(outside.path()).unwrap();
    let target = outside_canonical.join("escaped.txt");
    assert!(!target.exists(), "precondition: target must not exist");

    let canonical_root = std::fs::canonicalize(root.path()).unwrap();
    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let link = canonical_root.join("dangle.txt");
    symlink(&target, &link).unwrap();

    let result = safe_write(&link, &containment_root, b"pwned");
    assert!(
        result.is_err(),
        "bug CRITICAL-1: safe_write succeeded through dangling symlink; \
         target outside root was created: {}",
        target.exists()
    );
    assert!(
        !target.exists(),
        "bug CRITICAL-1: write escaped root — file created at {}",
        target.display()
    );
}

#[cfg(unix)]
#[test]
fn confirm_high_1_safe_exists_non_canonical_root_rejects_legitimate_file() {
    use std::os::unix::fs::symlink;

    let real_root = tempdir().unwrap();
    let sym_root_parent = tempdir().unwrap();
    let sym_root = sym_root_parent.path().join("sym_root");
    symlink(real_root.path(), &sym_root).unwrap();

    let file = real_root.path().join("legit.txt");
    std::fs::write(&file, "hello").unwrap();

    let path_via_sym = sym_root.join("legit.txt");

    let containment_root = ContainmentRoot::new(&sym_root).unwrap();
    let result = safe_exists(&path_via_sym, &containment_root);
    assert!(
        result.is_ok() && result.unwrap(),
        "bug HIGH-1: safe_exists with symlinked root rejected a legitimate file \
         (canonical parent doesn't start_with non-canonical root)"
    );
}

#[cfg(unix)]
#[test]
fn confirm_high_3_standalone_safe_read_non_canonical_root_rejects_legitimate_file() {
    use std::os::unix::fs::symlink;

    let real_root = tempdir().unwrap();
    let sym_root_parent = tempdir().unwrap();
    let sym_root = sym_root_parent.path().join("sym_root");
    symlink(real_root.path(), &sym_root).unwrap();

    let file = real_root.path().join("data.txt");
    std::fs::write(&file, "contents").unwrap();

    let path_via_sym = sym_root.join("data.txt");
    let containment_root = ContainmentRoot::new(&sym_root).unwrap();
    let result = safe_read(&path_via_sym, &containment_root, &FsConfig::default());
    assert!(
        result.is_ok(),
        "bug HIGH-3: standalone safe_read with symlinked root rejected a legitimate file: {:?}",
        result.unwrap_err()
    );
}

#[cfg(unix)]
#[test]
fn confirm_high_4_create_dir_dangling_symlink_escape() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_canonical = std::fs::canonicalize(outside.path()).unwrap();
    let outside_target = outside_canonical.join("injected");
    assert!(!outside_target.exists(), "precondition");

    let canonical_root = std::fs::canonicalize(root.path()).unwrap();
    let subdir = canonical_root.join("sub");
    std::fs::create_dir(&subdir).unwrap();
    let link = subdir.join("dlink");
    symlink(&outside_target, &link).unwrap();

    let new_dir = link.join("newdir");
    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let result = safe_create_dir(&new_dir, &containment_root);
    assert!(
        result.is_err(),
        "bug HIGH-4: safe_create_dir succeeded through dangling symlink"
    );
    assert!(
        !outside_target.exists(),
        "bug HIGH-4: directory created outside root at {}",
        outside_target.display()
    );
}

#[test]
fn confirm_medium_1_safe_exists_skips_containment_when_parent_missing() {
    let root = tempdir().unwrap();

    let outside_root = tempdir().unwrap();
    let nonexistent_parent = outside_root.path().join("no_such_dir");
    let outside_path = nonexistent_parent.join("file.txt");
    assert!(
        !nonexistent_parent.exists(),
        "precondition: parent must not exist"
    );

    let canonical_root = std::fs::canonicalize(root.path()).unwrap();
    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let result = safe_exists(&outside_path, &containment_root);
    assert!(
        result.is_err(),
        "bug MEDIUM-1: safe_exists returned Ok({:?}) for out-of-root path \
         with missing parent — containment check was skipped",
        result.unwrap()
    );
}

#[cfg(unix)]
#[test]
fn confirm_medium_3_scan_exposes_escape_symlink_metadata() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("real.txt"), "ok").unwrap();
    symlink("/etc/passwd", dir.path().join("escape_link")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let escape_entry = scan.entries().iter().find(|e| e.name == "escape_link");

    assert!(
        escape_entry.is_some(),
        "escape symlink not found in scan entries — behavior may have changed"
    );
    let entry = escape_entry.unwrap();
    assert!(
        entry.is_symlink,
        "expected is_symlink == true for escape link, got false"
    );

    let read_result = fs.read_checked(&scan, "escape_link");
    assert!(
        matches!(read_result, Err(FsError::SymlinkEscape { .. })),
        "expected SymlinkEscape when reading escape symlink via read_checked, \
         got: {:?}",
        read_result
    );
}

#[test]
fn confirm_low_3_invalid_content_display_panics_on_utf8_boundary() {
    let result = std::panic::catch_unwind(|| {
        let cause = "x".repeat(199) + "中abc";
        let err = FsError::InvalidContent {
            path: std::path::PathBuf::from("test.txt"),
            cause,
        };
        format!("{err}")
    });
    assert!(
        result.is_ok(),
        "bug LOW-3: InvalidContent Display panicked on multi-byte UTF-8 \
         at truncation boundary — use floor_char_boundary or chars().take()"
    );
}

#[test]
fn confirm_r3_1_safe_exists_relative_path_false_symlink_escape() {
    let dir = tempdir().unwrap();
    let canonical_root = std::fs::canonicalize(dir.path()).unwrap();

    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let result = safe_exists(
        std::path::Path::new("does_not_exist.txt"),
        &containment_root,
    );
    assert!(
        result.is_ok(),
        "bug R3-1: safe_exists with bare relative filename returned error: {:?}",
        result.unwrap_err()
    );
}

#[cfg(unix)]
#[test]
fn confirm_r3_2_create_dir_ancestor_walk_skips_dangling_symlink() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_canonical = std::fs::canonicalize(outside.path()).unwrap();
    let canonical_root = std::fs::canonicalize(root.path()).unwrap();

    let outside_target = outside_canonical.join("injected_r3");
    assert!(
        !outside_target.exists(),
        "precondition: target must not exist"
    );

    let subdir = canonical_root.join("sub");
    std::fs::create_dir(&subdir).unwrap();
    let link = subdir.join("dlink");
    symlink(&outside_target, &link).unwrap();

    let new_dir = link.join("newdir");
    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let result = safe_create_dir(&new_dir, &containment_root);
    assert!(
        result.is_err(),
        "bug R3-2: safe_create_dir followed dangling symlink in ancestor walk"
    );
    assert!(
        !outside_target.exists(),
        "bug R3-2: directory escaped root — created at {}",
        outside_target.display()
    );
}

#[test]
fn confirm_low_4_meta_to_kind_never_returns_file_for_regular_files() {
    let dir = tempdir().unwrap();
    let canonical_root = std::fs::canonicalize(dir.path()).unwrap();
    let containment_root = ContainmentRoot::new(&canonical_root).unwrap();
    let file_path = canonical_root.join("regular.txt");
    std::fs::write(&file_path, "content").unwrap();

    let err = safe_read_dir(&file_path, &containment_root).unwrap_err();
    match err {
        FsError::NotRegularFile { kind, .. } => {
            assert_eq!(
                kind,
                FileKind::File,
                "bug LOW-4: meta_to_kind returned {:?} for a regular file — \
                 is_file() branch is missing",
                kind
            );
        }
        other => panic!("expected NotRegularFile, got: {other}"),
    }
}
