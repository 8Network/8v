// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Reusable project scaffolding for tests.
//!
//! `TempProject` creates isolated temporary projects with containment safety.
//! All file operations go through `o8v_fs` — no raw `std::fs` in test infrastructure.

use o8v_fs::ContainmentRoot;
use std::path::{Path, PathBuf};

/// A temporary project directory with containment safety.
///
/// Owns the `TempDir` — the directory lives as long as this struct.
/// Provides safe file operations via `o8v_fs` containment.
pub struct TempProject {
    dir: tempfile::TempDir,
    root: ContainmentRoot,
}

impl TempProject {
    /// Empty temporary project directory with no files.
    ///
    /// Use `write_file()` and `create_dir()` to populate it.
    pub fn empty() -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = ContainmentRoot::new(dir.path()).expect("containment root");
        Self { dir, root }
    }

    /// Minimal passing Rust project: `cargo check`, `clippy`, and `cargo fmt --check` all pass.
    ///
    /// Package name: `"test-app"`, edition 2021.
    pub fn rust_passing() -> Self {
        let fixture = Self::testkit_fixture("rust-passing");
        Self::from_fixture(&fixture)
    }

    /// Rust project with clippy and fmt violations.
    ///
    /// - clippy: `needless_pass_by_ref_mut` (unused mut parameter)
    /// - rustfmt: single-line function body fails `cargo fmt --check`
    pub fn rust_violated() -> Self {
        let fixture = Self::testkit_fixture("rust-violated");
        Self::from_fixture(&fixture)
    }

    /// Polyglot project with 6 stacks — each has at least one violation.
    ///
    /// Stacks: Rust, Python, Go, TypeScript, Dockerfile, Terraform.
    /// Used by agent benchmarks to measure token efficiency on multi-stack projects.
    pub fn polyglot_violated() -> Self {
        let fixture = Self::testkit_fixture("polyglot-violated");
        Self::from_fixture(&fixture)
    }

    /// The project directory path.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// ContainmentRoot for safe file operations within this project.
    pub fn containment(&self) -> &ContainmentRoot {
        &self.root
    }

    /// Write a file safely within this project.
    ///
    /// `relative` is joined to the project root. Parent directories must exist.
    pub fn write_file(&self, relative: &str, content: &[u8]) -> Result<(), o8v_fs::FsError> {
        let path = self.dir.path().join(relative);
        o8v_fs::safe_write(&path, &self.root, content)
    }

    /// Create a subdirectory safely within this project.
    pub fn create_dir(&self, relative: &str) -> Result<(), o8v_fs::FsError> {
        let path = self.dir.path().join(relative);
        o8v_fs::safe_create_dir(&path, &self.root)
    }

    /// Resolve a fixture path within the testkit crate's `tests/fixtures/` directory.
    fn testkit_fixture(name: &str) -> PathBuf {
        // env! resolves CARGO_MANIFEST_DIR at compile time of THIS crate (o8v-testkit),
        // so this always points to o8v-testkit/tests/fixtures/ regardless of which crate
        // calls rust_passing(), rust_violated(), or polyglot_violated() at runtime.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    /// Create a TempProject by copying a fixture directory.
    /// All file operations go through o8v-fs containment.
    ///
    /// The source `fixture_path` is read with `std::fs` (it lives outside containment).
    /// All writes to the temp destination go through `o8v_fs`.
    pub fn from_fixture(fixture_path: &Path) -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = ContainmentRoot::new(dir.path()).expect("containment root");

        copy_dir_recursive(fixture_path, dir.path(), &root);

        Self { dir, root }
    }
}

/// Recursively copy `src` into `dest`, preserving relative structure.
/// Reads from `src` using `std::fs` (external source); writes via `o8v_fs`.
fn copy_dir_recursive(src: &Path, dest: &Path, root: &ContainmentRoot) {
    let entries = std::fs::read_dir(src).expect("read fixture source dir");

    for entry in entries {
        let entry = entry.expect("read fixture dir entry");
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        let file_type = entry.file_type().expect("read fixture entry file type");

        if file_type.is_dir() {
            o8v_fs::safe_create_dir(&dest_path, root).expect("safe_create_dir in fixture copy");
            copy_dir_recursive(&entry_path, &dest_path, root);
        } else if file_type.is_file() {
            let content = std::fs::read(&entry_path).expect("read fixture source file");
            o8v_fs::safe_write(&dest_path, root, &content).expect("safe_write in fixture copy");
        }
        // Symlinks are intentionally skipped — fixtures should not contain symlinks.
    }
}

/// Resolve a fixture path relative to a crate's test fixtures.
///
/// Example:
/// ```rust,ignore
/// fixture_path("o8v", "agent-benchmark/rust-violated")
/// ```
pub fn fixture_path(crate_name: &str, fixture_name: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let workspace_root = Path::new(&manifest_dir).parent().expect("workspace root");
    workspace_root
        .join(crate_name)
        .join("tests")
        .join("fixtures")
        .join(fixture_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_fixture_copies_files() {
        // Create a temp "fixture" source using TempProject
        let source = TempProject::empty();
        source.create_dir("src").unwrap();
        source
            .write_file("Cargo.toml", b"[package]\nname = \"test\"")
            .unwrap();
        source.write_file("src/main.rs", b"fn main() {}").unwrap();

        let project = TempProject::from_fixture(source.path());
        assert!(project.path().join("Cargo.toml").exists());
        assert!(project.path().join("src/main.rs").exists());

        let content = std::fs::read_to_string(project.path().join("Cargo.toml")).unwrap();
        assert!(content.contains("test"));
    }
}
