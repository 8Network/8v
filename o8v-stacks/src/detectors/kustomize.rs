//! Kustomize project detection from `kustomization.yaml` or `kustomization.yml`.
//!
//! ## Why
//!
//! A Kustomize project is recognized by the presence of a `kustomization.yaml`
//! or `kustomization.yml` file in the directory root.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Kustomize;

impl Detect for Kustomize {
    fn detect(
        &self,
        _fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for kustomization.yaml or kustomization.yml
        let has_kustomization = scan.by_name("kustomization.yaml").is_some()
            || scan.by_name("kustomization.yml").is_some();

        if !has_kustomization {
            return Ok(None);
        }

        // Use directory name as project name
        let name = root.dir_name().to_string();

        Project::new(
            root.clone(),
            name,
            None, // Kustomize doesn't have a standard version file
            Stack::Kustomize,
            ProjectKind::Standalone,
        )
        .map(Some)
        .map_err(|e| DetectError::ManifestInvalid {
            path: root.as_path().to_path_buf(),
            cause: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        Kustomize.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_kustomization_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("kustomization.yaml"),
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(project.version(), None);
        assert_eq!(project.stack(), Stack::Kustomize);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn detects_kustomization_yml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("kustomization.yml"),
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kustomize);
    }

    #[test]
    fn yaml_takes_priority_over_yml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("kustomization.yaml"),
            "kind: Kustomization\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("kustomization.yml"),
            "kind: Kustomization\n",
        )
        .unwrap();

        // Both present — still detects as Kustomize
        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kustomize);
    }

    #[test]
    fn empty_kustomization_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kustomization.yaml"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kustomize);
    }

    #[test]
    fn ignores_unrelated_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deployment.yaml"), "kind: Deployment\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn no_kustomization_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Not Kustomize\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn kustomize_ignores_other_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("kustomization.yaml"),
            "kind: Kustomization\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("deployment.yaml"), "kind: Deployment\n").unwrap();
        std::fs::write(dir.path().join("service.yaml"), "kind: Service\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kustomize);
    }

    #[test]
    fn kustomization_in_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let overlay_dir = dir.path().join("overlays").join("prod");
        std::fs::create_dir_all(&overlay_dir).unwrap();
        std::fs::write(
            overlay_dir.join("kustomization.yaml"),
            "kind: Kustomization\n",
        )
        .unwrap();

        let project = detect_in(&overlay_dir).unwrap().unwrap();
        assert_eq!(project.name(), "prod");
        assert_eq!(project.stack(), Stack::Kustomize);
    }

    #[test]
    fn case_sensitive_no_uppercase() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Kustomization.yaml"),
            "kind: Kustomization\n",
        )
        .unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }
}
