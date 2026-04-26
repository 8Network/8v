// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Workspace resolution — detect project root, open storage and config from a path.

use std::path::Path;

use super::{ConfigDir, StorageDir};

// ─── ContextError ────────────────────────────────────────────────────────────

/// Errors that can occur when resolving a workspace.
#[derive(Debug)]
pub enum ContextError {
    /// The given path could not be resolved to an absolute path.
    PathResolution(std::io::Error),
    /// No project root was detected at the given path.
    NoProjectRoot(String),
    /// The home-level storage directory (`~/.8v/`) could not be opened.
    Storage(std::io::Error),
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PathResolution(e) => write!(f, "path resolution failed: {e}"),
            Self::NoProjectRoot(msg) => write!(f, "no project root detected: {msg}"),
            Self::Storage(e) => write!(f, "storage directory unavailable: {e}"),
        }
    }
}

impl std::error::Error for ContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PathResolution(e) | Self::Storage(e) => Some(e),
            Self::NoProjectRoot(_) => None,
        }
    }
}

// ─── WorkspaceContext ────────────────────────────────────────────────────────

/// Named result of workspace resolution.
///
/// Returned by [`resolve_workspace`] so callers use field names instead of
/// positional tuple destructuring.
pub struct WorkspaceContext {
    pub root: o8v_core::project::ProjectRoot,
    pub storage: StorageDir,
    pub config: Option<ConfigDir>,
}

// ─── resolve_workspace ───────────────────────────────────────────────────────

/// Resolve a path to project root, storage, and config.
///
/// This is the single point where project detection and storage opening happens.
///
/// 1. Resolves `path` to an absolute path.
/// 2. Detects `ProjectRoot` by constructing it from the resolved path.
/// 3. Opens `StorageDir` at `~/.8v/`.
/// 4. Opens `ConfigDir` at `project_root/.8v/` if present; `Ok(None)` is fine.
///
/// Returns an error if the path cannot be resolved, if no project root is
/// found, or if storage cannot be opened. Every error is visible — no
/// silent fallbacks.
pub fn resolve_workspace(path: impl AsRef<Path>) -> Result<WorkspaceContext, ContextError> {
    // 1. Resolve to absolute path.
    let abs = std::fs::canonicalize(path.as_ref()).map_err(|e| {
        ContextError::PathResolution(std::io::Error::other(format!(
            "{}: {}",
            path.as_ref().display(),
            e
        )))
    })?;

    // 2. Detect ProjectRoot.
    let path_str = abs.to_string_lossy();
    let root = o8v_core::project::ProjectRoot::new(path_str.as_ref())
        .map_err(|e| ContextError::NoProjectRoot(e.to_string()))?;

    // 3. Open StorageDir at ~/.8v/.
    let storage = StorageDir::open().map_err(ContextError::Storage)?;

    // 4. Open ConfigDir — Ok(None) if .8v/ doesn't exist.
    let config = ConfigDir::open(&root).map_err(ContextError::Storage)?;

    Ok(WorkspaceContext {
        root,
        storage,
        config,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only helper: resolve workspace with an explicit home directory.
    ///
    /// Avoids mutating `HOME` in the environment — calls `StorageDir::at(home)`
    /// directly so tests are hermetic and env-mutation-free.
    fn resolve_workspace_with_home(
        path: impl AsRef<Path>,
        home: &Path,
    ) -> Result<WorkspaceContext, ContextError> {
        let abs = std::fs::canonicalize(path.as_ref()).map_err(|e| {
            ContextError::PathResolution(std::io::Error::other(format!(
                "{}: {}",
                path.as_ref().display(),
                e
            )))
        })?;
        let path_str = abs.to_string_lossy();
        let root = o8v_core::project::ProjectRoot::new(path_str.as_ref())
            .map_err(|e| ContextError::NoProjectRoot(e.to_string()))?;
        let storage = StorageDir::at(home).map_err(ContextError::Storage)?;
        let config = ConfigDir::open(&root).map_err(ContextError::Storage)?;
        Ok(WorkspaceContext {
            root,
            storage,
            config,
        })
    }

    #[test]
    fn resolve_workspace_from_valid_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a Cargo.toml so the directory looks like a project root.
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let result = resolve_workspace_with_home(tmp.path(), tmp.path());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn resolve_workspace_from_nonexistent_path() {
        let result = resolve_workspace("/nonexistent/path/that/does/not/exist");
        assert!(result.is_err(), "expected Err for nonexistent path, got Ok");
        match result {
            Err(ContextError::PathResolution(_)) => {}
            Err(e) => panic!("expected PathResolution error, got: {e}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    #[test]
    fn resolve_workspace_has_storage() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let ws = resolve_workspace_with_home(tmp.path(), tmp.path()).unwrap();
        // Verify the storage root itself was created/accessible.
        assert!(
            tmp.path().is_dir(),
            "storage root was not created by StorageDir::at"
        );
        // Verify last-check path accessible via storage.
        assert!(ws.storage.last_check().ends_with("last-check.json"));
    }

    #[test]
    fn resolve_workspace_config_is_none_without_dot8v() {
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        std::fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let ws = resolve_workspace_with_home(project.path(), home.path()).unwrap();
        assert!(
            ws.config.is_none(),
            "expected config to be None when no .8v/ directory exists in project"
        );
    }
}
