//! Ruby project detection from `Gemfile`.
//!
//! ## Why
//!
//! `Gemfile` is the universal Ruby dependency manifest. It lists gems (dependencies)
//! and specifies versions. Every Ruby project using Bundler has a Gemfile.
//!
//! Ruby project names are typically inferred from the directory name, as Gemfile
//! itself does not contain a canonical project name declaration. This matches
//! the pattern used by Go (module path → project name).
//!
//! ## Known limitations
//!
//! - Workspace support not implemented (Ruby monorepos use directory structure).
//! - Ruby version constraint (from Gemfile or .ruby-version) is not extracted.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Ruby;

impl Detect for Ruby {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        let Some(_file) = fs.read_checked(scan, "Gemfile")? else {
            return Ok(None);
        };

        let name = root.dir_name().to_string();

        Project::new(
            root.clone(),
            name,
            None,
            Stack::Ruby,
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
    use o8v_core::project::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        Ruby.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_gemfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\n\ngem 'rails', '~> 7.0'\ngem 'sqlite3'\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Ruby);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn project_name_from_directory() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("myapp");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("Gemfile"), "source 'https://rubygems.org'\n").unwrap();

        let project = detect_in(&subdir).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
    }

    #[test]
    fn no_gemfile() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn empty_gemfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Gemfile"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Ruby);
    }

    #[test]
    fn gemfile_with_comments() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Gemfile"),
            "# Ruby project\nsource 'https://rubygems.org'\n\n# Web framework\ngem 'rails'\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Ruby);
    }

    #[test]
    fn gemfile_with_gemlock() {
        // Gemfile.lock is common but not required for detection
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Gemfile.lock"),
            "GEM\n  remote: https://rubygems.org/\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Ruby);
    }
}
