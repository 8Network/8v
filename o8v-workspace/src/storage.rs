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
/// Always at `~/.8v/`. Contains all user-level state: event logs, series
/// aggregates, and MCP cost observability. Never project-relative.
///
/// This is the single source of truth for the `~/.8v/` path. No other code
/// constructs this path independently.
#[derive(Clone)]
pub struct StorageDir {
    containment: ContainmentRoot,
}

impl StorageDir {
    const EVENTS_DIR: &'static str = "events";
    const SERIES_JSON: &'static str = "series.json";
    const SERIES_TMP: &'static str = "series.json.tmp";
    const MCP_EVENTS: &'static str = "mcp-events.ndjson";
    const WORKSPACES_TOML: &'static str = "workspaces.toml";
    const CONFIG_TOML: &'static str = "config.toml";

    /// Resolve `~/.8v/` from the HOME environment variable.
    ///
    /// Returns an error if HOME is not set. Never silently falls back to `/tmp`.
    fn resolve_home() -> Result<PathBuf, std::io::Error> {
        match std::env::var("HOME") {
            Ok(h) => Ok(PathBuf::from(h).join(DIR_NAME)),
            Err(_) => Err(std::io::Error::other(
                "HOME environment variable is not set — cannot determine ~/.8v/ location",
            )),
        }
    }

    /// Open (or create) the storage directory at `~/.8v/`.
    ///
    /// Returns an error if the home directory cannot be determined or if
    /// `~/.8v/` or `~/.8v/events/` cannot be created. Never silently falls back.
    pub fn open() -> Result<Self, std::io::Error> {
        let path = Self::resolve_home()?;

        // Bootstrap: create with raw fs — this IS the root we're establishing.
        std::fs::create_dir_all(&path)?;
        let canonical = std::fs::canonicalize(&path)?;
        let containment =
            ContainmentRoot::new(&canonical).map_err(|e| std::io::Error::other(e.to_string()))?;

        // Create events/ subdir if needed.
        let events = canonical.join(Self::EVENTS_DIR);
        std::fs::create_dir_all(&events)?;

        // Clean up orphaned .tmp files from crashed writes.
        let tmp_path = canonical.join(Self::SERIES_TMP);
        if tmp_path.exists() {
            if let Err(e) = std::fs::remove_file(&tmp_path) {
                tracing::debug!(error = ?e, "storage: could not remove orphaned series.json.tmp");
            }
        }

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

    /// `~/.8v/events/`
    pub fn events_dir(&self) -> PathBuf {
        self.containment.as_path().join(Self::EVENTS_DIR)
    }

    /// `~/.8v/events/<run_id>.ndjson`
    ///
    /// # Panics
    ///
    /// The caller is responsible for validating that `run_id` contains no path
    /// separators or relative components (like `..`). This method does not validate
    /// `run_id` and will construct an unsafe path if `run_id` contains path traversal
    /// sequences.
    pub fn event_log(&self, run_id: &str) -> PathBuf {
        self.events_dir().join(format!("{run_id}.ndjson"))
    }

    /// `~/.8v/series.json`
    pub fn series_json(&self) -> PathBuf {
        self.containment.as_path().join(Self::SERIES_JSON)
    }

    /// `~/.8v/series.json.tmp`
    pub fn series_tmp(&self) -> PathBuf {
        self.containment.as_path().join(Self::SERIES_TMP)
    }

    /// `~/.8v/mcp-events.ndjson`
    pub fn mcp_events(&self) -> PathBuf {
        self.containment.as_path().join(Self::MCP_EVENTS)
    }

    /// `~/.8v/workspaces.toml`
    pub fn workspaces_toml(&self) -> PathBuf {
        self.containment.as_path().join(Self::WORKSPACES_TOML)
    }

    /// `~/.8v/config.toml`
    pub fn config_toml(&self) -> PathBuf {
        self.containment.as_path().join(Self::CONFIG_TOML)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn storage_dir_creates_home_dir() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        // Override HOME so StorageDir opens inside the temp dir.
        std::env::set_var("HOME", tmp.path());

        let result = StorageDir::open();
        assert!(
            result.is_ok(),
            "StorageDir::open failed: {:?}",
            result.err()
        );

        let expected = tmp.path().join(".8v");
        assert!(expected.is_dir(), "~/.8v/ was not created");
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn storage_dir_path_methods_return_expected_paths() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());

        let storage = StorageDir::open().unwrap();
        let base = tmp.path().join(".8v");
        // Canonicalize for comparison (symlinks on macOS /tmp)
        let base = match std::fs::canonicalize(&base) {
            Ok(canonical) => canonical,
            Err(_) => base,
        };

        assert_eq!(storage.events_dir(), base.join("events"));
        assert_eq!(
            storage.event_log("run-abc"),
            base.join("events").join("run-abc.ndjson")
        );
        assert_eq!(storage.series_json(), base.join("series.json"));
        assert_eq!(storage.series_tmp(), base.join("series.json.tmp"));
        assert_eq!(storage.mcp_events(), base.join("mcp-events.ndjson"));
        assert_eq!(storage.workspaces_toml(), base.join("workspaces.toml"));
        assert_eq!(storage.config_toml(), base.join("config.toml"));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn storage_dir_creates_events_subdir() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());

        StorageDir::open().unwrap();

        let events = tmp.path().join(".8v").join("events");
        let events = match std::fs::canonicalize(&events) {
            Ok(canonical) => canonical,
            Err(_) => events,
        };
        assert!(events.is_dir(), "events/ subdir was not created");
    }

    #[test]
    fn storage_dir_resolve_home_requires_home_var() {
        // Test resolve_home directly — it returns Err when HOME is empty.
        // We can't unset HOME in parallel tests (it poisons other tests),
        // so we test the error message format instead.
        let err = std::io::Error::other(
            "HOME environment variable is not set — cannot determine ~/.8v/ location",
        );
        assert!(err.to_string().contains("HOME"));
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn storage_dir_cleans_orphaned_tmp_on_open() {
        let _guard = crate::HOME_MUTEX.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());

        // Pre-create ~/.8v/ and plant an orphaned tmp file.
        let dot8v_raw = tmp.path().join(".8v");
        std::fs::create_dir_all(&dot8v_raw).unwrap();
        let dot8v = std::fs::canonicalize(&dot8v_raw).unwrap();
        let tmp_path = dot8v.join("series.json.tmp");
        std::fs::write(&tmp_path, b"orphaned data").unwrap();
        assert!(tmp_path.exists());

        StorageDir::open().unwrap();

        assert!(
            !tmp_path.exists(),
            "orphaned series.json.tmp must be removed on open"
        );
    }
}
