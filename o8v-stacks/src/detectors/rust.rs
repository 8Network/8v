//! Rust project detection from `Cargo.toml`.
//!
//! ## Why
//!
//! `Cargo.toml` is the universal Rust manifest. Detection handles three cases:
//! - **Standalone crate**: `[package]` section with name and version.
//! - **Workspace root**: `[workspace]` + `[package]` — a workspace that is also a package.
//! - **Virtual workspace**: `[workspace]` without `[package]` — the workspace root
//!   itself isn't a package. Name is derived from the directory name.
//!
//! Field inheritance (`version.workspace = true`) is recognized but not resolved —
//! the value comes from the workspace root, which may be a different directory.
//!
//! ## Known limitations
//!
//! - Workspace member globs are not expanded — stored as raw strings.
//! - Inherited field values are not resolved — version returns `None` for members.
//! - Workspace auto-discovery (no explicit `members`) is not supported.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};
use serde::Deserialize;

pub struct Rust;

// ─── Manifest types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CargoManifest {
    package: Option<PackageSection>,
    workspace: Option<WorkspaceSection>,
}

#[derive(Deserialize)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
}

#[derive(Deserialize)]
struct PackageSection {
    #[serde(default)]
    name: FieldValue,
    #[serde(default)]
    version: FieldValue,
}

/// A Cargo.toml field that can be a literal value or inherited from workspace.
#[derive(Deserialize, Default)]
#[serde(untagged)]
enum FieldValue {
    Literal(String),
    Inherited {
        workspace: bool,
    },
    #[default]
    Absent,
}

// ─── Detection ─────────────────────────────────────────────────────────────

impl Detect for Rust {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        let Some(file) = fs.read_checked(scan, "Cargo.toml")? else {
            return Ok(None);
        };

        let manifest: CargoManifest =
            toml::from_str(file.content()).map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: o8v_fs::truncate_error(&format!("{e}"), "check Cargo.toml format"),
            })?;

        // Workspace?
        if let Some(workspace) = manifest.workspace {
            let members = workspace.members;

            // Cargo supports auto-discovery: [workspace] without explicit members.
            // We can't detect members without scanning subdirs.
            // But if [package] exists, the root itself is a real project —
            // detect it as standalone rather than returning a silent false negative.
            if members.is_empty() {
                return match manifest.package {
                    Some(pkg) => {
                        let name = resolve_field(pkg.name, "name", false, file.path())?
                            .ok_or_else(|| DetectError::ManifestInvalid {
                                path: file.path().to_path_buf(),
                                cause: "workspace root has [package] but no name field".into(),
                            })?;
                        let version = resolve_field(pkg.version, "version", false, file.path())?;
                        make_project(root, name, version, ProjectKind::Standalone, file.path())
                    }
                    None => Ok(None), // pure auto-discovery workspace, no package — can't detect
                };
            }

            let (name, version) = if let Some(pkg) = manifest.package {
                let n = resolve_field(pkg.name, "name", true, file.path())?;
                let v = resolve_field(pkg.version, "version", true, file.path())?;
                (n, v)
            } else {
                let n = fs.dir_name().unwrap_or("unknown").to_string();
                (Some(n), None)
            };

            let name = name.ok_or_else(|| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: "workspace has no project name".into(),
            })?;

            return make_project(
                root,
                name,
                version,
                ProjectKind::Compound { members },
                file.path(),
            );
        }

        // Standalone — [package] must exist
        if let Some(pkg) = manifest.package {
            let name = resolve_field(pkg.name, "name", false, file.path())?
                .ok_or_else(|| DetectError::ManifestInvalid {
                    path: file.path().to_path_buf(),
                    cause: "Cargo.toml has [package] but no name field — add name = \"your-project\" to [package]".into(),
                })?;
            let version = resolve_field(pkg.version, "version", false, file.path())?;

            return make_project(root, name, version, ProjectKind::Standalone, file.path());
        }

        // No [workspace] and no [package] — not a root project
        Ok(None)
    }
}

fn resolve_field(
    value: FieldValue,
    field_name: &str,
    is_workspace_root: bool,
    manifest: &std::path::Path,
) -> Result<Option<String>, DetectError> {
    match value {
        FieldValue::Literal(s) => Ok(Some(s)),
        FieldValue::Absent => Ok(None),
        FieldValue::Inherited { workspace: true } => {
            if is_workspace_root {
                Err(DetectError::ManifestInvalid {
                    path: manifest.to_path_buf(),
                    cause: format!("workspace root has {field_name}.workspace = true — cannot inherit from self. Set {field_name} directly or remove .workspace = true"),
                })
            } else {
                Ok(None)
            }
        }
        FieldValue::Inherited { workspace: false } => {
            Err(DetectError::ManifestInvalid {
                path: manifest.to_path_buf(),
                cause: format!("{field_name}.workspace = false is not valid — set {field_name} directly or use .workspace = true to inherit"),
            })
        }
    }
}

fn make_project(
    root: &ProjectRoot,
    name: String,
    version: Option<String>,
    kind: ProjectKind,
    manifest: &std::path::Path,
) -> Result<Option<Project>, DetectError> {
    Project::new(root.clone(), name, version, Stack::Rust, kind)
        .map(Some)
        .map_err(|e| DetectError::ManifestInvalid {
            path: manifest.to_path_buf(),
            cause: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::project::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        Rust.detect(&fs, &scan, &root)
    }

    #[test]
    fn standalone_crate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"my-app\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-app");
        assert_eq!(project.version(), Some("1.0.0"));
        assert_eq!(project.stack(), Stack::Rust);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn workspace_with_package() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n\n[package]\nname = \"my-ws\"\nversion = \"0.1.0\"\n",
        ).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-ws");
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["crates/a", "crates/b"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn virtual_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\"]\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Rust);
        let expected = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(
            project.name(),
            expected,
            "virtual workspace should use directory name"
        );
    }

    #[test]
    fn standalone_inherited_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"member\"\nversion.workspace = true\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "member");
        assert_eq!(project.version(), None);
    }

    #[test]
    fn workspace_root_inherited_name_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n\n[package]\nname.workspace = true\nversion = \"1.0.0\"\n",
        ).unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn no_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "not valid toml {{{").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn name_wrong_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = 42\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn workspace_not_table() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "workspace = 42\n").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn package_not_table() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "package = 42\n").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn auto_discovery_without_package_returns_none() {
        // [workspace] without members and no [package] — pure auto-discovery, can't detect
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn auto_discovery_with_package_detects_standalone() {
        // [workspace] without members BUT [package] exists — the root is a real project
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\n\n[package]\nname = \"root-app\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let project = detect_in(dir.path())
            .unwrap()
            .expect("workspace root with [package] must be detected");
        assert_eq!(project.name(), "root-app");
        assert_eq!(project.version(), Some("0.1.0"));
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn no_package_no_workspace() {
        // Cargo.toml with neither [package] nor [workspace] — not a root
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nrand = \"0.8\"\n",
        )
        .unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn package_without_name() {
        // [package] section but no name field
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        assert!(format!("{err}").contains("no name field"));
    }

    #[test]
    fn workspace_inherited_name_on_workspace_root_is_error() {
        // name.workspace = true at the workspace root — can't inherit from self
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n\n[package]\nname.workspace = true\nversion = \"1.0.0\"\n",
        ).unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn version_workspace_false_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"app\"\nversion.workspace = false\n",
        )
        .unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn control_chars_in_cargo_name_is_error() {
        let dir = tempfile::tempdir().unwrap();
        // Use actual control character via raw bytes
        let content = b"[package]\nname = \"bad\x01name\"\nversion = \"1.0.0\"\n";
        std::fs::write(dir.path().join("Cargo.toml"), content).unwrap();

        // TOML spec disallows control chars in basic strings, so this should be a parse error
        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn workspace_with_unnamed_package() {
        // [workspace] + [package] but no name field — workspace root has no project name
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n\n[package]\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        assert!(format!("{err}").contains("no project name"));
    }

    #[test]
    fn whitespace_name_in_cargo_triggers_project_error() {
        // TOML preserves whitespace in strings — Project::new rejects leading/trailing whitespace
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \" spaced \"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }
}
