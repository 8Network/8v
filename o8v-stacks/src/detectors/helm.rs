//! Helm chart project detection from `Chart.yaml`.
//!
//! ## Why
//!
//! A Helm chart project is recognized by the presence of a `Chart.yaml` file.
//! The project name defaults to the directory name.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Helm;

impl Detect for Helm {
    fn detect(
        &self,
        _fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for Chart.yaml (exact name)
        let has_chart_yaml = scan.by_name("Chart.yaml").is_some();

        if !has_chart_yaml {
            return Ok(None);
        }

        // Use directory name as project name
        let name = root.dir_name().to_string();

        Project::new(
            root.clone(),
            name,
            None, // Chart.yaml version is inside the file but we don't parse it here
            Stack::Helm,
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
        Helm.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_helm_chart() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Chart.yaml"),
            "apiVersion: v2\nname: my-chart\nversion: 0.1.0\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(project.version(), None);
        assert_eq!(project.stack(), Stack::Helm);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn empty_chart_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Chart.yaml"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Helm);
    }

    #[test]
    fn no_chart_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("values.yaml"), "replicaCount: 1\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn ignores_chart_yml() {
        // Must be Chart.yaml, not Chart.yml
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Chart.yml"), "apiVersion: v2\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn ignores_lowercase_chart_yaml() {
        // Must be Chart.yaml (capital C), not chart.yaml
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("chart.yaml"), "apiVersion: v2\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn helm_with_templates_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Chart.yaml"),
            "apiVersion: v2\nname: my-chart\nversion: 0.1.0\n",
        )
        .unwrap();
        let templates_dir = dir.path().join("templates");
        std::fs::create_dir(&templates_dir).unwrap();
        std::fs::write(
            templates_dir.join("deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Helm);
    }

    #[test]
    fn helm_chart_in_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let chart_dir = dir.path().join("my-chart");
        std::fs::create_dir(&chart_dir).unwrap();
        std::fs::write(
            chart_dir.join("Chart.yaml"),
            "apiVersion: v2\nname: my-chart\nversion: 0.1.0\n",
        )
        .unwrap();

        let project = detect_in(&chart_dir).unwrap().unwrap();
        assert_eq!(project.name(), "my-chart");
        assert_eq!(project.stack(), Stack::Helm);
    }
}
