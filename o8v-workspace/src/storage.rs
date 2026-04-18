// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `StorageDir` — home-level 8v storage directory at `~/.8v/`.

use o8v_fs::ContainmentRoot;
use std::path::PathBuf;

use crate::DIR_NAME;

// ─── StorageDir ──────────────────────────────────────────────────────────────

/// The home-level 8v storage directory.
///
/// Always at `~/.8v/`. Contains all user-level state: lifecycle events, last
/// check results, workspace registry, and config. Never project-relative.
///
/// This is the single source of truth for the `~/.8v/` path. No other code
/// constructs this path independently.
#[derive(Clone)]
pub struct StorageDir {
    containment: ContainmentRoot,
}

impl StorageDir {
    const EVENTS: &'static str = "events.ndjson";
    const LAST_CHECK: &'static str = "last-check.json";
    const WORKSPACES_TOML: &'static str = "workspaces.toml";
    const CONFIG_TOML: &'static str = "config.toml";

    /// Resolve `~/.8v/` from the HOME environment variable.
    ///
    /// Returns an error if HOME is not set. Never silently falls back to `/tmp`.
    ///
    /// `_8V_HOME` overrides HOME when set — this is the test-isolation fence.
    /// `cargo test` sets `_8V_HOME=target/test-home` via `.cargo/config.toml`
    /// so integration tests that fork the `8v` binary never write to the real
    /// `~/.8v/events.ndjson`. Production shells have no `_8V_HOME` set, so the
    /// HOME path is used as normal.
    fn resolve_home() -> Result<PathBuf, std::io::Error> {
        if let Ok(v) = std::env::var("_8V_HOME") {
            return Ok(PathBuf::from(v).join(DIR_NAME));
        }
        match std::env::var("HOME") {
            Ok(h) => Ok(PathBuf::from(h).join(DIR_NAME)),
            Err(_) => Err(std::io::Error::other(
                "HOME environment variable is not set — cannot determine ~/.8v/ location",
            )),
        }
    }

    /// Open (or create) the storage directory at `~/.8v/`.
    ///
    /// Production entry point. Resolves HOME, then delegates to `at()`.
    /// Returns an error if HOME is not set or the directory cannot be created.
    pub fn open() -> Result<Self, std::io::Error> {
        Self::at(Self::resolve_home()?)
    }

    /// Open (or create) a storage directory at the given path.
    ///
    /// This is the path-based constructor. Tests and benchmarks pass a temp dir.
    /// Production calls `open()` which resolves `~/.8v/` and delegates here.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn at(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref();

        // Bootstrap: create with raw fs — this IS the root we're establishing.
        std::fs::create_dir_all(path)?;
        let canonical = std::fs::canonicalize(path)?;
        let containment =
            ContainmentRoot::new(&canonical).map_err(|e| std::io::Error::other(e.to_string()))?;

        Ok(Self { containment })
    }

    /// The raw `~/.8v/` path, resolved from HOME.
    ///
    /// Used by `WorkspaceDir::home()` and `register_workspace` — the only
    /// callers that need the path before `StorageDir` is fully opened.
    pub fn home_path() -> Result<PathBuf, std::io::Error> {
        Self::resolve_home()
    }

    /// The containment root for all fs operations inside `~/.8v/`.
    pub fn containment(&self) -> &ContainmentRoot {
        &self.containment
    }

    // ─── Named path methods ──────────────────────────────────────────────────

    /// `~/.8v/events.ndjson` — unified lifecycle events for all callers.
    pub fn events(&self) -> PathBuf {
        self.containment.as_path().join(Self::EVENTS)
    }

    /// `~/.8v/workspaces.toml`
    pub fn workspaces_toml(&self) -> PathBuf {
        self.containment.as_path().join(Self::WORKSPACES_TOML)
    }

    /// `~/.8v/config.toml`
    pub fn config_toml(&self) -> PathBuf {
        self.containment.as_path().join(Self::CONFIG_TOML)
    }

    /// `~/.8v/last-check.json`
    pub fn last_check(&self) -> PathBuf {
        self.containment.as_path().join(Self::LAST_CHECK)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_creates_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage_path = tmp.path().join("storage");

        let result = StorageDir::at(&storage_path);
        assert!(result.is_ok(), "StorageDir::at failed: {:?}", result.err());
        assert!(storage_path.is_dir(), "storage dir was not created");
    }

    #[test]
    fn at_path_methods_return_expected_paths() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = StorageDir::at(tmp.path()).unwrap();
        let base = std::fs::canonicalize(tmp.path()).unwrap();

        assert_eq!(storage.events(), base.join("events.ndjson"));
        assert_eq!(storage.workspaces_toml(), base.join("workspaces.toml"));
        assert_eq!(storage.config_toml(), base.join("config.toml"));
        assert_eq!(storage.last_check(), base.join("last-check.json"));
    }

    #[test]
    fn at_nonexistent_parent_fails() {
        let result = StorageDir::at("/nonexistent/path/that/does/not/exist");
        assert!(result.is_err());
    }
}
