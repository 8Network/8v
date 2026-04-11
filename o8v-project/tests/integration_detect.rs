//! Basic integration tests for `detect_all`.

use o8v_project::{detect_all, ProjectKind, ProjectRoot, Stack};

/// This crate itself is a standalone Rust project.
#[test]
fn detect_self() {
    let root = ProjectRoot::new(env!("CARGO_MANIFEST_DIR")).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1, "should detect exactly one project");
    assert_eq!(
        r.projects()[0].name(),
        "o8v-project",
        "project name should be o8v-project"
    );
    assert_eq!(
        r.projects()[0].stack(),
        Stack::Rust,
        "project stack should be Rust"
    );
    assert!(
        matches!(r.projects()[0].kind(), ProjectKind::Standalone),
        "project kind should be Standalone"
    );
}

/// Directory with no manifest files — should detect nothing.
#[test]
fn detect_empty_with_non_manifest_files() {
    let dir = tempfile::tempdir().unwrap();
    // Files that are NOT manifests
    std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert!(
        r.projects().is_empty(),
        "non-manifest files should not trigger detection"
    );
}

/// Empty temp directory — zero projects, zero errors.
#[test]
fn detect_empty() {
    let dir = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        r.projects().is_empty(),
        "empty dir should detect no projects"
    );
    assert!(r.errors().is_empty(), "empty dir should have no errors");
}

/// Corrupt manifest produces an error, not silence.
#[test]
fn corrupt_manifest_surfaces_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        r.projects().is_empty(),
        "corrupt manifest should produce no projects"
    );
    assert_eq!(
        r.errors().len(),
        1,
        "corrupt manifest should produce exactly one error"
    );
}

/// Multiple corrupt manifests — all errors collected, not short-circuited.
#[test]
fn multiple_corrupt_manifests_all_surface() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();
    std::fs::write(dir.path().join("package.json"), "not json").unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "{{invalid").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        r.projects().is_empty(),
        "multiple corrupt manifests should produce no projects"
    );
    assert!(
        r.errors().len() >= 3,
        "errors from multiple stacks: got {}",
        r.errors().len()
    );
}

/// `DetectResult` convenience methods.
#[test]
fn detect_result_is_ok_when_no_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.is_ok(), "empty dir should have no errors");
    assert!(r.is_empty(), "empty dir should have no projects");
}

#[test]
fn detect_result_not_ok_when_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        !r.is_ok(),
        "corrupt manifest should make is_ok return false"
    );
    assert!(
        !r.is_empty(),
        "corrupt manifest should make is_empty return false"
    );
}

/// If the directory becomes unreadable after `ProjectRoot` creation,
/// `detect_all` returns a `DirectoryUnreadable` error.
#[cfg(unix)]
#[test]
fn detect_all_unreadable_directory() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(dir.path()).unwrap();

    // Remove read permission
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o000)).unwrap();

    let r = detect_all(&root);

    // Restore permissions before asserting (so cleanup works)
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(!r.is_ok(), "unreadable directory should produce an error");
    assert!(
        r.projects().is_empty(),
        "unreadable directory should produce no projects"
    );
    assert_eq!(
        r.errors().len(),
        1,
        "unreadable directory should produce exactly one error"
    );
}

/// Monorepo: root has no manifest, but two subdirectories each have one.
/// `detect_all` should perform a shallow scan and find both.
#[test]
fn detect_monorepo_subdirectories() {
    let dir = tempfile::tempdir().unwrap();

    // frontend/ — JavaScript project
    let frontend = dir.path().join("frontend");
    std::fs::create_dir(&frontend).unwrap();
    std::fs::write(
        frontend.join("package.json"),
        r#"{"name": "frontend", "version": "1.0.0"}"#,
    )
    .unwrap();

    // backend/ — Go project
    let backend = dir.path().join("backend");
    std::fs::create_dir(&backend).unwrap();
    std::fs::write(backend.join("go.mod"), "module backend\n\ngo 1.21\n").unwrap();

    // README.md at root — not a manifest
    std::fs::write(dir.path().join("README.md"), "# monorepo").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert_eq!(
        r.projects().len(),
        2,
        "should detect both subdirectory projects, got: {:?}",
        r.projects().iter().map(|p| p.name()).collect::<Vec<_>>()
    );

    let names: Vec<&str> = r.projects().iter().map(|p| p.name()).collect();
    assert!(
        names.contains(&"frontend"),
        "frontend project not found in {:?}",
        names
    );

    let stacks: Vec<Stack> = r.projects().iter().map(|p| p.stack()).collect();
    assert!(
        stacks.contains(&Stack::Go),
        "Go stack not found in {:?}",
        stacks
    );
}

/// Monorepo: root has a manifest — subdirectory scan must NOT trigger.
#[test]
fn detect_monorepo_skips_subdir_scan_when_root_has_project() {
    let dir = tempfile::tempdir().unwrap();

    // Root has a Rust project
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"root-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Subdirectory also has a manifest
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name": "sub-pkg", "version": "0.0.1"}"#,
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert_eq!(
        r.projects().len(),
        1,
        "root project found — subdirectory scan should not trigger"
    );
    assert_eq!(r.projects()[0].name(), "root-crate");
}

/// Monorepo: root has errors — subdirectory scan MUST trigger (harvest errors, continue).
/// The bug was that errors at root prevented discovering valid projects in subdirectories.
/// The fix: errors are collected but don't block subdirectory scanning.
#[test]
fn detect_monorepo_skips_subdir_scan_when_root_has_errors() {
    let dir = tempfile::tempdir().unwrap();

    // Root has a corrupt manifest
    std::fs::write(dir.path().join("Cargo.toml"), "{{invalid").unwrap();

    // Subdirectory has a valid manifest
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name": "sub-pkg", "version": "0.0.1"}"#,
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    // The subdirectory scan MUST trigger despite root errors.
    // Errors are harvested (collected) but don't prevent the scan.
    assert_eq!(
        r.projects().len(),
        1,
        "subdirectory project should be detected despite root error"
    );
    assert_eq!(
        r.projects()[0].name(),
        "sub-pkg",
        "should find the subdirectory project"
    );
    assert_eq!(
        r.errors().len(),
        1,
        "should still harvest the error from the corrupt root manifest"
    );
}

/// Monorepo: known skip directories (node_modules, .git, target) are ignored.
#[test]
fn detect_monorepo_skips_known_noise_directories() {
    let dir = tempfile::tempdir().unwrap();

    // node_modules has a package.json — must be skipped
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir(&node_modules).unwrap();
    std::fs::write(
        node_modules.join("package.json"),
        r#"{"name": "some-dep", "version": "1.0.0"}"#,
    )
    .unwrap();

    // .git directory — must be skipped
    let git = dir.path().join(".git");
    std::fs::create_dir(&git).unwrap();

    // A real project in a non-skipped subdirectory
    let app = dir.path().join("app");
    std::fs::create_dir(&app).unwrap();
    std::fs::write(
        app.join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert_eq!(
        r.projects().len(),
        1,
        "only 'app' should be detected, not node_modules/.git"
    );
    assert_eq!(r.projects()[0].name(), "app");
}

/// Regression test: build.gradle.kts must be claimed by Kotlin detector only, not Java.
/// If Java incorrectly claims build.gradle.kts, this test will detect dual projects.
#[test]
fn java_does_not_claim_build_gradle_kts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("build.gradle.kts"),
        "plugins {\n    id(\"org.jetbrains.kotlin.jvm\") version \"1.9.0\"\n}\n",
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "unexpected errors: {:?}", r.errors());
    assert_eq!(
        r.projects().len(),
        1,
        "should detect exactly one project (Kotlin), not Java+Kotlin"
    );
    assert_eq!(
        r.projects()[0].stack(),
        Stack::Kotlin,
        "build.gradle.kts should be claimed by Kotlin detector only"
    );
}
