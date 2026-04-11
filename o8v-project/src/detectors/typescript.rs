//! TypeScript project detection from `package.json` + TypeScript evidence.
//!
//! ## Why
//!
//! TypeScript detection requires `package.json` AND at least one of:
//! - `tsconfig.json` at root, OR
//! - `typescript` listed in `dependencies` or `devDependencies`
//!
//! `package.json` alone is JavaScript, not TypeScript. This is a deliberate
//! choice — 8v treats JS and TS as different stacks because they have
//! different toolchains, different lint rules, and different build systems.
//!
//! The `devDependencies` signal catches projects like the TypeScript compiler
//! itself, where `tsconfig.json` lives in `src/` rather than at the root.
//!
//! Name is optional in `package.json` (private packages may omit it).
//! When absent, the directory name is used as fallback.
//!
//! ## Known limitations
//!
//! - pnpm workspaces are configured in `pnpm-workspace.yaml`, not `package.json`.
//!   This detector only reads `package.json`, so pnpm workspaces are not detected.
//! - Pure JavaScript projects (no tsconfig, no `typescript` dep) are handled
//!   by the JavaScript detector, not this one.
//! - Compound project detection only covers npm/yarn `"workspaces"` field in `package.json`.

use super::npm::{PackageJson, Workspaces};
use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct TypeScript;

// ─── Detection ─────────────────────────────────────────────────────────────

impl Detect for TypeScript {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // TypeScript requires package.json (cheap scan lookup) to avoid
        // reporting tsconfig errors in directories that aren't JS/TS at all.
        if scan.by_name("package.json").is_none() {
            return Ok(None);
        }

        // Read package.json to check both workspaces and dep-based TS signal.
        let Some(file) = fs.read_checked(scan, "package.json")? else {
            return Ok(None);
        };

        let pkg: PackageJson =
            serde_json::from_str(file.content()).map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: o8v_fs::truncate_error(&format!("{e}"), "check package.json format"),
            })?;

        // TypeScript evidence: root-level tsconfig.json OR `typescript` in deps.
        // The dep-based signal catches projects like the TypeScript compiler itself
        // where tsconfig.json lives in src/, not at root.
        let has_tsconfig = fs.validate_entry(scan, "tsconfig.json")?.is_some();
        let has_ts_dep = pkg.has_typescript_dep();

        if !has_tsconfig && !has_ts_dep {
            return Ok(None);
        }

        // Name is optional — private packages may omit it
        let name = pkg
            .name
            .unwrap_or_else(|| fs.dir_name().unwrap_or("unknown").trim().to_string());

        let version = pkg.version;

        // Workspaces
        let kind = pkg
            .workspaces
            .map_or(ProjectKind::Standalone, Workspaces::into_kind);

        Project::new(root.clone(), name, version, Stack::TypeScript, kind)
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
        TypeScript.detect(&fs, &scan, &root)
    }

    #[test]
    fn standalone_with_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "version": "2.0.0"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-app");
        assert_eq!(project.version(), Some("2.0.0"));
        assert_eq!(project.stack(), Stack::TypeScript);
    }

    #[test]
    fn skipped_without_tsconfig_and_no_ts_dep() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "js-app"}"#).unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn detected_via_devdependencies_without_tsconfig() {
        // Matches repos like microsoft/TypeScript where tsconfig is in src/, not root.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "typescript", "devDependencies": {"typescript": "^5.0.0"}}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "typescript");
        assert_eq!(project.stack(), Stack::TypeScript);
    }

    #[test]
    fn detected_via_dependencies_without_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "dependencies": {"typescript": "^5.0.0"}}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::TypeScript);
    }

    #[test]
    fn not_detected_when_other_deps_only() {
        // react in deps without typescript is still JavaScript
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "react-app", "dependencies": {"react": "^18.0.0"}}"#,
        )
        .unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "not json!!!").unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn name_wrong_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": 42}"#).unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn workspace_array_style() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "mono", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["packages/*"]);
        } else {
            panic!("expected compound");
        }
    }

    #[test]
    fn no_name_falls_back_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"version": "1.0.0"}"#).unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        let expected = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(
            project.name(),
            expected,
            "should fall back to directory name"
        );
    }

    #[test]
    fn no_package_json_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        assert!(
            detect_in(dir.path()).unwrap().is_none(),
            "tsconfig without package.json is not TypeScript"
        );
    }

    #[test]
    fn broken_tsconfig_without_package_json_is_none() {
        // Directory with only a broken tsconfig (no package.json) — not TypeScript, no error
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("tsconfig.json")).unwrap();

        assert!(
            detect_in(dir.path()).unwrap().is_none(),
            "broken tsconfig without package.json should not produce an error"
        );
    }

    #[test]
    fn tsconfig_directory_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "app"}"#).unwrap();
        std::fs::create_dir(dir.path().join("tsconfig.json")).unwrap();

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
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert!(
            matches!(project.kind(), ProjectKind::Standalone),
            "empty workspaces array should be Standalone, got {:?}",
            project.kind()
        );
    }

    #[test]
    fn empty_workspaces_object_is_standalone() {
        // Bug regression test: yarn object style with empty packages should be Standalone.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "mono", "workspaces": {"packages": []}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert!(
            matches!(project.kind(), ProjectKind::Standalone),
            "empty packages in workspaces object should be Standalone, got {:?}",
            project.kind()
        );
    }
}
