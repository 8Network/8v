// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `CommandContext` — the full operational context for a command that operates on a project.

use std::path::Path;

use crate::{ConfigDir, StorageDir};

// ─── ContextError ────────────────────────────────────────────────────────────

/// Errors that can occur when building a [`CommandContext`].
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

// ─── CommandContext ───────────────────────────────────────────────────────────

/// The full operational context for a command that operates on a project.
///
/// Built by the command handler from the path argument. The entrypoint
/// (CLI or MCP server) provides only the raw path string — it does not detect
/// project root, does not open storage, does not check config.
pub struct CommandContext {
    pub project_root: o8v_project::ProjectRoot,
    pub storage: StorageDir,
    pub config: Option<ConfigDir>,
}

impl CommandContext {
    /// Build a `CommandContext` from a raw path argument.
    ///
    /// 1. Resolves `path` to an absolute path.
    /// 2. Detects `ProjectRoot` by constructing it from the resolved path.
    /// 3. Opens `StorageDir` at `~/.8v/`.
    /// 4. Opens `ConfigDir` at `project_root/.8v/` if present; `Ok(None)` is fine.
    ///
    /// Returns an error if the path cannot be resolved, if no project root is
    /// found, or if storage cannot be opened. Every error is visible — no
    /// silent fallbacks.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ContextError> {
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

        Ok(Self {
            project_root,
            storage,
            config,
        })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn context_from_valid_project() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a Cargo.toml so the directory looks like a project root.
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // Override HOME so StorageDir opens inside temp.
        std::env::set_var("HOME", tmp.path());

        let result = CommandContext::from_path(tmp.path());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn context_from_nonexistent_path() {
        let result = CommandContext::from_path("/nonexistent/path/that/does/not/exist");
        assert!(result.is_err(), "expected Err for nonexistent path, got Ok");
        match result {
            Err(ContextError::PathResolution(_)) => {}
            Err(e) => panic!("expected PathResolution error, got: {e}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn context_has_storage() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", tmp.path());

        let ctx = CommandContext::from_path(tmp.path()).unwrap();
        // Verify storage directory was created.
        let storage_path = tmp.path().join(".8v");
        assert!(
            storage_path.is_dir(),
            "~/.8v/ was not created by StorageDir::open"
        );
        // Verify events dir accessible via storage.
        assert!(ctx.storage.events_dir().ends_with("events"));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn context_config_is_none_without_dot8v() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
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

        let ctx = CommandContext::from_path(project.path()).unwrap();
        assert!(
            ctx.config.is_none(),
            "expected config to be None when no .8v/ directory exists in project"
        );
    }
}
