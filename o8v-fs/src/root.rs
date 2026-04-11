// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the MIT License. See LICENSE file in this crate's directory.

//! `ContainmentRoot` — a validated, canonical directory path used as a containment boundary.

use crate::error::{classify_io_error, FsError};
use std::path::{Path, PathBuf};

/// A canonical, absolute, directory-verified path used as a containment boundary.
///
/// Guaranteed at construction time to be:
/// - Canonical (no symlinks, `..`, or `.` components — `fs::canonicalize` resolves all)
/// - Absolute
/// - A directory (not a file, symlink, or other)
///
/// Pass `&ContainmentRoot` instead of `&Path` to all `safe_*` functions.
///
/// ## Limitations
///
/// **TOCTOU**: The canonicalization and directory check happen at construction time.
/// Between `ContainmentRoot::new()` and any subsequent `safe_*` call, the filesystem
/// can change. This is an inherent limitation of filesystem APIs — not fixable without
/// kernel-level primitives (openat2 + O_PATH). Use `ContainmentRoot` to prevent
/// _accidental_ escapes; it is not a defense against a local attacker with write
/// access to the directory.
///
/// **`Display`**: Shows the fully resolved canonical path (symlinks resolved, `..`
/// collapsed). On macOS, `/tmp` displays as `/private/tmp`. This is intentional —
/// the canonical path is what the guards actually enforce.
///
/// **`resolve()`**: Designed for a narrow set of inputs: `"."`, `"./subpath"`,
/// and absolute paths. Not a general-purpose path resolver. Bare filenames (`"foo"`)
/// and hidden-file names (`.hidden`) are not paths starting with `./` and are handled
/// by the final fallback (`PathBuf::from(relative)`), which is CWD-relative, not
/// root-relative. Use `root.as_path().join(name)` for simple file joins.
///
/// **Thread safety**: No cross-call atomicity. Multiple `contains()` calls or a
/// `contains()` + `safe_read()` sequence are not atomic with respect to the filesystem.
#[derive(Debug, Clone)]
pub struct ContainmentRoot {
    path: PathBuf,
}

impl ContainmentRoot {
    /// Construct a `ContainmentRoot` from any path.
    ///
    /// Canonicalizes the path and verifies it is a directory.
    /// Returns `Err` if the path does not exist, is not a directory, or cannot be canonicalized.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, FsError> {
        let path = path.as_ref();
        let canonical = std::fs::canonicalize(path).map_err(|e| classify_io_error(path, e))?;
        if !canonical.is_dir() {
            let meta = std::fs::symlink_metadata(&canonical)
                .map_err(|e| classify_io_error(&canonical, e))?;
            return Err(FsError::NotRegularFile {
                path: canonical,
                kind: crate::error::meta_to_kind(&meta),
            });
        }
        Ok(Self { path: canonical })
    }

    /// Return the canonical path as a `&Path`.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Resolve a `./`-relative path within this containment root.
    ///
    /// ## Supported inputs
    ///
    /// | Input        | Result                         |
    /// |--------------|--------------------------------|
    /// | `"."`        | root itself                    |
    /// | `"./foo/bar"`| root joined with `foo/bar`     |
    /// | `"/abs/path"`| returned as-is (absolute path) |
    ///
    /// ## Limitations
    ///
    /// This is a narrow helper, NOT a general-purpose path resolver:
    ///
    /// - Bare names (`"Cargo.toml"`) are returned as `PathBuf::from("Cargo.toml")` — a
    ///   CWD-relative path, NOT `root/Cargo.toml`. Use `root.as_path().join(name)` instead.
    /// - Hidden files (`.hidden`) follow the same fallback and are also CWD-relative.
    /// - No containment check is performed on the returned path. Absolute inputs can point
    ///   anywhere. Always validate the result with `safe_*` functions before use.
    pub fn resolve(&self, relative: &str) -> PathBuf {
        if relative == "." {
            return self.path.clone();
        }
        if let Some(rest) = relative.strip_prefix("./") {
            return self.path.join(rest);
        }
        // Absolute paths and bare names returned as-is.
        PathBuf::from(relative)
    }

    /// Check if a path is within this containment boundary.
    ///
    /// Uses `safe_exists` internally — canonicalizes the parent, verifies
    /// containment, and rejects symlinks.
    ///
    /// Returns:
    /// - `Ok(true)` — path exists and is within boundary
    /// - `Ok(false)` — path doesn't exist, but parent is within boundary
    /// - `Err(SymlinkEscape)` — path escapes the containment root
    /// - `Err(IsSymlink)` — path is a symlink
    /// - `Err(...)` — other filesystem error
    pub fn contains(&self, path: &Path) -> Result<bool, FsError> {
        // Path equal to root is trivially contained.
        if path == self.path {
            return Ok(true);
        }
        crate::safe_exists(path, self)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_root() -> (TempDir, ContainmentRoot) {
        let dir = TempDir::new().unwrap();
        let root = ContainmentRoot::new(dir.path()).unwrap();
        (dir, root)
    }

    #[test]
    fn resolve_dot_returns_root() {
        let (_dir, root) = make_root();
        assert_eq!(root.resolve("."), root.as_path());
    }

    #[test]
    fn resolve_relative_joins_with_root() {
        let (_dir, root) = make_root();
        let resolved = root.resolve("./foo/bar");
        assert_eq!(resolved, root.as_path().join("foo/bar"));
    }

    #[test]
    fn resolve_absolute_returns_as_is() {
        let (_dir, root) = make_root();
        let resolved = root.resolve("/some/absolute/path");
        assert_eq!(resolved, PathBuf::from("/some/absolute/path"));
    }

    #[test]
    fn contains_root_itself() {
        let (_dir, root) = make_root();
        assert!(root.contains(root.as_path()).unwrap());
    }

    #[test]
    fn contains_file_inside_root() {
        let (dir, root) = make_root();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello").unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        assert!(root.contains(&canonical).unwrap());
    }

    #[test]
    fn contains_rejects_outside_root() {
        let (_dir, root) = make_root();
        let outside = PathBuf::from("/tmp");
        let result = root.contains(&outside);
        // Either an error (escape/symlink) or Ok(false) — never Ok(true)
        assert!(result.is_err() || matches!(result, Ok(false)));
    }
}

impl AsRef<Path> for ContainmentRoot {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Display for ContainmentRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.display().fmt(f)
    }
}
