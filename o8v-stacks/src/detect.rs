// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Project detection entry point.
//!
//! [`detect_all`] is the single function callers use to detect projects in a
//! validated directory. It creates a `SafeFs`, scans the directory once, runs
//! all detectors, and performs a one-level shallow subdirectory scan if no
//! projects were found at the root.

use o8v_core::project::{DetectError, ProjectRoot};
use o8v_fs::{FileKind, FileSystem, FsConfig, SafeFs};

use crate::detectors::{self, DetectResult};

/// Scan `path` with the standard project scan policy.
///
/// Single source of truth for how project scanning works.
/// Returns an error if the directory cannot be opened or indexed.
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
                Err(e) => {
                    errors.push(DetectError::SubdirRootInvalid {
                        path: subdir_path,
                        cause: e,
                    });
                    continue;
                }
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

    /// Regression test for F7/F16: subdir detection failures must surface in errors.
    ///
    /// Strategy: create a tempdir with a restricted subdir (mode 0o000). The parent
    /// directory scan lists it as FileKind::Directory. When the subdir loop runs, either
    /// ProjectRoot::new fails (SubdirRootInvalid) or scan_root fails (Fs/DirectoryUnreadable)
    /// depending on OS canonicalize behavior. Either way, the error must be recorded —
    /// the pre-fix bug was Err(_) => continue which silently discarded it entirely.
    #[cfg(unix)]
    #[test]
    fn subdir_root_invalid_surfaces_in_errors() {
        use std::os::unix::fs::PermissionsExt;

        let root_dir = tempfile::tempdir().unwrap();
        let subdir = root_dir.path().join("restricted-subdir");
        std::fs::create_dir(&subdir).unwrap();

        // Remove all permissions so either canonicalize or directory scan fails.
        std::fs::set_permissions(&subdir, std::fs::Permissions::from_mode(0o000)).unwrap();

        let root = ProjectRoot::new(root_dir.path()).unwrap();
        let result = detect_all(&root);

        // Restore permissions so tempdir cleanup succeeds.
        let _ = std::fs::set_permissions(&subdir, std::fs::Permissions::from_mode(0o755));

        // Pre-fix: errors is empty (bug). Post-fix: at least one error is recorded.
        // The kind may be subdir_root_invalid (Linux) or directory_unreadable (macOS),
        // because macOS canonicalize succeeds for 0o000 dirs but entering them fails.
        assert!(
            !result.errors().is_empty(),
            "subdir failure for restricted-subdir must be recorded in errors, not silently dropped; got errors: {:?}",
            result.errors()
        );
    }
}