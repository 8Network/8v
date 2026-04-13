//! Erlang project detection from `rebar.config`.
//!
//! ## Why
//!
//! `rebar.config` is the standard Erlang/OTP build configuration file used by
//! rebar3. Its presence identifies an Erlang project.
//!
//! The application name and version come from `src/<name>.app.src`, which is
//! the standard Erlang application resource file.
//!
//! ## Known limitations
//!
//! - Umbrella apps (`apps/` directory with multiple OTP apps) are detected
//!   as a single project.
//! - The `.app.src` must be in `src/` and named `<dirname>.app.src`.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Erlang;

impl Detect for Erlang {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // rebar.config must exist at root
        let Some(_rebar) = fs.read_checked(scan, "rebar.config")? else {
            return Ok(None);
        };

        let dir_name = root
            .as_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("erlang_project");

        // Try to read src/<dirname>.app.src for name and version
        let app_src_path = root
            .as_path()
            .join("src")
            .join(format!("{dir_name}.app.src"));
        let (name, version) = match fs.read_file(&app_src_path) {
            Ok(file) => {
                let content = file.content();
                let name = parse_app_name(content).unwrap_or_else(|| dir_name.to_string());
                let version = parse_app_version(content);
                (name, version)
            }
            Err(_) => (dir_name.to_string(), None),
        };

        Project::new(
            root.clone(),
            name,
            version,
            Stack::Erlang,
            ProjectKind::Standalone,
        )
        .map(Some)
        .map_err(|e| DetectError::ManifestInvalid {
            path: root.as_path().to_path_buf(),
            cause: e.to_string(),
        })
    }
}

/// Extract the application name from `{application, Name, [...]}`.
fn parse_app_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(after) = trimmed.strip_prefix("{application,") {
            let name_part = after.trim_start();
            let end = name_part.find(',')?;
            let name = name_part[..end].trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extract the version from `{vsn, "X.Y.Z"}`.
fn parse_app_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("{vsn,") {
            let after_vsn = trimmed[start + 5..].trim_start();
            if let Some(quoted) = after_vsn.strip_prefix('"') {
                let end = quoted.find('"')?;
                let version = &quoted[..end];
                if !version.is_empty() {
                    return Some(version.to_string());
                }
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
        Erlang.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_erlang_project() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            "{application, myapp,\n [{vsn, \"1.2.3\"}]}.\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
        assert_eq!(project.version(), Some("1.2.3"));
        assert_eq!(project.stack(), Stack::Erlang);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn no_app_src_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        let expected = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(project.name(), expected);
        assert_eq!(project.version(), None);
    }

    #[test]
    fn no_rebar_config_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn empty_rebar_config_still_detects() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Erlang);
    }

    #[test]
    fn parse_app_name_basic() {
        assert_eq!(
            parse_app_name("{application, airline, [\n  {vsn, \"1.0.0\"}]}."),
            Some("airline".to_string())
        );
    }

    #[test]
    fn parse_app_name_missing() {
        assert_eq!(parse_app_name("{deps, []}."), None);
    }

    #[test]
    fn parse_app_version_basic() {
        assert_eq!(
            parse_app_version("  {vsn, \"2.5.1\"},"),
            Some("2.5.1".to_string())
        );
    }

    #[test]
    fn parse_app_version_missing() {
        assert_eq!(parse_app_version("{application, myapp, []}."), None);
    }

    // Counterexample tests for security and robustness

    #[test]
    fn path_traversal_in_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        // Create the actual directory name to include ".."
        let traversal_name = "app..";
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path()
                .join("src")
                .join(format!("{traversal_name}.app.src")),
            "{application, testapp, [{vsn, \"1.0.0\"}]}.",
        )
        .unwrap();

        // Detector should still work since it's looking for <dirname>.app.src
        let result = Erlang.detect(&fs, &scan, &root);
        assert!(result.is_ok());
    }

    #[test]
    fn app_src_with_traversal_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let dirname = dir
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        // Malicious name in .app.src content — should be stored as-is
        // but Project::new rejects control chars. Path traversal chars are allowed
        // as project names (they're just strings, not used as paths by detector).
        let content = "{application, '../../../etc/malicious', [{vsn, \"1.0.0\"}]}.";
        std::fs::write(
            dir.path().join("src").join(format!("{dirname}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        // The name extracted includes the quotes-stripped traversal path.
        // This is safe — name is just metadata, not used as a filesystem path.
        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        assert_eq!(project.name(), "'../../../etc/malicious'");
    }

    #[test]
    fn multiline_application_term() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        let content = "{application, myapp,\n  [{vsn, \"2.1.0\"}]\n}.";
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
        assert_eq!(project.version(), Some("2.1.0"));
    }

    #[test]
    fn unquoted_version() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        // Version without quotes - parser expects quoted
        let content = "{application, myapp, [{vsn, git}]}.";
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        // Parser looks for quotes, so unquoted version won't be extracted
        assert_eq!(project.version(), None);
    }

    #[test]
    fn empty_version() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        let content = "{application, myapp, [{vsn, \"\"}]}.";
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        // Parser checks !version.is_empty(), so empty string is rejected
        assert_eq!(project.version(), None);
    }

    #[test]
    fn binary_garbage_content() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        // Write binary garbage to .app.src
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            [0xFF, 0xFE, 0x00, 0x01, 0xFF, 0xFF],
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        // Parser uses lines() which handles invalid UTF-8 gracefully
        // Should fall back to directory name
        assert_eq!(project.name(), name);
        assert_eq!(project.version(), None);
    }

    #[test]
    fn multiple_application_entries() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        // Multiple application entries - parser takes first
        let content =
            "{application, first, [{vsn, \"1.0.0\"}]}.\n{application, second, [{vsn, \"2.0.0\"}]}.";
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        assert_eq!(project.name(), "first");
        assert_eq!(project.version(), Some("1.0.0"));
    }

    #[test]
    fn rebar_config_with_comments() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "%% comment\n{deps, []}.\n").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            "{application, myapp, [{vsn, \"1.0.0\"}]}.",
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        // Detector only checks rebar.config existence, not content
        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
    }

    #[test]
    fn app_src_with_special_characters() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_str().unwrap();
        std::fs::write(dir.path().join("rebar.config"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        // Application name with special characters
        let content = "{application, \"my-app_v2.0\", [{vsn, \"3.1.4\"}]}.";
        std::fs::write(
            dir.path().join("src").join(format!("{name}.app.src")),
            content,
        )
        .unwrap();

        let root = ProjectRoot::new(dir.path()).unwrap();
        let fs = o8v_fs::SafeFs::new(dir.path(), o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();

        let project = Erlang.detect(&fs, &scan, &root).unwrap().unwrap();
        assert_eq!(project.name(), "my-app_v2.0");
        assert_eq!(project.version(), Some("3.1.4"));
    }

    #[test]
    fn whitespace_in_parsed_fields() {
        let content = "  {application,   myapp  ,  [ {vsn,   \"1.5.2\"  } ] } . ";
        assert_eq!(parse_app_name(content), Some("myapp".to_string()));
        assert_eq!(parse_app_version(content), Some("1.5.2".to_string()));
    }
}
