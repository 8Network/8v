//! Go project detection from `go.mod`.
//!
//! ## Why
//!
//! `go.mod` is the universal Go module manifest. The `module` directive
//! declares the module path (name), and the `go` directive declares the
//! minimum Go version.
//!
//! Go does not have a native workspace format in `go.mod`. Go workspaces
//! use a separate `go.work` file — not yet detected.
//!
//! ## Known limitations
//!
//! - `go.work` (Go workspace) is not detected.
//! - Module version is the Go language version (`go 1.21`), not a
//!   module release version. Go modules are versioned via git tags.
//! - The module path is used as the project name. For paths like
//!   `github.com/user/repo`, the full path is the name.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Go;

impl Detect for Go {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        let Some(file) = fs.read_checked(scan, "go.mod")? else {
            return Ok(None);
        };

        let name = parse_module(file.content()).ok_or_else(|| DetectError::ManifestInvalid {
            path: file.path().to_path_buf(),
            cause:
                "go.mod has no module directive — add 'module your/module/path' as the first line"
                    .into(),
        })?;

        let version = parse_go_version(file.content());

        Project::new(
            root.clone(),
            name,
            version,
            Stack::Go,
            ProjectKind::Standalone,
        )
        .map(Some)
        .map_err(|e| DetectError::ManifestInvalid {
            path: file.path().to_path_buf(),
            cause: e.to_string(),
        })
    }
}

/// Extract the module path from the `module` directive.
/// Format: `module github.com/user/repo`
/// Uses space/tab boundary to avoid matching `moduleinfo` etc.
fn parse_module(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        // Match "module " or "module\t" — not "modulefoo"
        let path = trimmed
            .strip_prefix("module ")
            .or_else(|| trimmed.strip_prefix("module\t"));
        if let Some(path) = path {
            // Strip inline comments: module github.com/app // pinned
            let path = path.find("//").map_or(path, |i| &path[..i]);
            // Strip quotes: module "example.com/app" (go.mod allows quoted paths)
            let path = path.trim().trim_matches('"');
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

/// Extract the Go version from the `go` directive.
/// Format: `go 1.21`
fn parse_go_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        let version = trimmed
            .strip_prefix("go ")
            .or_else(|| trimmed.strip_prefix("go\t"));
        if let Some(version) = version {
            let version = version.find("//").map_or(version, |i| &version[..i]);
            let version = version.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
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
        Go.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_go_module() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/myapp\n\ngo 1.21\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "github.com/user/myapp");
        assert_eq!(project.version(), Some("1.21"));
        assert_eq!(project.stack(), Stack::Go);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn simple_module_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module myapp\n\ngo 1.22\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
    }

    #[test]
    fn no_go_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module myapp\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
        assert_eq!(project.version(), None);
    }

    #[test]
    fn no_module_directive_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "go 1.21\n").unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn empty_go_mod_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "").unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn no_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn module_with_require_block() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "\
module github.com/user/app

go 1.21

require (
\tgithub.com/foo/bar v1.2.3
\tgithub.com/baz/qux v0.1.0
)
",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "github.com/user/app");
        assert_eq!(project.version(), Some("1.21"));
    }

    #[test]
    fn module_prefix_not_matched() {
        // "moduleinfo" should NOT match "module " — word boundary required
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "moduleinfo github.com/wrong\nmodule github.com/right\ngo 1.21\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            "github.com/right",
            "should match 'module ' not 'moduleinfo'"
        );
    }

    #[test]
    fn module_with_version_suffix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/repo/v2\n\ngo 1.22\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "github.com/user/repo/v2");
    }

    #[test]
    fn inline_comment_stripped_from_module() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/app // pinned\n\ngo 1.21\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            "github.com/app",
            "inline comment should be stripped"
        );
    }

    #[test]
    fn tab_separated_go_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module myapp\n\ngo\t1.23.0\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.version(),
            Some("1.23.0"),
            "tab-separated go version should be detected"
        );
    }

    #[test]
    fn control_chars_in_module_name_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module bad\x01name\n\ngo 1.21\n").unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn module_line_comment_only_is_error() {
        // "module // comment" — path is empty after stripping inline comment
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module // this is just a comment\n\ngo 1.21\n",
        )
        .unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn go_version_with_inline_comment() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module myapp\n\ngo 1.21 // minimum version\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.version(),
            Some("1.21"),
            "inline comment should be stripped from go version"
        );
    }

    #[test]
    fn go_version_comment_only() {
        // "go // comment" — version empty after strip
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module myapp\n\ngo // no version\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.version(),
            None,
            "go line with only comment should produce no version"
        );
    }
}
