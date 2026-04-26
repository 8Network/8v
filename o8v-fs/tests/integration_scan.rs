// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Directory scanning tests — scan, has_entry, validate_entry, DirEntry indexing.

use o8v_fs::*;
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
// Basic scan operations
// ---------------------------------------------------------------------------

#[test]
fn scan_empty_dir() {
    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert_eq!(scan.entries().len(), 0);
    assert_eq!(scan.errors().len(), 0);
}

#[test]
fn scan_with_files() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(dir.path().join("README.md"), "# hi").unwrap();
    std::fs::write(dir.path().join("lib.rs"), "fn main(){}").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    assert_eq!(scan.entries().len(), 3);
    assert!(scan.by_name("Cargo.toml").is_some());
    assert!(scan.by_name("README.md").is_some());
    assert!(scan.by_name("lib.rs").is_some());
    assert!(scan.by_name("nonexistent").is_none());

    let toml_entries: Vec<_> = scan.entries_with_extension("toml").collect();
    assert_eq!(toml_entries.len(), 1);
    assert_eq!(toml_entries[0].name, "Cargo.toml");

    let rs_entries: Vec<_> = scan.entries_with_extension("rs").collect();
    assert_eq!(rs_entries.len(), 1);
}

// ---------------------------------------------------------------------------
// validate_entry
// ---------------------------------------------------------------------------

#[test]
fn validate_entry_file() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.validate_entry(&scan, "package.json").unwrap();
    assert!(result.is_some());
    let entry = result.unwrap();
    assert_eq!(entry.name, "package.json");
    assert!(entry.is_file());
}

#[test]
fn validate_entry_absent() {
    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let result = fs.validate_entry(&scan, "nope.txt").unwrap();
    assert!(result.is_none());
}

#[test]
fn validate_entry_directory() {
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    let err = fs.validate_entry(&scan, "src").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not a regular file"),
        "expected NotRegularFile, got: {msg}"
    );
}

#[test]
fn has_entry_true_false() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("exists.txt"), "yes").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert!(scan.has_entry("exists.txt"));
    assert!(!scan.has_entry("missing.txt"));
}

#[test]
fn root_and_dir_name() {
    let dir = tempdir().unwrap();
    let fs = fs_default(dir.path());

    assert!(fs.root().exists());
    assert!(fs.root().is_absolute());

    let name = fs.dir_name();
    assert!(name.is_some(), "dir_name should be Some for tempdir");
    let name = name.unwrap();
    assert!(!name.is_empty());
    assert_eq!(name, fs.root().file_name().unwrap().to_str().unwrap());
}

// ---------------------------------------------------------------------------
// Non-UTF-8 filenames
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_filename() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("good.txt"), "ok").unwrap();

    let bad_name = OsStr::from_bytes(b"bad\xff.txt");
    let bad_path = dir.path().join(bad_name);
    std::fs::write(&bad_path, "invalid name").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    assert!(scan.by_name("good.txt").is_some());
    assert_eq!(
        scan.entries().len(),
        1,
        "non-UTF-8 filename should be skipped"
    );
    assert_eq!(scan.errors().len(), 1);
    let err = &scan.errors()[0];
    let msg = format!("{err}");
    assert!(msg.contains("non-UTF-8 filename skipped"));
}

#[cfg(target_os = "macos")]
#[test]
fn non_utf8_filename_macos_fallback() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("good.txt"), "ok").unwrap();
    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();
    assert_eq!(scan.entries().len(), 1);
    assert!(scan.by_name("good.txt").is_some());
    assert_eq!(scan.errors().len(), 0);
}

// ---------------------------------------------------------------------------
// Symlink in scan
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn symlink_in_dir_scan() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("real.txt");
    std::fs::write(&target, "real").unwrap();
    let link = dir.path().join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let entry = scan.by_name("link.txt").expect("symlink should be indexed");
    assert!(entry.is_symlink, "entry should be marked as symlink");
    assert!(entry.is_file(), "symlink to file should report as file");

    let real_entry = scan
        .by_name("real.txt")
        .expect("real file should be indexed");
    assert!(!real_entry.is_symlink);
}

// ---------------------------------------------------------------------------
// Composite scan operations
// ---------------------------------------------------------------------------

#[test]
fn composite_validate_entry_type_check() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("real.txt"), "ok").unwrap();
    std::fs::create_dir(dir.path().join("fake_file")).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let entry = fs.validate_entry(&scan, "real.txt").unwrap().unwrap();
    assert!(entry.is_file());

    let err = fs.validate_entry(&scan, "fake_file").unwrap_err();
    assert!(format!("{err}").contains("not a regular file"));

    assert!(fs.validate_entry(&scan, "nope").unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn composite_harvest_yield() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("good1.txt"), "ok1").unwrap();
    std::fs::write(dir.path().join("good2.txt"), "ok2").unwrap();

    std::os::unix::fs::symlink(
        dir.path().join("nonexistent_target"),
        dir.path().join("dangling_link.txt"),
    )
    .unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    assert!(scan.by_name("good1.txt").is_some());
    assert!(scan.by_name("good2.txt").is_some());

    assert!(
        !scan.errors().is_empty(),
        "dangling symlink should produce scan error"
    );
    assert!(
        scan.by_name("dangling_link.txt").is_none(),
        "dangling symlink should not be in entries"
    );
}

#[test]
fn composite_harvest_yield_take_errors() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "ok").unwrap();

    let fs = fs_default(dir.path());
    let mut scan = fs.scan().unwrap();

    assert_eq!(scan.entries().len(), 1);
    let errors = scan.take_errors();
    assert!(errors.is_empty());
    assert_eq!(scan.errors().len(), 0);
}

#[test]
fn composite_root_dir_name() {
    let dir = tempdir().unwrap();
    let specific = dir.path().join("my-project");
    std::fs::create_dir(&specific).unwrap();

    let fs = fs_default(&specific);
    assert!(fs.root().ends_with("my-project"));
    assert_eq!(fs.dir_name(), Some("my-project"));
}

#[test]
fn scan_extension_case_normalization() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("App.CSPROJ"), "<Project/>").unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let entry = scan.by_name("App.CSPROJ").unwrap();
    assert_eq!(entry.extension.as_deref(), Some("csproj"));

    let entries: Vec<_> = scan.entries_with_extension("csproj").collect();
    assert_eq!(entries.len(), 1);
}

// ---------------------------------------------------------------------------
// Scan limits
// ---------------------------------------------------------------------------

#[test]
fn security_zero_max_dir_entries_triggers_limit() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();

    let config = FsConfig {
        max_dir_entries: 0,
        ..FsConfig::default()
    };
    let fs = fs_with_config(dir.path(), config);

    let result = fs.scan();
    assert!(
        result.is_ok(),
        "scan should return Ok even when limit exceeded"
    );
    let scan = result.unwrap();
    let has_too_many = scan
        .errors()
        .iter()
        .any(|e| matches!(e, FsError::TooManyEntries { .. }));
    assert!(
        has_too_many,
        "scan.errors() should contain TooManyEntries with limit=0"
    );
}

// ---------------------------------------------------------------------------
// validate_entry with symlinks
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn validate_entry_symlink_within_root() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("real.txt");
    std::fs::write(&target, "content").unwrap();
    let link = dir.path().join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let entry = fs.validate_entry(&scan, "link.txt").unwrap().unwrap();
    assert!(entry.is_symlink);
    assert!(entry.is_file());
}

#[cfg(unix)]
#[test]
fn validate_entry_symlink_escape() {
    let dir = tempdir().unwrap();
    let outside = tempdir().unwrap();
    std::fs::write(outside.path().join("ext.txt"), "external").unwrap();
    let link = dir.path().join("escape.txt");
    std::os::unix::fs::symlink(outside.path().join("ext.txt"), &link).unwrap();

    let fs = fs_default(dir.path());
    let scan = fs.scan().unwrap();

    let err = fs.validate_entry(&scan, "escape.txt").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink escapes"),
        "validate_entry should catch symlink escape: {msg}"
    );
}
