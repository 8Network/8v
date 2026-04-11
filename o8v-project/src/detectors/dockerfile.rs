//! Dockerfile project detection from `Dockerfile`.
//!
//! ## Why
//!
//! A Dockerfile project is recognized by the presence of a `Dockerfile` file.
//! The project name defaults to the directory name.

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Dockerfile;

impl Detect for Dockerfile {
    fn detect(
        &self,
        _fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for Dockerfile (exact name)
        let has_dockerfile = scan.by_name("Dockerfile").is_some();

        if !has_dockerfile {
            return Ok(None);
        }

        // Use directory name as project name
        let name = root.dir_name().to_string();

        Project::new(
            root.clone(),
            name,
            None, // Dockerfile doesn't have a standard version file
            Stack::Dockerfile,
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
        Dockerfile.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_dockerfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM ubuntu:20.04\nRUN apt-get update\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(project.version(), None);
        assert_eq!(project.stack(), Stack::Dockerfile);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn empty_dockerfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dockerfile"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Dockerfile);
    }

    #[test]
    fn ignores_dockerfile_dev() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dockerfile.dev"), "FROM ubuntu:20.04\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn ignores_dockerfile_prod() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dockerfile.prod"), "FROM ubuntu:20.04\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn dockerfile_ignores_other_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Dockerfile"), "FROM ubuntu\n").unwrap();
        std::fs::write(dir.path().join("docker-compose.yml"), "version: '3'\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Dockerfile);
    }

    #[test]
    fn no_dockerfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Not Docker\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn dockerfile_in_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let docker_dir = dir.path().join("docker");
        std::fs::create_dir(&docker_dir).unwrap();
        std::fs::write(docker_dir.join("Dockerfile"), "FROM alpine\n").unwrap();

        let project = detect_in(&docker_dir).unwrap().unwrap();
        assert_eq!(project.name(), "docker");
        assert_eq!(project.stack(), Stack::Dockerfile);
    }

    #[test]
    fn case_sensitive_dockerfile_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dockerfile"), "FROM ubuntu\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn dockerfile_multiline() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM ubuntu:20.04\n\
            RUN apt-get update && apt-get install -y curl\n\
            COPY . /app\n\
            WORKDIR /app\n\
            CMD [\"./run.sh\"]\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Dockerfile);
    }
}
