// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Workspace management — .8v/ layout, registry, config discovery.
//!
//! Single source of truth for all .8v/ paths and formats.
//! No other crate hardcodes .8v/ paths.
//!
//! ## Path constants
//!
//! All `.8v/` path knowledge lives here:
//! - [`DIR_NAME`] — the `.8v` directory name
//! - [`REGISTRY_FILE`] — `workspaces.toml`
//! - [`local_8v_dir`] — construct `.8v/` path relative to any project root

use o8v_fs::{ContainmentRoot, FsConfig};
use o8v_project::ProjectRoot;
use std::path::PathBuf;

pub mod config;
pub use config::ConfigDir;

pub mod context;
pub use context::{CommandContext, ContextError};

pub mod storage;
pub use storage::StorageDir;

// ─── Path constants ──────────────────────────────────────────────────────────

/// The `.8v/` state directory name.
///
/// Every path under `.8v/` in the project tree is constructed from this constant.
/// No other module hardcodes the string `".8v"`.
pub const DIR_NAME: &str = ".8v";

/// The workspace registry filename inside `~/.8v/`.
pub const REGISTRY_FILE: &str = "workspaces.toml";

/// Construct the `.8v/` path relative to a project root.
#[must_use]
pub fn local_8v_dir(root: &ProjectRoot) -> PathBuf {
    PathBuf::from(root.to_string()).join(DIR_NAME)
}

// ─── Config Location ────────────────────────────────────────────────────────

/// Where workspace configuration is stored.
///
/// Fields are module-private. Use the typed methods — callers never touch the raw path.
pub enum WorkspaceDir {
    /// `.8v/` directory inside the project root.
    ///
    /// `containment` is anchored at the project root (validated at construction).
    /// Stored here so `create()` never re-derives or re-canonicalizes it.
    Local {
        config_dir: PathBuf,
        containment: ContainmentRoot,
    },
    /// `~/.8v/` directory in the user's home.
    Home { config_dir: PathBuf },
}

impl WorkspaceDir {
    /// Local `.8v/` directory inside the given project root.
    ///
    /// Returns `Err` if the project root cannot be turned into a containment root
    /// (should not happen for a valid `ProjectRoot`, but propagated for safety).
    pub fn local(root: &ProjectRoot) -> Result<Self, o8v_fs::FsError> {
        let containment = root.as_containment_root()?;
        Ok(Self::Local {
            config_dir: local_8v_dir(root),
            containment,
        })
    }

    /// Home `~/.8v/` directory.
    ///
    /// Returns `Err` if HOME is not set.
    pub fn home() -> Result<Self, std::io::Error> {
        Ok(Self::Home {
            config_dir: StorageDir::home_path()?,
        })
    }

    /// Create the workspace directory on disk with the correct containment root.
    pub fn create(&self) -> std::io::Result<()> {
        match self {
            Self::Local {
                config_dir,
                containment,
            } => o8v_fs::safe_create_dir(config_dir, containment).map_err(to_io),
            Self::Home { config_dir } => {
                // Bootstrap: home may not exist yet. Use its parent as the
                // temporary containment boundary only for this one mkdir.
                let parent = config_dir.parent().unwrap_or(config_dir.as_path());
                let containment = ContainmentRoot::new(parent).map_err(to_io)?;
                o8v_fs::safe_create_dir(config_dir, &containment).map_err(to_io)
            }
        }
    }

    /// Display the config directory path for user messages.
    pub fn display(&self) -> std::path::Display<'_> {
        match self {
            Self::Local { config_dir, .. } | Self::Home { config_dir } => config_dir.display(),
        }
    }

    /// Whether this is the home directory variant.
    pub fn is_home(&self) -> bool {
        matches!(self, Self::Home { .. })
    }
}

// ─── Workspace Entry ────────────────────────────────────────────────────────

/// An entry in the home workspace registry (`~/.8v/workspaces.toml`).
pub struct WorkspaceEntry {
    /// Absolute path of the workspace, stored as a string in the registry.
    pub path: String,
    pub name: String,
}

impl WorkspaceEntry {
    pub fn from_project_path(root: &ProjectRoot) -> Self {
        let name = root.dir_name().to_string();
        let path = root.to_string();
        Self { path, name }
    }

    pub fn to_toml_entry(&self) -> Result<String, String> {
        let path_str = self.path.as_str();

        // Reject paths containing control characters (including newlines, tabs, etc.)
        // TOML basic strings require proper escaping of control chars, but no legitimate
        // filesystem path contains newlines or tabs. Defense in depth: reject entirely.
        for ch in path_str.chars() {
            if ch.is_control() {
                return Err(format!(
                    "Path contains control character (U+{:04X}): {:?}",
                    ch as u32, path_str
                ));
            }
        }

        // Also reject the name field
        for ch in self.name.chars() {
            if ch.is_control() {
                return Err(format!(
                    "Workspace name contains control character (U+{:04X}): {:?}",
                    ch as u32, self.name
                ));
            }
        }

        let escaped_path = path_str.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_name = self.name.replace('\\', "\\\\").replace('"', "\\\"");

        Ok(format!(
            "\n[[workspace]]\npath = \"{escaped_path}\"\nname = \"{escaped_name}\"\n"
        ))
    }
}

// ─── Error Conversion ───────────────────────────────────────────────────────

/// Convert o8v_fs::FsError to std::io::Error for uniform error handling.
pub fn to_io(e: o8v_fs::FsError) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

// ─── Home Registry ──────────────────────────────────────────────────────────

pub fn register_workspace(root: &ProjectRoot) -> std::io::Result<()> {
    let storage = StorageDir::open()?;
    let registry_path = storage.workspaces_toml();
    let containment_root = storage.containment();
    let entry = WorkspaceEntry::from_project_path(root);
    let abs_path = root.to_string();

    // Validate workspace entry before using it (rejects control characters)
    let toml_entry = entry.to_toml_entry().map_err(std::io::Error::other)?;

    match o8v_fs::safe_exists(&registry_path, containment_root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(&registry_path, containment_root, &FsConfig::default())
                .map_err(to_io)?;
            let content = guarded.content();
            if content.contains(&abs_path as &str) {
                return Ok(());
            }
            o8v_fs::safe_append(&registry_path, containment_root, toml_entry.as_bytes())
                .map_err(to_io)?;
        }
        Ok(false) => {
            let content = format!("# 8v workspace registry\n{}", toml_entry);
            o8v_fs::safe_write(&registry_path, containment_root, content.as_bytes())
                .map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }

    #[cfg(unix)]
    o8v_fs::safe_set_permissions(&registry_path, containment_root, 0o600).map_err(to_io)?;

    Ok(())
}

// ─── Test helpers ───────────────────────────────────────────────────────────

/// HOME is process-global state. All tests that call `std::env::set_var("HOME", …)`
/// must hold this lock for the duration of the test — including the `StorageDir::open()`
/// call — to prevent parallel tests from racing on the same env var.
///
/// Declared at crate root so both `storage::tests` and `context::tests` share
/// the exact same `Mutex` instance.
#[cfg(test)]
pub(crate) static HOME_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_entry_derives_name_from_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = ProjectRoot::new(dir.path().to_str().unwrap()).unwrap();
        let entry = WorkspaceEntry::from_project_path(&root);

        let expected_name = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(entry.name, expected_name);
    }

    #[test]
    fn workspace_entry_toml_escapes_backslashes() {
        let entry = WorkspaceEntry {
            path: r"C:\Users\test\project".to_string(),
            name: "project".to_string(),
        };
        let toml = entry.to_toml_entry().unwrap();
        assert!(toml.contains(r"C:\\Users\\test\\project"));
    }

    #[test]
    fn workspace_entry_toml_escapes_quotes() {
        let entry = WorkspaceEntry {
            path: r#"/home/user/my "project""#.to_string(),
            name: "project".to_string(),
        };
        let toml = entry.to_toml_entry().unwrap();
        assert!(toml.contains(r#"my \"project\""#));
    }

    #[test]
    fn workspace_entry_toml_format() {
        let entry = WorkspaceEntry {
            path: "/home/user/myproject".to_string(),
            name: "myproject".to_string(),
        };
        let toml = entry.to_toml_entry().unwrap();
        assert!(toml.contains("[[workspace]]"));
        assert!(toml.contains(r#"path = "/home/user/myproject""#));
        assert!(toml.contains(r#"name = "myproject""#));
    }

    #[test]
    fn workspace_entry_with_spaces_in_path() {
        let entry = WorkspaceEntry {
            path: "/home/user/my project".to_string(),
            name: "my project".to_string(),
        };
        let toml = entry.to_toml_entry().unwrap();
        assert!(toml.contains(r#"path = "/home/user/my project""#));
    }

    #[test]
    fn workspace_entry_with_unicode() {
        let entry = WorkspaceEntry {
            path: "/home/user/проект".to_string(),
            name: "проект".to_string(),
        };
        let toml = entry.to_toml_entry().unwrap();
        assert!(toml.contains("проект"));
    }

    #[test]
    fn workspace_entry_rejects_control_chars_in_path() {
        let entry = WorkspaceEntry {
            path: "/home/user/my\nproject".to_string(),
            name: "project".to_string(),
        };
        let result = entry.to_toml_entry();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("control character"));
    }

    #[test]
    fn workspace_entry_rejects_control_chars_in_name() {
        let entry = WorkspaceEntry {
            path: "/home/user/project".to_string(),
            name: "my\tproject".to_string(),
        };
        let result = entry.to_toml_entry();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("control character"));
    }

    #[test]
    fn workspace_entry_path_and_name_consistency() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = ProjectRoot::new(dir.path().to_str().unwrap()).unwrap();
        let entry = WorkspaceEntry::from_project_path(&root);

        assert_eq!(entry.path, root.to_string());
        assert!(!entry.name.is_empty());
    }
}
