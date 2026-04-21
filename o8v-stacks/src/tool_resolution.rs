//! Tool resolution utilities — finding tools in project-specific locations.

use o8v_fs::{FileSystem, FsConfig, SafeFs};

/// Scan a project directory using the standard scan policy.
///
/// Single source of truth for how `o8v-core` scans projects for files.
/// All stacks that need to discover files use this — never `SafeFs::new()`
/// directly — so any policy change (exclusions, symlink handling) applies everywhere.
pub(crate) fn scan_project(project_dir: &o8v_fs::ContainmentRoot) -> Option<o8v_fs::DirScan> {
    match SafeFs::new(project_dir.as_path(), FsConfig::default()) {
        Ok(fs) => match fs.scan() {
            Ok(scan) => Some(scan),
            Err(e) => {
                tracing::warn!(error = %e, "failed to scan project directory");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to create SafeFs for project scan");
            None
        }
    }
}

/// Look for `program` in `start/node_modules/.bin`.
///
/// Only checks the project root itself — never walks into ancestor directories.
/// A binary placed in a parent's `node_modules/.bin/` must not be executed;
/// it belongs to a different project and could be an untrusted executable.
pub(crate) fn find_node_bin(
    start: &o8v_fs::ContainmentRoot,
    program: &str,
) -> Option<std::path::PathBuf> {
    let candidate = start.as_path().join("node_modules/.bin").join(program);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::project::ProjectRoot;
    use std::fs;

    fn make_root(dir: &std::path::Path) -> o8v_fs::ContainmentRoot {
        ProjectRoot::new(dir)
            .unwrap()
            .as_containment_root()
            .unwrap()
    }

    /// Binary present in project's own node_modules/.bin — must be found.
    #[test]
    fn finds_bin_in_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("node_modules/.bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("vitest"), b"#!/bin/sh").unwrap();

        let root = make_root(tmp.path());
        let result = find_node_bin(&root, "vitest");
        assert!(
            result.is_some(),
            "should find binary in project node_modules/.bin"
        );
        // Compare canonicalized paths to handle macOS /var → /private/var symlink.
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            bin_dir.join("vitest").canonicalize().unwrap()
        );
    }

    /// Binary absent from project root — must return None without walking up.
    #[test]
    fn returns_none_when_not_in_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        assert!(find_node_bin(&root, "vitest").is_none());
    }

    /// Ancestor has the binary but project root does not — must NOT find it.
    ///
    /// This is the regression test for the security fix: the old walk-up
    /// implementation would have returned the ancestor binary here.
    #[test]
    fn does_not_walk_into_ancestor_node_modules() {
        let tmp = tempfile::tempdir().unwrap();

        // Place a binary in the *ancestor* (tmp root) node_modules/.bin.
        let ancestor_bin_dir = tmp.path().join("node_modules/.bin");
        fs::create_dir_all(&ancestor_bin_dir).unwrap();
        fs::write(ancestor_bin_dir.join("vitest"), b"#!/bin/sh").unwrap();

        // The project lives one level deeper — it has no node_modules of its own.
        let project_dir = tmp.path().join("my-project");
        fs::create_dir_all(&project_dir).unwrap();

        let root = make_root(&project_dir);
        let result = find_node_bin(&root, "vitest");
        assert!(
            result.is_none(),
            "must not find binary from ancestor node_modules/.bin (security boundary)"
        );
    }
}
