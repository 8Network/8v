// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Read operation tests — safe_read, read_file, read_checked, read_by_ext,
//! BOM handling, size limits, error variants.

use o8v_fs::*;
use std::io::Write;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fs_default(root: &std::path::Path) -> SafeFs {
    SafeFs::new(root, FsConfig::default()).expect("SafeFs::new failed")
}

fn fs_with_config(root: &std::path::Path, config: FsConfig) -> SafeFs {
    SafeFs::new(root, config).expect("SafeFs::new failed")
}

// ---------------------------------------------------------------------------
// Basic read operations
// ---------------------------------------------------------------------------

#[test]
fn read_regular_file() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();

    let fs = fs_default(dir.path());
    let file = fs.read_file(&dir.path().join("hello.txt")).unwrap();
    assert_eq!(file.content(), "hello world");
    assert!(file.path().ends_with("hello.txt"));
}

#[test]
fn bom_stripped() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bom.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"\xEF\xBB\xBFcontent after bom").unwrap();

    let fs = fs_default(dir.path());
    let file = fs.read_file(&path).unwrap();
    assert_eq!(file.content(), "content after bom");
}

#[test]
fn bom_not_stripped_when_disabled() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bom.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"\xEF\xBB\xBFcontent").unwrap();

    let config = FsConfig {
        strip_bom: false,
        ..FsConfig::default()
    };
    let fs = fs_with_config(dir.path(), config);
    let file = fs.read_file(&path).unwrap();
    assert!(
        file.content().starts_with('\u{FEFF}'),
        "BOM should be preserved"
    );
    assert!(file.content().ends_with("content"));
}

#[test]
fn oversized_file_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("big.bin");
    let data = vec![b'x'; 11 * 1024 * 1024];
    std::fs::write(&path, &data).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("too large"), "expected TooLarge, got: {msg}");
}

/// SIZE-1: "file too large" error must include both the actual file size in bytes
/// and the configured limit in bytes so the user knows why it failed and what the
/// threshold is. Pre-fix: error only said "too large" with no numbers.
/// Post-fix: message matches `file too large ({actual} bytes, limit {limit}): {path}`.
#[test]
fn size1_too_large_error_includes_actual_size_and_limit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("big.bin");
    // 11 MiB — exceeds the default 10 MiB limit.
    let actual_size: usize = 11 * 1024 * 1024; // 11534336
    let limit: usize = 10 * 1024 * 1024; // 10485760
    let data = vec![b'x'; actual_size];
    std::fs::write(&path, &data).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&path).unwrap_err();
    let msg = format!("{err}");

    assert!(
        msg.contains(&actual_size.to_string()),
        "error must include actual file size ({actual_size} bytes); got: {msg}"
    );
    assert!(
        msg.contains(&limit.to_string()),
        "error must include the configured limit ({limit} bytes); got: {msg}"
    );
}

#[test]
fn file_at_size_limit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("exact.bin");
    let data = vec![b'A'; 10 * 1024 * 1024];
    std::fs::write(&path, &data).unwrap();

    let fs = fs_default(dir.path());
    let file = fs.read_file(&path).unwrap();
    assert_eq!(file.content().len(), 10 * 1024 * 1024);
}

#[test]
fn empty_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.txt");
    std::fs::write(&path, b"").unwrap();

    let fs = fs_default(dir.path());
    let file = fs.read_file(&path).unwrap();
    assert_eq!(file.content(), "");
}

#[cfg(unix)]
#[test]
fn permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("secret.txt");
    std::fs::write(&path, "secret").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("permission denied"),
        "expected PermissionDenied, got: {msg}"
    );
    assert!(!msg.contains("not found"), "must not be NotFound: {msg}");

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn nonexistent_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does_not_exist.txt");

    let fs = fs_default(dir.path());
    let err = fs.read_file(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("not found"), "expected NotFound, got: {msg}");
}

#[test]
fn directory_as_file() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("Cargo.toml");
    std::fs::create_dir(&subdir).unwrap();

    let fs = fs_default(dir.path());
    let err = fs.read_file(&subdir).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not a regular file") && msg.contains("directory"),
        "expected NotRegularFile(Directory), got: {msg}"
    );
}

#[test]
fn truncate_error_direct() {
    let long_err = "x".repeat(300);
    let result = truncate_error(&long_err, "check format");
    assert!(
        result.contains("(truncated)"),
        "expected truncation marker in: {result}"
    );
    let prefix = result.split("...").next().unwrap();
    assert!(prefix.len() <= 200, "prefix too long: {}", prefix.len());

    let short = truncate_error("short error", "hint");
    assert!(!short.contains("(truncated)"));
    assert!(short.contains("short error"));
    assert!(short.contains("hint"));
}

// ---------------------------------------------------------------------------
// read_checked
// ---------------------------------------------------------------------------

#[test]
fn read_checked_found() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.read_checked(&scan, "Cargo.toml").unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().content().contains("[package]"));
}

#[test]
fn read_checked_absent() {
    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.read_checked(&scan, "nonexistent.toml").unwrap();
    assert!(result.is_none());
}

#[test]
fn read_checked_directory() {
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Cargo.toml")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let err = fs.read_checked(&scan, "Cargo.toml").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not a regular file"),
        "expected NotRegularFile, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// read_by_ext
// ---------------------------------------------------------------------------

#[test]
fn read_by_ext_unique() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.read_by_ext(&scan, "toml").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().content(), "[package]");
}

#[test]
fn read_by_ext_ambiguous() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("App.csproj"), "<Project/>").unwrap();
    std::fs::write(dir.path().join("Lib.csproj"), "<Project/>").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let err = fs.read_by_ext(&scan, "csproj").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("multiple .csproj"),
        "expected Ambiguous, got: {msg}"
    );
}

#[test]
fn read_by_ext_absent() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hi").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.read_by_ext(&scan, "xyz").unwrap();
    assert!(result.is_none());
}

#[test]
fn read_by_ext_directory_with_extension() {
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join("weird.toml")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let result = fs.read_by_ext(&scan, "toml").unwrap();
    assert!(
        result.is_none(),
        "directory with .toml extension should not match"
    );
}

// ---------------------------------------------------------------------------
// Composite read pipelines
// ---------------------------------------------------------------------------

#[test]
fn composite_read_checked_pipeline() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"demo\"").unwrap();
    std::fs::write(dir.path().join("README.md"), "# Demo").unwrap();
    std::fs::create_dir(dir.path().join("build.rs")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let cargo = fs.read_checked(&scan, "Cargo.toml").unwrap().unwrap();
    assert!(cargo.content().contains("[package]"));

    assert!(fs.read_checked(&scan, "missing.txt").unwrap().is_none());

    let err = fs.read_checked(&scan, "build.rs").unwrap_err();
    assert!(format!("{err}").contains("not a regular file"));
}

#[test]
fn composite_read_by_ext_ambiguity() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.json"), "{}").unwrap();
    std::fs::write(dir.path().join("b.json"), "[]").unwrap();
    std::fs::write(dir.path().join("only.yaml"), "key: val").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let err = fs.read_by_ext(&scan, "json").unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("multiple .json"), "expected Ambiguous: {msg}");

    let yaml = fs.read_by_ext(&scan, "yaml").unwrap().unwrap();
    assert_eq!(yaml.content(), "key: val");

    assert!(fs.read_by_ext(&scan, "xml").unwrap().is_none());
}

#[test]
fn composite_every_error_variant() {
    let dir = tempdir().unwrap();

    let fs = fs_default(dir.path());
    let err = fs
        .read_file(&dir.path().join("no_such_file.txt"))
        .unwrap_err();
    assert!(format!("{err}").contains("not found"));

    std::fs::create_dir(dir.path().join("adir")).unwrap();
    let err = fs.read_file(&dir.path().join("adir")).unwrap_err();
    assert!(format!("{err}").contains("not a regular file"));

    let big_path = dir.path().join("huge.bin");
    std::fs::write(&big_path, vec![b'x'; 11 * 1024 * 1024]).unwrap();
    let err = fs.read_file(&big_path).unwrap_err();
    assert!(format!("{err}").contains("too large"));

    let err = FsError::InvalidContent {
        path: dir.path().join("bad.toml"),
        cause: "parse error".to_string(),
    };
    assert!(format!("{err}").contains("invalid content"));

    std::fs::write(dir.path().join("x.ext"), "1").unwrap();
    std::fs::write(dir.path().join("y.ext"), "2").unwrap();
    let scan = fs.scan().unwrap();
    let err = fs.read_by_ext(&scan, "ext").unwrap_err();
    assert!(format!("{err}").contains("multiple .ext"));
}

#[test]
fn composite_config_max_size() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("small.txt");
    std::fs::write(&path, "hello").unwrap();

    let config = FsConfig {
        max_file_size: 3,
        ..FsConfig::default()
    };
    let fs = fs_with_config(dir.path(), config);
    let err = fs.read_file(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("too large"),
        "3-byte limit should reject 5-byte file: {msg}"
    );

    let config = FsConfig {
        max_file_size: 100,
        ..FsConfig::default()
    };
    let fs = fs_with_config(dir.path(), config);
    let file = fs.read_file(&path).unwrap();
    assert_eq!(file.content(), "hello");
}

// ---------------------------------------------------------------------------
// Error boundary tests
// ---------------------------------------------------------------------------

#[test]
fn security_binary_file_content() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("binary.bin");
    std::fs::write(&path, b"\xff\xfe not valid utf8 \x80\x81").unwrap();

    let fs = fs_default(dir.path());
    let result = fs.read_file(&path);
    assert!(
        result.is_err(),
        "expected error reading binary file, got {:?}",
        result
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        "io_error",
        "expected Io error for binary content"
    );
}

#[test]
fn security_zero_max_file_size_rejects_all() {
    let dir = tempdir().unwrap();
    let empty_path = dir.path().join("empty.txt");
    let small_path = dir.path().join("small.txt");
    std::fs::write(&empty_path, b"").unwrap();
    std::fs::write(&small_path, b"hello").unwrap();

    let config = FsConfig {
        max_file_size: 0,
        ..FsConfig::default()
    };
    let fs = fs_with_config(dir.path(), config);

    let result = fs.read_file(&empty_path);
    assert!(
        result.is_ok(),
        "empty file should pass with limit=0, got {:?}",
        result
    );

    let result = fs.read_file(&small_path);
    assert!(
        matches!(result, Err(FsError::TooLarge { .. })),
        "expected TooLarge for 5-byte file with limit=0, got {:?}",
        result
    );
}

#[test]
fn security_error_kind_values_are_stable() {
    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());

    let missing_path = dir.path().join("missing.txt");
    let err = fs.read_file(&missing_path).unwrap_err();
    assert_eq!(
        err.kind(),
        "not_found",
        "NotFound should return 'not_found'"
    );

    let big_path = dir.path().join("big.txt");
    std::fs::write(&big_path, vec![0u8; 100]).unwrap();
    let config = FsConfig {
        max_file_size: 10,
        ..FsConfig::default()
    };
    let fs2 = fs_with_config(dir.path(), config);
    let err = fs2.read_file(&big_path).unwrap_err();
    assert_eq!(
        err.kind(),
        "too_large",
        "TooLarge should return 'too_large'"
    );

    let config3 = FsConfig {
        max_dir_entries: 0,
        ..FsConfig::default()
    };
    let fs3 = fs_with_config(dir.path(), config3);
    std::fs::write(dir.path().join("one.txt"), b"x").unwrap();
    let result = fs3.scan();
    assert!(
        result.is_ok(),
        "scan should return Ok even when limit exceeded"
    );
    let scan = result.unwrap();
    let err_opt = scan
        .errors()
        .iter()
        .find(|e| matches!(e, FsError::TooManyEntries { .. }));
    assert!(
        err_opt.is_some(),
        "scan.errors() should contain TooManyEntries with limit=0"
    );
    let err = err_opt.unwrap();
    assert_eq!(
        err.kind(),
        "too_many_entries",
        "TooManyEntries should return 'too_many_entries'"
    );
}

#[test]
fn security_invalid_content_error_truncated() {
    let long_cause = "x".repeat(500);
    let err = FsError::InvalidContent {
        path: std::path::PathBuf::from("/fake/path"),
        cause: long_cause,
    };
    let msg = err.to_string();
    assert!(
        msg.len() <= 400,
        "InvalidContent error message too long: {} chars (msg: {})",
        msg.len(),
        msg
    );
}

// ---------------------------------------------------------------------------
// Unreadable entry via scan + read_checked
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn scan_with_unreadable_entry() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("readable.txt"), "ok").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "hidden").unwrap();
    std::fs::set_permissions(
        dir.path().join("secret.txt"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    assert!(scan.by_name("readable.txt").is_some());
    assert!(scan.by_name("secret.txt").is_some());

    let err = fs.read_checked(&scan, "secret.txt").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("permission denied"),
        "expected PermissionDenied on read, got: {msg}"
    );

    std::fs::set_permissions(
        dir.path().join("secret.txt"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
}
