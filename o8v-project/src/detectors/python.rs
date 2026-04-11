//! Python project detection from `pyproject.toml`.
//!
//! ## Why
//!
//! `pyproject.toml` is the modern Python manifest (PEP 621). Name and version
//! come from `[project]`. Poetry projects use `[tool.poetry]` instead — both
//! are checked, with `[project]` taking priority.
//!
//! A `pyproject.toml` with only tool configuration (e.g. `[tool.ruff]`) is
//! config-only and returns `None` — it's not a Python project declaration.
//! But if `[project]` or `[tool.poetry]` exists without a name, that's an
//! error — the section declares a project but is incomplete.
//!
//! ## Known limitations
//!
//! - Hatch workspaces use a different structure — not detected.
//! - Poetry workspaces are not a standard feature — not detected.
//! - PDM workspaces are not detected.
//! - `setup.py`/`setup.cfg`-only projects are invisible.

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};
use serde::Deserialize;

pub struct Python;

// ─── Manifest types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PyprojectToml {
    project: Option<ProjectSection>,
    tool: Option<ToolSection>,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct ToolSection {
    poetry: Option<PoetrySection>,
    uv: Option<UvSection>,
}

#[derive(Deserialize)]
struct PoetrySection {
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct UvSection {
    workspace: Option<UvWorkspace>,
}

#[derive(Deserialize)]
struct UvWorkspace {
    #[serde(default)]
    members: Vec<String>,
}

// ─── Detection ─────────────────────────────────────────────────────────────

impl Detect for Python {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        let Some(file) = fs.read_checked(scan, "pyproject.toml")? else {
            return Ok(None);
        };

        let manifest: PyprojectToml =
            toml::from_str(file.content()).map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: o8v_fs::truncate_error(&format!("{e}"), "check pyproject.toml format"),
            })?;

        // Name resolution — PEP 621 requires name when [project] exists.
        // Do NOT fall back to [tool.poetry] if [project] is present.
        let name = if let Some(ref project) = manifest.project {
            // [project] exists — name MUST come from here per PEP 621
            match &project.name {
                Some(n) => n.clone(),
                None => return Err(DetectError::ManifestInvalid {
                    path: file.path().to_path_buf(),
                    cause: "pyproject.toml has [project] but no name field — add name = \"your-project\" under [project]".into(),
                }),
            }
        } else if let Some(poetry) = manifest.tool.as_ref().and_then(|t| t.poetry.as_ref()) {
            // No [project] — try [tool.poetry]
            match &poetry.name {
                Some(n) => n.clone(),
                None => return Err(DetectError::ManifestInvalid {
                    path: file.path().to_path_buf(),
                    cause: "pyproject.toml has [tool.poetry] but no name field — add name = \"your-project\" under [tool.poetry]".into(),
                }),
            }
        } else {
            // No [project] and no [tool.poetry] — config-only
            return Ok(None);
        };

        // Version — same source as name: [project] if present, otherwise [tool.poetry]
        let version = if manifest.project.is_some() {
            manifest.project.as_ref().and_then(|p| p.version.clone())
        } else {
            manifest
                .tool
                .as_ref()
                .and_then(|t| t.poetry.as_ref())
                .and_then(|p| p.version.clone())
        };

        // uv workspaces
        let kind = manifest
            .tool
            .as_ref()
            .and_then(|t| t.uv.as_ref())
            .and_then(|u| u.workspace.as_ref())
            .map_or(ProjectKind::Standalone, |ws| {
                // Empty workspace arrays should not create Compound projects
                if ws.members.is_empty() {
                    ProjectKind::Standalone
                } else {
                    ProjectKind::Compound {
                        members: ws.members.clone(),
                    }
                }
            });

        Project::new(root.clone(), name, version, Stack::Python, kind)
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: e.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        Python.detect(&fs, &scan, &root)
    }

    #[test]
    fn standalone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"my-lib\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-lib");
        assert_eq!(project.version(), Some("0.1.0"));
        assert_eq!(project.stack(), Stack::Python);
    }

    #[test]
    fn poetry_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.poetry]\nname = \"poetry-app\"\nversion = \"2.0.0\"\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "poetry-app");
        assert_eq!(project.version(), Some("2.0.0"));
    }

    #[test]
    fn config_only_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.ruff]\nline-length = 88\n",
        )
        .unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn project_without_name_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn no_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "not valid {{{").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn name_wrong_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = 42\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn project_without_name_does_not_fallback_to_poetry() {
        // PEP 621: if [project] exists, name MUST come from [project]
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"1.0.0\"\n\n[tool.poetry]\nname = \"poetry-name\"\n",
        )
        .unwrap();

        // Should error — not fall back to poetry name
        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }
}
