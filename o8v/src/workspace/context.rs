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
pub fn resolve_workspace(
    path: impl AsRef<Path>,
) -> Result<(o8v_project::ProjectRoot, StorageDir, Option<ConfigDir>), ContextError> {
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
    let project_root = o8v_project::ProjectRoot::new(path_str.as_ref())
        .map_err(|e| ContextError::NoProjectRoot(e.to_string()))?;

    // 3. Open StorageDir at ~/.8v/.
    let storage = StorageDir::open().map_err(ContextError::Storage)?;

    // 4. Open ConfigDir — Ok(None) if .8v/ doesn't exist.
    let config = ConfigDir::open(&project_root).map_err(ContextError::Storage)?;

    Ok((project_root, storage, config))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn resolve_workspace_from_valid_project() {
        let _guard = crate::workspace::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a Cargo.toml so the directory looks like a project root.
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // Override HOME so StorageDir opens inside temp.
        std::env::set_var("HOME", tmp.path());

        let result = resolve_workspace(tmp.path());
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
    #[allow(clippy::disallowed_methods)]
    fn resolve_workspace_has_storage() {
        let _guard = crate::workspace::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", tmp.path());

        let (_project_root, storage, _config) = resolve_workspace(tmp.path()).unwrap();
        // Verify storage directory was created.
        let storage_path = tmp.path().join(".8v");
        assert!(
            storage_path.is_dir(),
            "~/.8v/ was not created by StorageDir::open"
        );
        // Verify last-check path accessible via storage.
        assert!(storage.last_check().ends_with("last-check.json"));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn resolve_workspace_config_is_none_without_dot8v() {
        let _guard = crate::workspace::HOME_MUTEX.lock().unwrap();
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        std::fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // Use a separate HOME so StorageDir::open() doesn't create .8v/ inside
        // the project directory.
        std::env::set_var("HOME", home.path());

        let (_project_root, _storage, config) = resolve_workspace(project.path()).unwrap();
        assert!(
            config.is_none(),
            "expected config to be None when no .8v/ directory exists in project"
        );
    }
}
