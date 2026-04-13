// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! # o8v-project
//!
//! Project detection for 8v. Identifies what stack a directory contains
//! by reading manifest files.
//!
//! No network access. No home directory access.
//! Reads only the directory you point it at.
//!
//! ```text
//! o8v-fs  →  o8v-project  →  o8v-check  →  o8v-core(render)  →  o8v(cli)
//! (safe I/O)  (what is it)   (is it ok)    (present)             (interface)
//!                                  ↑
//!                             o8v-process
//! ```
//!
//! ## Pipeline
//!
//! [`ProjectRoot`] → [`o8v_fs::SafeFs`] → Scan → Detectors → [`DetectResult`]
//!
//! 1. **[`ProjectRoot`]**: Validated, canonical, absolute directory path.
//!    Guarantees the directory existed at construction time.
//! 2. **Scan**: Single `read_dir` pass. Indexes files by name and extension.
//!    No stack knowledge — detectors query what they need.
//! 3. **Detectors**: Each stack implements `Detect`. Reads its manifest from
//!    the scan, deserializes into typed structs, extracts name/version/kind.
//! 4. **[`DetectResult`]**: Projects found + errors encountered. Both collected —
//!    detection errors are surfaced, not swallowed.
//!
//! ## Detection Contract
//!
//! Each detector returns one of three outcomes:
//!
//! - `Ok(Some(Project))` — stack detected, manifest valid
//! - `Ok(None)` — stack not present (no manifest, or config-only file)
//! - `Err(DetectError)` — manifest exists but is invalid
//!
//! "Not present" is different from "invalid." A directory without `Cargo.toml`
//! returns `None`. A directory with an invalid `Cargo.toml` returns `Err`.
//!
//! ## Glossary
//!
//! These terms are used consistently across the crate, CLI output, and docs.
//! For the authoritative definitions, see `docs/glossary.md`.
//!
//! - **[`Stack`]**: Technology ecosystem identified by its manifest file.
//!   Rust, JavaScript, TypeScript, Python, `DotNet`. Not "programming language" —
//!   a stack includes the build system, package manager, and conventions.
//!   JavaScript and TypeScript are separate stacks because they have different
//!   toolchains, lint rules, and build systems.
//!
//! - **[`Project`]**: A directory where a stack was detected. Has a name,
//!   optional version, a stack, and a kind (Standalone or Compound).
//!
//! - **[`Compound`](ProjectKind::Compound)**: A project that contains member
//!   sub-projects. Cargo workspace, npm/yarn workspaces, uv workspace,
//!   .sln/.slnx solution. Members are stored as raw strings (may be glob patterns).
//!
//! - **[`Standalone`](ProjectKind::Standalone)**: A project that is not a compound project.
//!   Single crate, single package, single .csproj.
//!
//! - **Manifest**: The file that declares a project. `Cargo.toml`, `package.json`,
//!   `pyproject.toml`, `.csproj`, `.sln`, `.slnx`. Specifically the file that
//!   identifies the stack — not every file in the directory.
//!
//! - **Config-only**: A manifest file that exists but doesn't declare a project.
//!   Example: `pyproject.toml` with only `[tool.ruff]`. Detection returns `None`,
//!   not `Err` — the file is valid, it's just not a project.
//!
//! - **Virtual workspace**: Cargo-specific. A workspace root with `[workspace]`
//!   but no `[package]` section. The workspace itself isn't a package. Name is
//!   derived from the directory.
//!
//! - **Inherited field**: Cargo-specific. `version.workspace = true` means the
//!   value is defined at the workspace root, not in this manifest. Detection
//!   returns `None` for the value.
//!
//! Note: "Workspace" in 8v refers to the managed directory tree (created by `8v init`),
//! not compound projects. Compound projects are sometimes called "workspaces" in
//! individual stack ecosystems (Cargo, npm, etc.), but are called **compound projects**
//! in 8v to avoid confusion with 8v's Workspace concept.

mod detectors;
pub mod error;
pub mod path;
pub mod project;
pub mod stack;

/// Shared error truncation utility for detectors.
/// Used by dotnet's `parse_sln_projects` and any detector that does
/// custom parsing with error sanitization. Not in o8v-fs (parsing concern).
pub(crate) fn truncate_error(error: &str, hint: &str) -> String {
    o8v_fs::truncate_error(error, hint)
}

pub use detectors::DetectResult;
pub use error::{DetectError, PathError, ProjectError};
pub use o8v_core::project::ProjectRoot;
pub use project::{Project, ProjectKind};
pub use o8v_core::project::Stack;

use o8v_fs::{FileKind, FileSystem, FsConfig, SafeFs};

/// Scan `path` with the standard project scan policy.
///
/// Single source of truth for how `o8v-project` scans directories.
/// Returns `None` on error (caller adds to error list).
fn scan_root(path: &std::path::Path) -> Result<(SafeFs, o8v_fs::DirScan), o8v_fs::FsError> {
    let fs = SafeFs::new(path, FsConfig::default())?;
    let scan = fs.scan()?;
    Ok((fs, scan))
}

/// Known directories that never contain project manifests.
/// Skipped during shallow subdirectory scanning.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    "coverage",
    ".tox",
    "obj",
    "bin",
];

/// Detect all projects in a validated directory.
///
/// Creates a `SafeFs` for guarded reads, scans the directory once, then
/// runs all detectors. Errors are returned alongside successes.
///
/// If the root scan finds zero projects and zero errors, performs one additional
/// level of scanning: iterates the root's immediate subdirectories and runs
/// detection on each. Known non-project directories (node_modules, .git, target,
/// etc.) are skipped. This supports monorepos where manifests live one level
/// below the root.
///
/// # Example
///
/// ```no_run
/// use o8v_project::{ProjectRoot, detect_all};
///
/// let root = ProjectRoot::new(".").expect("invalid path");
/// let result = detect_all(&root);
///
/// for project in result.projects() {
///     println!("{} ({}, {})",
///         project.name(),
///         project.stack(),
///         if matches!(project.kind(), o8v_project::ProjectKind::Compound { .. }) {
///             "compound"
///         } else {
///             "standalone"
///         },
///     );
/// }
///
/// for error in result.errors() {
///     eprintln!("error: {error}");
/// }
/// ```
#[must_use]
pub fn detect_all(root: &ProjectRoot) -> DetectResult {
    let (fs, mut scan) = match scan_root(root.as_path()) {
        Ok(pair) => pair,
        Err(e) => {
            return DetectResult {
                projects: Vec::new(),
                errors: vec![DetectError::Fs(e)],
            }
        }
    };

    // Surface scan-level errors (harvest/yield) as DetectError::Fs.
    let mut errors: Vec<DetectError> = scan
        .take_errors()
        .into_iter()
        .map(DetectError::Fs)
        .collect();

    let mut projects = Vec::new();

    for detector in detectors::detectors() {
        match detector.detect(&fs, &scan, root) {
            Ok(Some(p)) => projects.push(p),
            Ok(None) => {}
            Err(e) => errors.push(e),
        }
    }

    // Shallow subdirectory scan: if root found no projects, try one level deeper.
    // This handles monorepos where each member has its own manifest but the root
    // has none. Errors at root are harvested and returned; they don't block the scan.
    if projects.is_empty() {
        // Collect subdirectory paths first to avoid borrowing scan while
        // mutating projects/errors.
        let subdirs: Vec<std::path::PathBuf> = scan
            .entries()
            .iter()
            .filter(|e| e.kind == FileKind::Directory && !SKIP_DIRS.contains(&e.name.as_str()))
            .map(|e| e.path.clone())
            .collect();

        for subdir_path in subdirs {
            // ProjectRoot::new canonicalizes and validates — skip non-UTF-8
            // or otherwise invalid paths silently (no manifest possible).
            let subdir_root = match ProjectRoot::new(&subdir_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let (sub_fs, mut sub_scan) = match scan_root(subdir_root.as_path()) {
                Ok(pair) => pair,
                Err(e) => {
                    errors.push(DetectError::Fs(e));
                    continue;
                }
            };

            // Harvest scan-level errors from subdirectory.
            for e in sub_scan.take_errors() {
                errors.push(DetectError::Fs(e));
            }

            for detector in detectors::detectors() {
                match detector.detect(&sub_fs, &sub_scan, &subdir_root) {
                    Ok(Some(p)) => projects.push(p),
                    Ok(None) => {}
                    Err(e) => errors.push(e),
                }
            }
        }
    }

    DetectResult { projects, errors }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that subdirectory scanning happens even when root has errors.
    ///
    /// The bug: if root has a corrupt pyproject.toml (error), subdirectory
    /// scan is skipped and valid Cargo.toml projects in subdirs are missed.
    ///
    /// The fix: subdirectory scan should only check `projects.is_empty()`,
    /// not also require `errors.is_empty()`. Errors are harvested (collected)
    /// but don't block the scan.
    #[test]
    fn subdirectory_scan_runs_despite_root_errors() {
        let root_dir = tempfile::tempdir().unwrap();

        // Create an invalid pyproject.toml at root — missing required [project] name field.
        // This will produce a DetectError, not just silently return None.
        std::fs::write(
            root_dir.path().join("pyproject.toml"),
            "[project]\n# missing 'name' field\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        // Create a subdirectory with a valid Cargo.toml.
        let subdir = root_dir.path().join("my-workspace");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(
            subdir.join("Cargo.toml"),
            "[package]\nname = \"valid-crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Run detect_all on root.
        let root = ProjectRoot::new(root_dir.path()).unwrap();
        let result = detect_all(&root);

        // Assert: the Cargo.toml project IS detected (subdirectory scan ran).
        assert!(
            !result.projects().is_empty(),
            "Cargo.toml in subdirectory should be detected despite root error"
        );
        assert_eq!(result.projects()[0].name(), "valid-crate");
        assert_eq!(result.projects()[0].stack(), Stack::Rust);

        // Assert: the pyproject.toml error IS reported (errors are harvested).
        assert!(
            !result.errors().is_empty(),
            "Root pyproject.toml error should be harvested"
        );
    }

    /// Test that subdirectory scan is skipped when root has projects.
    ///
    /// If root finds a project, we don't scan subdirectories (optimization:
    /// user probably meant the root project, not its subdirectories).
    #[test]
    fn subdirectory_scan_skipped_when_root_has_projects() {
        let root_dir = tempfile::tempdir().unwrap();

        // Create a valid Cargo.toml at root.
        std::fs::write(
            root_dir.path().join("Cargo.toml"),
            "[package]\nname = \"root-crate\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        // Create a subdirectory with another Cargo.toml.
        let subdir = root_dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(
            subdir.join("Cargo.toml"),
            "[package]\nname = \"sub-crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Run detect_all on root.
        let root = ProjectRoot::new(root_dir.path()).unwrap();
        let result = detect_all(&root);

        // Assert: only the root project is detected (subdirectory scan skipped).
        assert_eq!(result.projects().len(), 1);
        assert_eq!(result.projects()[0].name(), "root-crate");
    }
}
