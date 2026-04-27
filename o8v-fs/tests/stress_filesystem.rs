// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_fs::*;
use tempfile::tempdir;

fn fs_default(root: &std::path::Path) -> SafeFs {
    SafeFs::new(root, FsConfig::default()).expect("SafeFs::new failed")
}

#[cfg(unix)]
#[test]
#[ignore]
fn concurrent_dir_modification() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let dir = tempdir().unwrap();
    for i in 0..50 {
        std::fs::write(
            dir.path().join(format!("file_{i}.txt")),
            format!("content {i}"),
        )
        .unwrap();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let dir_path = dir.path().to_path_buf();
    let remover = std::thread::spawn(move || {
        let mut i = 0;
        while !stop_clone.load(Ordering::Relaxed) && i < 50 {
            let _ = std::fs::remove_file(dir_path.join(format!("file_{i}.txt")));
            i += 1;
        }
    });
    let fs = fs_default(dir.path());
    let result = fs.scan();
    stop.store(true, Ordering::Relaxed);
    remover.join().unwrap();
    if let Ok(scan) = result {
        let total = scan.entries().len() + scan.errors().len();
        assert!(total <= 50, "should not exceed original count: {total}");
    }
}

#[test]
fn stress_deeply_nested_directories() {
    let dir = tempdir().unwrap();
    let mut current = dir.path().to_path_buf();
    for i in 0..50 {
        let subdir = current.join(format!("level_{:02}", i));
        std::fs::create_dir(&subdir).unwrap();
        current = subdir;
    }
    std::fs::write(current.join("deepest.txt"), "at bottom").unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let _ = scan;
    let deepest_path = current.join("deepest.txt");
    let file = fs.read_file(&deepest_path).unwrap();
    assert_eq!(file.content(), "at bottom");
}

#[test]
fn stress_very_long_filename() {
    let dir = tempdir().unwrap();
    let long_name = "a".repeat(250) + ".txt";
    assert_eq!(long_name.len(), 254);
    let path = dir.path().join(&long_name);
    std::fs::write(&path, "content of long filename").unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let entry = scan
        .by_name(&long_name)
        .expect("long filename not found in scan");
    assert_eq!(entry.name, long_name);
    let file = fs.read_file(&path).unwrap();
    assert_eq!(file.content(), "content of long filename");
}

#[test]
fn stress_many_files_in_one_dir() {
    let dir = tempdir().unwrap();
    for i in 0..1000 {
        let name = format!("file_{:04}.txt", i);
        let content = format!("content {}", i);
        std::fs::write(dir.path().join(&name), &content).unwrap();
    }
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert_eq!(scan.entries().len(), 1000, "expected 1000 entries");
    assert!(scan.by_name("file_0000.txt").is_some());
    assert!(scan.by_name("file_0500.txt").is_some());
    assert!(scan.by_name("file_0999.txt").is_some());
    let file = fs.read_file(&dir.path().join("file_0500.txt")).unwrap();
    assert_eq!(file.content(), "content 500");
}

#[test]
fn stress_empty_files() {
    let dir = tempdir().unwrap();
    for i in 0..100 {
        let name = format!("empty_{:03}.txt", i);
        std::fs::write(dir.path().join(&name), b"").unwrap();
    }
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert_eq!(scan.entries().len(), 100);
    for i in 0..100 {
        let name = format!("empty_{:03}.txt", i);
        let file = fs.read_file(&dir.path().join(&name)).unwrap();
        assert_eq!(file.content(), "", "empty file {} was not empty", i);
    }
}

#[cfg(unix)]
#[test]
fn stress_symlink_chains() {
    use std::os::unix::fs as unix_fs;
    let dir = tempdir().unwrap();
    let actual = dir.path().join("actual.txt");
    std::fs::write(&actual, "at the end of the chain").unwrap();
    let link_c = dir.path().join("link_c");
    unix_fs::symlink(&actual, &link_c).unwrap();
    let link_b = dir.path().join("link_b");
    unix_fs::symlink(&link_c, &link_b).unwrap();
    let link_a = dir.path().join("link_a");
    unix_fs::symlink(&link_b, &link_a).unwrap();
    let fs = fs_default(dir.path());
    let file = fs.read_file(&link_a).unwrap();
    assert_eq!(file.content(), "at the end of the chain");
    let scan = fs.scan().unwrap();
    assert!(scan.by_name("actual.txt").is_some());
    assert!(scan.by_name("link_a").is_some());
    assert!(scan.by_name("link_b").is_some());
    assert!(scan.by_name("link_c").is_some());
}

#[cfg(unix)]
#[test]
fn stress_circular_symlinks() {
    use std::os::unix::fs as unix_fs;
    let dir = tempdir().unwrap();
    let link_a = dir.path().join("link_a");
    let link_b = dir.path().join("link_b");
    unix_fs::symlink(&link_b, &link_a).unwrap();
    unix_fs::symlink(&link_a, &link_b).unwrap();
    let fs = fs_default(dir.path());
    let result = fs.read_file(&link_a);
    assert!(
        result.is_err(),
        "circular symlink should error, not succeed"
    );
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("symlink")
            || msg.to_lowercase().contains("escapes")
            || msg.to_lowercase().contains("too many levels"),
        "expected symlink-related error, got: {msg}"
    );
    let _scan_result = fs.scan();
}

#[cfg(unix)]
#[test]
fn stress_permission_denied_files() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    for i in 0..10 {
        let name = format!("file_{:02}.txt", i);
        std::fs::write(dir.path().join(&name), format!("content {}", i)).unwrap();
        if i % 2 == 0 {
            std::fs::set_permissions(
                dir.path().join(&name),
                std::fs::Permissions::from_mode(0o000),
            )
            .unwrap();
        }
    }
    let fs = fs_default(dir.path());
    for i in [1, 3, 5, 7, 9] {
        let name = format!("file_{:02}.txt", i);
        let file = fs.read_file(&dir.path().join(&name)).unwrap();
        assert_eq!(file.content(), format!("content {}", i));
    }
    for i in [0, 2, 4, 6, 8] {
        let name = format!("file_{:02}.txt", i);
        let err = fs.read_file(&dir.path().join(&name)).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("permission denied"),
            "expected PermissionDenied for {}, got: {msg}",
            name
        );
    }
    for i in [0, 2, 4, 6, 8] {
        let name = format!("file_{:02}.txt", i);
        std::fs::set_permissions(
            dir.path().join(&name),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
    }
}

#[test]
fn stress_special_characters_in_filenames() {
    let dir = tempdir().unwrap();
    let filenames = vec![
        "file with spaces.txt",
        "file-with-dashes.txt",
        "file_with_underscores.txt",
        "file.multiple.dots.txt",
        "file(with)parens.txt",
        "file[with]brackets.txt",
        "file{with}braces.txt",
        "file'with'quotes.txt",
        "file\"with\"doublequotes.txt",
        "файл.txt",
        "文件.txt",
        "ファイル.txt",
    ];
    for filename in &filenames {
        let path = dir.path().join(filename);
        std::fs::write(&path, format!("content of {}", filename)).unwrap();
    }
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    for filename in &filenames {
        let entry = scan
            .by_name(filename)
            .unwrap_or_else(|| panic!("filename not found in scan: {}", filename));
        assert_eq!(entry.name, *filename);
        let path = dir.path().join(filename);
        let file = fs.read_file(&path).unwrap();
        assert_eq!(file.content(), format!("content of {}", filename));
    }
}

#[test]
fn stress_hidden_files() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join(".hidden_file"), "hidden content").unwrap();
    std::fs::write(dir.path().join(".bashrc"), "bashrc content").unwrap();
    std::fs::write(dir.path().join(".config"), "config content").unwrap();
    std::fs::create_dir(dir.path().join(".hidden_dir")).unwrap();
    std::fs::write(
        dir.path().join(".hidden_dir").join(".nested_hidden"),
        "nested hidden",
    )
    .unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert!(scan.by_name(".hidden_file").is_some());
    assert!(scan.by_name(".bashrc").is_some());
    assert!(scan.by_name(".config").is_some());
    let file = fs.read_file(&dir.path().join(".hidden_file")).unwrap();
    assert_eq!(file.content(), "hidden content");
    let file = fs.read_file(&dir.path().join(".bashrc")).unwrap();
    assert_eq!(file.content(), "bashrc content");
    let hidden_dir_fs = SafeFs::new(dir.path().join(".hidden_dir"), FsConfig::default()).unwrap();
    let hidden_scan = hidden_dir_fs.scan().unwrap();
    assert!(hidden_scan.by_name(".nested_hidden").is_some());
}

#[test]
fn stress_empty_directory() {
    let dir = tempdir().unwrap();
    let empty_subdir = dir.path().join("empty");
    std::fs::create_dir(&empty_subdir).unwrap();
    let fs = SafeFs::new(&empty_subdir, FsConfig::default()).unwrap();
    let scan = fs.scan().unwrap();
    assert_eq!(scan.entries().len(), 0);
    assert_eq!(scan.errors().len(), 0);
    assert!(scan.by_name("nonexistent").is_none());
}

#[test]
fn append_concurrent_50_no_spurious_blank_lines() {
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("concurrent_append.txt");
    // Seed without trailing newline to exercise the separator logic.
    std::fs::write(&file_path, b"seed").unwrap();

    let root = o8v_fs::ContainmentRoot::new(dir.path()).unwrap();
    let root = Arc::new(root);
    let file_path = Arc::new(file_path);

    let handles: Vec<_> = (0..50)
        .map(|i| {
            let root = Arc::clone(&root);
            let path = Arc::clone(&file_path);
            std::thread::spawn(move || {
                let line = format!("line{i}");
                o8v_fs::safe_append_with_separator(&path, &root, line.as_bytes())
                    .expect("append failed");
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    let bytes = std::fs::read(file_path.as_ref()).unwrap();
    let content = String::from_utf8(bytes).unwrap();

    // No two consecutive newlines (no spurious blank lines).
    assert!(
        !content.contains("\n\n"),
        "spurious blank line detected: {:?}",
        content
    );

    // Exactly 51 non-empty lines: 1 seed + 50 line{i}.
    let non_empty: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        non_empty.len(),
        51,
        "expected 51 non-empty lines, got {}: {:?}",
        non_empty.len(),
        content
    );

    assert!(non_empty.contains(&"seed"), "seed line missing");
    for i in 0..50 {
        let expected = format!("line{i}");
        assert!(non_empty.contains(&expected.as_str()), "missing {expected}");
    }
}

/// PR#4-R10: Concurrent 50-thread append on a CRLF file without trailing newline.
/// Each thread must detect CRLF and append with \r\n as the separator.
/// Today each thread misdetects the file as LF (same root cause as PR#4-R1),
/// so the result contains bare \n separators mixed into CRLF content.
#[test]
fn append_concurrent_50_crlf_seed_no_trailing_newline_preserves_crlf() {
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("crlf_concurrent.txt");
    // CRLF seed with no trailing newline -- triggers the is_crlf detection bug.
    std::fs::write(&file_path, b"seed\r\nstart").unwrap();

    let root = o8v_fs::ContainmentRoot::new(dir.path()).unwrap();
    let root = Arc::new(root);
    let file_path = Arc::new(file_path);

    let handles: Vec<_> = (0..50)
        .map(|i| {
            let root = Arc::clone(&root);
            let path = Arc::clone(&file_path);
            std::thread::spawn(move || {
                let line = format!("line{i}");
                o8v_fs::safe_append_with_separator(&path, &root, line.as_bytes())
                    .expect("append failed");
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    let bytes = std::fs::read(file_path.as_ref()).unwrap();

    // Every \n must be preceded by \r (pure CRLF).
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            assert!(
                i > 0 && bytes[i - 1] == b'\r',
                "bare \\n at byte {} -- CRLF not preserved under concurrent append; file: {:?}",
                i,
                bytes
            );
        }
        if b == b'\r' {
            assert!(
                i + 1 < bytes.len() && bytes[i + 1] == b'\n',
                "lone \\r at byte {} -- malformed CRLF sequence; file: {:?}",
                i,
                bytes
            );
        }
    }
}
