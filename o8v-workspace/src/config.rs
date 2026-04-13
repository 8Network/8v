// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `ConfigDir` — project-local `.8v/` configuration directory.

use o8v_fs::ContainmentRoot;
use o8v_core::project::ProjectRoot;
use std::path::PathBuf;

use crate::DIR_NAME;

// ─── ConfigDir ───────────────────────────────────────────────────────────────

/// The project-local `.8v/` configuration directory.
///
/// Optional. Not every project has `.8v/`. `ConfigDir::open` returns `Ok(None)`
/// if `.8v/` does not exist — this is not an error.
pub struct ConfigDir {
    containment: ContainmentRoot,
}

impl ConfigDir {
    const CONFIG_TOML: &'static str = "config.toml";

    /// Open the `.8v/` directory inside `project_root`.
    ///
    /// Returns `Ok(None)` if `.8v/` does not exist. Returns `Err` only on
    /// real I/O failures — permission denied, path is a file, etc.
    pub fn open(project_root: &ProjectRoot) -> Result<Option<Self>, std::io::Error> {
        let dot_8v = PathBuf::from(project_root.to_string()).join(DIR_NAME);

        match std::fs::metadata(&dot_8v) {
            Ok(meta) if meta.is_dir() => {
                let canonical = std::fs::canonicalize(&dot_8v)?;
                let containment = ContainmentRoot::new(&canonical)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                Ok(Some(Self { containment }))
            }
            Ok(_) => {
                // Path exists but is not a directory — treat as I/O error.
                Err(std::io::Error::other(format!(
                    "{} exists but is not a directory",
                    dot_8v.display()
                )))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Path to `.8v/config.toml`.
    pub fn config_toml(&self) -> PathBuf {
        self.containment.as_path().join(Self::CONFIG_TOML)
    }

    /// The containment root for all fs operations inside `.8v/`.
    pub fn containment(&self) -> &ContainmentRoot {
        &self.containment
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_returns_none_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = ProjectRoot::new(tmp.path().to_str().unwrap()).unwrap();
        // No .8v/ directory created — should return Ok(None).
        let result = ConfigDir::open(&root);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert!(
            result.unwrap().is_none(),
            "expected None when .8v/ is absent"
        );
    }

    #[test]
    fn config_dir_opens_when_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".8v")).unwrap();
        let root = ProjectRoot::new(tmp.path().to_str().unwrap()).unwrap();

        let result = ConfigDir::open(&root);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert!(result.unwrap().is_some(), "expected Some when .8v/ exists");
    }

    #[test]
    fn config_dir_config_toml_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".8v")).unwrap();
        let root = ProjectRoot::new(tmp.path().to_str().unwrap()).unwrap();

        let config_dir = ConfigDir::open(&root).unwrap().unwrap();
        let toml_path = config_dir.config_toml();

        // Canonicalize base for comparison (macOS /tmp is a symlink).
        let base = std::fs::canonicalize(tmp.path().join(".8v")).unwrap();
        assert_eq!(toml_path, base.join("config.toml"));
    }
}
