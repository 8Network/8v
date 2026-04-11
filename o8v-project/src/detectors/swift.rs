//! Swift project detection from `Package.swift`.
//!
//! ## Why
//!
//! Swift Package Manager uses `Package.swift` as the project manifest.
//! Detection is simple: check for presence of `Package.swift`.
//!
//! ## Known limitations
//!
//! - Xcode projects (`.xcodeproj`) are not detected.
//! - Cocoapods projects (`Podfile`) are not detected.

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Swift;

impl Detect for Swift {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for Package.swift (Swift Package Manager)
        if let Some(_file) = fs.read_checked(scan, "Package.swift")? {
            let name = root.dir_name().to_string();

            return Project::new(
                root.clone(),
                name,
                None,
                Stack::Swift,
                ProjectKind::Standalone,
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: root.as_path().join("Package.swift").to_path_buf(),
                cause: e.to_string(),
            });
        }

        Ok(None)
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
        Swift.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_swift_package_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            r#"// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "MyPackage"
)
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Swift);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn uses_directory_name_as_project_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            r#"import PackageDescription
let package = Package(name: "MyApp")
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn no_swift_project() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn detects_with_swift_source_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            r#"import PackageDescription
let package = Package(name: "MyApp")
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("main.swift"), "print(\"Hello\")\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Swift);
    }

    #[test]
    fn detects_with_subdirectory_sources() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Package.swift"),
            r#"import PackageDescription
let package = Package(name: "MyApp")
"#,
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("Sources")).unwrap();
        std::fs::write(dir.path().join("Sources/main.swift"), "print(\"Hello\")\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Swift);
    }

    #[test]
    fn not_detected_without_package_swift() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.swift"), "print(\"Hello\")\n").unwrap();

        assert!(detect_in(dir.path()).unwrap().is_none());
    }
}
