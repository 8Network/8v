// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Symlink security tests — escape detection, containment, TOCTOU races,
//! device nodes, dangling links, self-referential loops.

use o8v_fs::*;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fs_default(root: &std::path::Path) -> SafeFs {
    SafeFs::new(root, FsConfig::default()).expect("SafeFs::new failed")
}

// ---------------------------------------------------------------------------
// Basic symlink containment
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn symlink_within_root() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    std::fs::write(&target, "linked content").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let fs = fs_default(dir.path());
    let file = fs.read_file(&link).unwrap();
    assert_eq!(file.content(), "linked content");
}

#[cfg(unix)]
#[test]
fn symlink_escape() {
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_file = outside.path().join("secret.txt");
    std::fs::write(&outside_file, "top secret").unwrap();

    let link = dir.path().join("escape.txt");
    std::os::unix::fs::symlink(&outside_file, &link).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "expected SymlinkEscape, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn symlink_to_directory() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    let link = dir.path().join("link_to_dir");
    std::os::unix::fs::symlink(&subdir, &link).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not a regular file") && msg.contains("directory"),
        "expected NotRegularFile(Directory), got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn fifo_rejected() {
    let dir = tempdir().unwrap();
    let fifo_path = dir.path().join("pipe.fifo");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo_path)
        .status()
        .expect("mkfifo command failed");
    assert!(status.success(), "mkfifo returned non-zero");

    let fs = fs_default(dir.path());
    let err = fs.read_file(&fifo_path).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not a regular file") && msg.contains("FIFO"),
        "expected NotRegularFile(Fifo), got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn deeply_nested_symlinks() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("real.txt");
    std::fs::write(&target, "deep content").unwrap();

    let mut prev = target;
    for i in 1..=5 {
        let link = dir.path().join(format!("link{i}.txt"));
        std::os::unix::fs::symlink(&prev, &link).unwrap();
        prev = link;
    }

    let fs = fs_default(dir.path());
    let file = fs.read_file(&prev).unwrap();
    assert_eq!(file.content(), "deep content");
}

// ---------------------------------------------------------------------------
// TOCTOU race tests (ignored — non-deterministic)
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
#[ignore]
fn symlink_race_type_change() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let target = dir.path().join("racey.txt");
    std::fs::write(&target, "initial content").unwrap();

    let fs = fs_default(dir.path());

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let path_clone = target.clone();

    let swapper = std::thread::spawn(move || {
        for _ in 0..1000 {
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }
            let _ = std::fs::remove_file(&path_clone);
            let _ = std::process::Command::new("mkfifo")
                .arg(&path_clone)
                .status();
            let _ = std::fs::remove_file(&path_clone);
            let _ = std::fs::write(&path_clone, "back to file");
        }
    });

    let mut saw_race_or_other_error = false;
    for _ in 0..1000 {
        match fs.read_file(&target) {
            Ok(_) => {}
            Err(e) => {
                let msg = format!("{e}");
                saw_race_or_other_error = true;
                if msg.contains("changed type") {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();

    assert!(
        saw_race_or_other_error,
        "expected at least one error during concurrent file-type swap, but every read succeeded"
    );
}

#[cfg(unix)]
#[test]
#[ignore]
fn symlink_race_containment() {
    // Known limitation: canonicalize → open TOCTOU gap cannot be tested
    // deterministically. No assertions are possible here.
}

#[cfg(unix)]
#[test]
#[ignore]
fn deep_symlink_chain() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("deep_target.txt");
    std::fs::write(&target, "deep").unwrap();

    let mut prev = target;
    for i in 1..=40 {
        let link = dir.path().join(format!("deep_link_{i}.txt"));
        std::os::unix::fs::symlink(&prev, &link).unwrap();
        prev = link;
    }

    let fs = fs_default(dir.path());
    match fs.read_file(&prev) {
        Ok(file) => assert_eq!(file.content(), "deep"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.contains("not found") || msg.contains("I/O error"),
                "unexpected error for deep chain: {msg}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Device symlinks (ignored — requires /dev)
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
#[ignore]
fn symlink_to_dev_zero() {
    let dir = tempdir().unwrap();
    let link = dir.path().join("zero_link");
    std::os::unix::fs::symlink("/dev/zero", &link).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes") || msg.contains("not a regular file"),
        "expected rejection of /dev/zero symlink, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
#[ignore]
fn symlink_to_dev_random() {
    let dir = tempdir().unwrap();
    let link = dir.path().join("random_link");
    std::os::unix::fs::symlink("/dev/random", &link).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes") || msg.contains("not a regular file"),
        "expected rejection of /dev/random symlink, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
#[ignore]
fn world_writable_root() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    std::fs::write(dir.path().join("public.txt"), "anyone can write").unwrap();

    let outside = tempdir().unwrap();
    std::fs::write(outside.path().join("escape.txt"), "external").unwrap();
    let link = dir.path().join("escape_link");
    std::os::unix::fs::symlink(outside.path().join("escape.txt"), &link).unwrap();

    let fs = fs_default(dir.path());

    let file = fs.read_file(&dir.path().join("public.txt")).unwrap();
    assert_eq!(file.content(), "anyone can write");

    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "containment should work even with 0777 root, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Composite unix error variants
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn composite_every_error_variant_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());

    let secret = dir.path().join("noperm.txt");
    std::fs::write(&secret, "secret").unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    let err = fs.read_file(&secret).unwrap_err();
    assert!(format!("{err}").contains("permission denied"));
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();

    let outside = tempdir().unwrap();
    std::fs::write(outside.path().join("ext.txt"), "external").unwrap();
    let link = dir.path().join("escape_link.txt");
    std::os::unix::fs::symlink(outside.path().join("ext.txt"), &link).unwrap();
    let err = fs.read_file(&link).unwrap_err();
    assert!(format!("{err}").contains("symlink escapes"));
}

// ---------------------------------------------------------------------------
// Chain escape and indirection attacks
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn security_symlink_chain_escape() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "leaked").unwrap();

    let link2 = dir.path().join("link2");
    let link1 = dir.path().join("link1");
    symlink(&secret, &link2).unwrap();
    symlink(&link2, &link1).unwrap();

    let fs = fs_default(dir.path());

    let err = fs.read_file(&link1).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "symlink chain escape should be blocked, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn security_symlink_with_dotdot_target() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();

    let link = subdir.join("evil_link");
    symlink("../../etc/passwd", &link).unwrap();

    let fs = fs_default(dir.path());

    let err = fs.read_file(&link).unwrap_err();
    let msg = format!("{err}");

    assert!(
        msg.contains("symlink escapes")
            || msg.contains("not found")
            || msg.contains("permission denied"),
        "symlink with .. target should be blocked, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn security_hardlink_contains_file() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    std::fs::write(&target, "content via hardlink").unwrap();

    let hardlink = dir.path().join("hardlink.txt");
    std::fs::hard_link(&target, &hardlink).unwrap();

    let fs = fs_default(dir.path());

    let file = fs.read_file(&hardlink).unwrap();
    assert_eq!(file.content(), "content via hardlink");
}

#[cfg(unix)]
#[test]
fn security_null_byte_in_path() {
    let dir = tempdir().unwrap();
    let safe_file = dir.path().join("safe.txt");
    std::fs::write(&safe_file, "content").unwrap();

    let fs = fs_default(dir.path());

    let file = fs.read_file(&safe_file).unwrap();
    assert_eq!(file.content(), "content");
}

// ---------------------------------------------------------------------------
// Composite operations with symlink escapes
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn security_validate_entry_symlink_escape() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "leaked").unwrap();

    let link = dir.path().join("escape");
    symlink(&secret, &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let result = fs.validate_entry(&scan, "escape");
    assert!(
        result.is_err(),
        "validate_entry should reject symlink escape as error, not None"
    );

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "error should mention escape"
    );
}

#[cfg(unix)]
#[test]
fn security_read_checked_symlink_escape() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "leaked").unwrap();

    let link = dir.path().join("config.txt");
    symlink(&secret, &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let result = fs.read_checked(&scan, "config.txt");
    assert!(
        result.is_err(),
        "read_checked should reject symlink escape as error"
    );

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "error should mention escape"
    );
}

#[cfg(unix)]
#[test]
fn security_read_by_ext_symlink_escape() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let secret = outside.path().join("secret.json");
    std::fs::write(&secret, r#"{"api_key":"leaked"}"#).unwrap();

    let link = dir.path().join("config.json");
    symlink(&secret, &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let result = fs.read_by_ext(&scan, "json");
    assert!(result.is_err(), "read_by_ext should reject symlink escape");

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "error should mention escape"
    );
}

#[cfg(unix)]
#[test]
fn security_dangling_symlink_in_scan() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let target = dir.path().join("does_not_exist.txt");
    let link = dir.path().join("dangling.txt");
    symlink(&target, &link).unwrap();

    let fs = fs_default(dir.path());
    let _scan = fs.scan().unwrap();

    let result = fs.read_file(&link);
    assert!(
        result.is_err(),
        "expected error reading dangling symlink, got {:?}",
        result
    );
    let err = result.unwrap_err();
    let kind = err.kind();
    assert!(
        kind == "not_found" || kind == "io_error",
        "unexpected error kind '{}' for dangling symlink",
        kind
    );
}

#[cfg(unix)]
#[test]
fn security_self_referential_symlink() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let link = dir.path().join("loop.txt");
    symlink(&link, &link).unwrap();

    let fs = fs_default(dir.path());
    let result = fs.read_file(&link);
    assert!(
        result.is_err(),
        "expected error for self-referential symlink, got {:?}",
        result
    );
    let err = result.unwrap_err();
    let kind = err.kind();
    assert!(
        kind == "io_error",
        "expected io_error for symlink loop, got {}",
        kind
    );
}

#[cfg(unix)]
#[test]
fn security_escape_via_nested_symlink_in_subdir() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let top_link = dir.path().join("escape_top.txt");
    symlink("/etc/passwd", &top_link).unwrap();

    let fs = fs_default(dir.path());

    let result = fs.read_file(&top_link);
    assert!(
        matches!(result, Err(FsError::SymlinkEscape { .. })),
        "expected SymlinkEscape, got {:?}",
        result
    );
}

#[cfg(unix)]
#[test]
fn security_root_via_symlink_resolves_correctly() {
    use std::os::unix::fs::symlink;

    let real_dir = tempdir().unwrap();
    let file_path = real_dir.path().join("file.txt");
    std::fs::write(&file_path, b"hello").unwrap();

    let link_parent = tempdir().unwrap();
    let link_to_root = link_parent.path().join("symlinked_root");
    symlink(real_dir.path(), &link_to_root).unwrap();

    let fs = SafeFs::new(&link_to_root, FsConfig::default()).unwrap();
    let result = fs.read_file(&file_path);
    assert!(
        result.is_ok(),
        "expected Ok reading file via symlinked root, got {:?}",
        result
    );
}

#[cfg(unix)]
#[test]
fn security_symlink_escape_error_hides_target() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let link = dir.path().join("escape.txt");
    symlink("/etc/passwd", &link).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&link).unwrap_err();

    assert!(
        matches!(err, FsError::SymlinkEscape { .. }),
        "expected SymlinkEscape, got {:?}",
        err
    );

    let msg = err.to_string();
    assert!(
        !msg.contains("/etc/passwd"),
        "SymlinkEscape error message leaked the resolved target: {}",
        msg
    );
    assert!(
        !msg.contains("passwd"),
        "SymlinkEscape error message leaked filename from target: {}",
        msg
    );
}
