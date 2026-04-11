//! Terraform project detection from `*.tf` files.
//!
//! ## Why
//!
//! A Terraform project is recognized by the presence of `*.tf` files.
//! The project name defaults to the directory name.

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Terraform;

impl Detect for Terraform {
    fn detect(
        &self,
        _fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for any .tf files in the directory
        let has_tf_files = scan.entries_with_extension("tf").next().is_some();

        if !has_tf_files {
            return Ok(None);
        }

        // Use directory name as project name
        let name = root.dir_name().to_string();

        Project::new(
            root.clone(),
            name,
            None, // Terraform doesn't have a standard version file
            Stack::Terraform,
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
        Terraform.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_terraform_main_tf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.tf"),
            "resource \"aws_instance\" \"web\" {}\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(project.version(), None);
        assert_eq!(project.stack(), Stack::Terraform);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn detects_terraform_variables_tf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("variables.tf"),
            "variable \"instance_count\" {}\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Terraform);
    }

    #[test]
    fn detects_terraform_outputs_tf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("outputs.tf"), "output \"instance_id\" {}\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Terraform);
    }

    #[test]
    fn detects_multiple_tf_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.tf"), "# main\n").unwrap();
        std::fs::write(dir.path().join("variables.tf"), "# vars\n").unwrap();
        std::fs::write(dir.path().join("outputs.tf"), "# outputs\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Terraform);
    }

    #[test]
    fn no_terraform_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Not Terraform\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn ignores_non_tf_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.py"), "print('hello')\n").unwrap();
        std::fs::write(dir.path().join("main.go"), "package main\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn terraform_in_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let tf_dir = dir.path().join("terraform");
        std::fs::create_dir(&tf_dir).unwrap();
        std::fs::write(
            tf_dir.join("main.tf"),
            "resource \"aws_instance\" \"web\" {}\n",
        )
        .unwrap();

        let project = detect_in(&tf_dir).unwrap().unwrap();
        assert_eq!(project.name(), "terraform");
        assert_eq!(project.stack(), Stack::Terraform);
    }

    #[test]
    fn empty_tf_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("empty.tf"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Terraform);
    }
}
