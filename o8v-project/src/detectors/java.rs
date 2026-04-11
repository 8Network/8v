//! Java project detection from `pom.xml` (Maven) or `build.gradle` (Gradle).
//!
//! ## Why
//!
//! Maven uses `pom.xml` as the project manifest, and Gradle uses `build.gradle` (Groovy DSL).
//! Both are detected by their presence. If both exist, Maven takes priority (pom.xml
//! is checked first). Note: `build.gradle.kts` (Kotlin DSL) is exclusively claimed by the Kotlin detector.
//!
//! Name extraction is simple: for Maven, we try to extract `<artifactId>` from
//! `pom.xml`. If parsing fails or the field is missing, we fall back to the
//! directory name. For Gradle, we use the directory name as the project name
//! (parsing Gradle DSL is complex and unnecessary for detection).
//!
//! ## Known limitations
//!
//! - Maven workspace detection is not supported.
//! - Gradle composite builds are not detected.
//! - Version extraction from pom.xml is not implemented — use directory structure.

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Java;

impl Detect for Java {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for Maven first (pom.xml takes priority over Gradle)
        if let Some(file) = fs.read_checked(scan, "pom.xml")? {
            let name =
                extract_artifact_id(file.content()).unwrap_or_else(|| root.dir_name().to_string());

            return Project::new(
                root.clone(),
                name,
                None,
                Stack::Java,
                ProjectKind::Standalone,
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: e.to_string(),
            });
        }

        // Check for Gradle: build.gradle (Groovy DSL only)
        // Note: build.gradle.kts (Kotlin DSL) is claimed by the Kotlin detector
        if let Some(_file) = fs.read_checked(scan, "build.gradle")? {
            let name = root.dir_name().to_string();

            return Project::new(
                root.clone(),
                name,
                None,
                Stack::Java,
                ProjectKind::Standalone,
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: root.as_path().join("build.gradle").to_path_buf(),
                cause: e.to_string(),
            });
        }

        // No Maven or Gradle detected
        Ok(None)
    }
}

/// Extract `<artifactId>` from pom.xml content.
/// Simple line-by-line search — does not handle complex XML structure.
fn extract_artifact_id(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(id) = trimmed
            .strip_prefix("<artifactId>")
            .and_then(|s| s.strip_suffix("</artifactId>"))
        {
            let id = id.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
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
        Java.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_maven_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
</project>
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-app");
        assert_eq!(project.stack(), Stack::Java);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn detects_gradle_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle"),
            "plugins {\n    id 'java'\n}\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Java);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn maven_takes_priority_over_gradle() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>maven-app</artifactId>
</project>
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("build.gradle"),
            "plugins {\n    id 'java'\n}\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "maven-app", "Maven should take priority");
    }

    #[test]
    fn fallback_to_directory_name_when_artifact_id_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <version>1.0.0</version>
</project>
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
    fn no_java_project() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn artifact_id_with_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>  my-app  </artifactId>
</project>
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "my-app");
    }

    #[test]
    fn empty_artifact_id_falls_back_to_directory_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId></artifactId>
</project>
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
    }
}
