//! `ProjectRoot` — validated project root directory.

use super::error::PathError;
use o8v_fs::ContainmentRoot;
use std::path::{Path, PathBuf};

/// A validated, canonical, absolute path to a project root directory.
///
/// Guarantees (both at construction and deserialization):
/// - The path existed on disk and was a directory
/// - It is absolute and canonicalized (no `..`, no symlinks, no ambiguity)
/// - The directory has a valid UTF-8 name (not a filesystem root)
/// - Two `ProjectRoots` pointing to the same directory will be equal
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    /// Create a `ProjectRoot` from any path.
    ///
    /// Canonicalizes the path (resolves `..`, symlinks, makes absolute).
    /// Validates that the result is a directory.
    ///
    /// # Errors
    ///
    /// Returns `PathError` if the path does not exist, is not a directory,
    /// cannot be canonicalized, or has no valid UTF-8 directory name.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, PathError> {
        let path = path.as_ref();

        // canonicalize resolves symlinks, .., and confirms existence in one call
        let canonical = std::fs::canonicalize(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PathError::NotFound {
                    path: path.to_path_buf(),
                }
            } else {
                PathError::CannotResolve {
                    path: path.to_path_buf(),
                    cause: e,
                }
            }
        })?;

        if !canonical.is_dir() {
            return Err(PathError::NotDirectory { path: canonical });
        }

        // Guarantee: directory has a valid UTF-8 name.
        // This rejects filesystem roots (/) and non-UTF-8 directory names.
        // Downstream code (dir_name fallback in detectors) relies on this.
        if canonical.file_name().and_then(|n| n.to_str()).is_none() {
            return Err(PathError::InvalidDirectoryName { path: canonical });
        }

        Ok(Self(canonical))
    }

    /// Create a [`ContainmentRoot`] anchored at this project root.
    ///
    /// The only way external callers get a raw `&Path` — through the security
    /// primitive. All `o8v-fs` operations require a `ContainmentRoot`.
    pub fn as_containment_root(&self) -> Result<ContainmentRoot, o8v_fs::FsError> {
        ContainmentRoot::new(&self.0)
    }

    /// The underlying path.
    ///
    /// External callers that only need a `&Path` (e.g. `detect_all` in o8v-stacks)
    /// may use this. Prefer [`as_containment_root`] for guarded I/O operations.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The directory name, guaranteed to be valid UTF-8.
    ///
    /// # Safety invariant
    ///
    /// `ProjectRoot::new` rejects any path where `file_name()` is `None` or
    /// non-UTF-8 (see the `InvalidDirectoryName` check in the constructor).
    /// Because `ProjectRoot` can only be constructed through `new` or
    /// deserialization (which re-validates), the `expect` below cannot fire
    /// for a properly constructed instance. If it does, the constructor has a
    /// bug — the panic message makes the invariant violation visible.
    ///
    /// # Panics
    ///
    /// Panics if the `ProjectRoot` invariant is violated (path has no valid
    /// UTF-8 directory name). This indicates a bug in the constructor.
    #[must_use]
    pub fn dir_name(&self) -> &str {
        self.0
            .file_name()
            .and_then(|n| n.to_str())
            .expect("BUG: ProjectRoot invariant violated — constructor must reject paths without a valid UTF-8 directory name")
    }
}

impl std::fmt::Display for ProjectRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl<'de> serde::Deserialize<'de> for ProjectRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        Self::new(path).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_directory() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = ProjectRoot::new(dir.path()).unwrap();
        assert!(project_path.as_path().is_absolute());
    }

    #[test]
    fn rejects_nonexistent() {
        let result = ProjectRoot::new("/this/does/not/exist");
        assert!(matches!(result, Err(PathError::NotFound { .. })));
    }

    #[test]
    fn rejects_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hello").unwrap();
        let result = ProjectRoot::new(&file);
        assert!(matches!(result, Err(PathError::NotDirectory { .. })));
    }

    #[test]
    fn resolves_dot_dot() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("sub");
        std::fs::create_dir(&child).unwrap();

        let via_dotdot = ProjectRoot::new(child.join("..")).unwrap();
        let direct = ProjectRoot::new(dir.path()).unwrap();
        assert_eq!(via_dotdot, direct, "path/sub/.. should equal path");
    }

    #[test]
    fn relative_path_resolved() {
        let path = ProjectRoot::new(".").unwrap();
        assert!(path.as_path().is_absolute());
        assert!(!path.as_path().to_str().unwrap().contains(".."));
    }

    #[test]
    fn same_dir_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let a = ProjectRoot::new(dir.path()).unwrap();
        let b = ProjectRoot::new(dir.path()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn display_impl() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let display = format!("{path}");
        assert!(
            display.starts_with('/'),
            "display should show absolute path"
        );
        assert!(!display.is_empty());
    }

    #[test]
    fn as_containment_root_returns_same_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let containment = path.as_containment_root().unwrap();
        assert_eq!(containment.as_path(), path.as_path());
    }

    #[test]
    fn deserialize_valid() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(dir.path()).unwrap();
        let json = format!("\"{}\"", canonical.display());
        let path: ProjectRoot = serde_json::from_str(&json).unwrap();
        assert_eq!(path.as_path(), canonical.as_path());
    }

    #[test]
    fn deserialize_nonexistent() {
        let json = "\"/this/does/not/exist/at/all\"";
        let result: Result<ProjectRoot, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_root_path() {
        let result = ProjectRoot::new("/");
        assert!(matches!(
            result,
            Err(PathError::InvalidDirectoryName { .. })
        ));
    }

    #[test]
    fn dir_name_returns_valid_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let name = path.dir_name();
        assert!(!name.is_empty());
        assert_eq!(name, dir.path().file_name().unwrap().to_str().unwrap());
    }

    #[test]
    fn cannot_resolve_permission_denied() {
        // On unix, a path through a directory we can't access triggers CannotResolve
        // This test is best-effort — if it can't set up the scenario, it passes
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().unwrap();
            let blocked = dir.path().join("blocked");
            std::fs::create_dir(&blocked).unwrap();
            let target = blocked.join("inner");
            std::fs::create_dir(&target).unwrap();

            // Remove read+exec permissions on parent
            std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000)).unwrap();

            let result = ProjectRoot::new(&target);
            // Restore permissions before asserting (so cleanup works)
            std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755)).unwrap();

            assert!(matches!(result, Err(PathError::CannotResolve { .. })));
        }
    }
}
