//! JavaScript project detection from `package.json`.
//!
//! ## Why
//!
//! JavaScript detection uses `package.json` WITHOUT `tsconfig.json`.
//! If `tsconfig.json` exists, the TypeScript detector handles it instead.
//! This ensures plain Node.js apps, which were previously invisible, are detected.
//!
//! The JavaScript detector runs AFTER the TypeScript detector. If TypeScript
//! already claimed the project, JavaScript returns `None` to avoid duplicates.
//!
//! ## Known limitations
//!
//! - No runtime distinction (Node.js vs Bun). Bun uses the same `package.json`.
//! - pnpm workspaces (`pnpm-workspace.yaml`) are not detected.
//! - Workspace detection only covers npm/yarn `"workspaces"` field.

use super::npm::{PackageJson, Workspaces};
use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct JavaScript;

impl Detect for JavaScript {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        let Some(file) = fs.read_checked(scan, "package.json")? else {
            return Ok(None);
        };

        // If tsconfig.json exists in ANY form (file, directory, symlink),
        // the TypeScript detector handles this project. Even if tsconfig is broken
        // (directory, dangling symlink), the intent is TypeScript — TS will surface
        // the error. JS should not claim the project as a fallback.
        if scan.by_name("tsconfig.json").is_some() {
            return Ok(None);
        }

        let pkg: PackageJson =
            serde_json::from_str(file.content()).map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: o8v_fs::truncate_error(&format!("{e}"), "check package.json format"),
            })?;

        // If `typescript` is in dependencies/devDependencies, the TypeScript
        // detector handles this project (even without a root tsconfig.json).
        if pkg.has_typescript_dep() {
            return Ok(None);
        }

        let name = pkg
            .name
            .unwrap_or_else(|| fs.dir_name().unwrap_or("unknown").trim().to_string());

        let version = pkg.version;

        let kind = pkg
            .workspaces
            .map_or(ProjectKind::Standalone, Workspaces::into_kind);

        Project::new(root.clone(), name, version, Stack::JavaScript, kind)
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
    use o8v_core::project::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        JavaScript.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_plain_node_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "version": "1.0.0"}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-app");
        assert_eq!(project.version(), Some("1.0.0"));
        assert_eq!(project.stack(), Stack::JavaScript);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn skipped_when_tsconfig_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "ts-app"}"#).unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn skipped_when_typescript_in_devdependencies() {
        // Project with typescript in devDeps but no tsconfig at root
        // must be handled by the TypeScript detector, not JavaScript.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "ts-app", "devDependencies": {"typescript": "^5.0.0"}}"#,
        )
        .unwrap();

        assert!(
            detect_in(dir.path()).unwrap().is_none(),
            "project with typescript devDep must not be detected as JavaScript"
        );
    }

    #[test]
    fn skipped_when_typescript_in_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "ts-lib", "dependencies": {"typescript": "^5.0.0"}}"#,
        )
        .unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn detects_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "mono", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["packages/*"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn no_package_json() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn no_name_falls_back_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"version": "1.0.0"}"#).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        let expected = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(
            project.name(),
            expected,
            "should fall back to directory name"
        );
    }

    #[test]
    fn invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "not json!!!").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn name_wrong_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": 42}"#).unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn empty_workspaces_array_is_standalone() {
        // Bug regression test: "workspaces": [] should return Standalone, not a zero-member Workspace.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "mono", "workspaces": []}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert!(
            matches!(project.kind(), ProjectKind::Standalone),
            "empty workspaces array should be Standalone, got {:?}",
            project.kind()
        );
    }
}
