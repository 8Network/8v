//! Kotlin project detection from `build.gradle.kts` (Kotlin DSL) or `build.gradle` with `*.kt` files.
//!
//! ## Why
//!
//! Kotlin projects typically use Gradle with Kotlin DSL (`build.gradle.kts`) or
//! standard Gradle (`build.gradle`) with Kotlin source files (`.kt`).
//! We detect Kotlin if `build.gradle.kts` is present, or if `build.gradle`
//! is present alongside at least one `*.kt` file.
//!
//! ## Known limitations
//!
//! - Simple heuristic: presence of `.kt` files with `build.gradle` indicates Kotlin.
//! - Maven (`pom.xml`) with `*.kt` files is not explicitly detected as Kotlin
//!   (detected as Java instead).

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem};

pub struct Kotlin;

impl Detect for Kotlin {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Check for build.gradle.kts (Kotlin DSL for Gradle) — indicates Kotlin
        if fs.read_checked(scan, "build.gradle.kts")?.is_some() {
            let name = root.dir_name().to_string();

            return Project::new(
                root.clone(),
                name,
                None,
                Stack::Kotlin,
                ProjectKind::Standalone,
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: root.as_path().join("build.gradle.kts").to_path_buf(),
                cause: e.to_string(),
            });
        }

        // Check for build.gradle with .kt files at root level — indicates Kotlin
        if fs.read_checked(scan, "build.gradle")?.is_some() {
            // Check for .kt files in the immediate directory
            if scan.entries_with_extension("kt").next().is_some() {
                let name = root.dir_name().to_string();

                return Project::new(
                    root.clone(),
                    name,
                    None,
                    Stack::Kotlin,
                    ProjectKind::Standalone,
                )
                .map(Some)
                .map_err(|e| DetectError::ManifestInvalid {
                    path: root.as_path().join("build.gradle").to_path_buf(),
                    cause: e.to_string(),
                });
            }
        }

        Ok(None)
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
        Kotlin.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_kotlin_dsl_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle.kts"),
            "plugins {\n    kotlin(\"jvm\") version \"1.9.0\"\n}\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kotlin);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn detects_gradle_with_kotlin_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle"),
            "plugins {\n    id 'org.jetbrains.kotlin.jvm' version '1.9.0'\n}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Main.kt"), "fun main() {}\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kotlin);
    }

    #[test]
    fn detects_build_gradle_kts_takes_priority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle.kts"),
            "plugins {\n    kotlin(\"jvm\")\n}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("build.gradle"), "plugins { }\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kotlin);
    }

    #[test]
    fn gradle_without_kotlin_files_not_detected_as_kotlin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle"),
            "plugins {\n    id 'java'\n}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Main.java"), "public class Main {}\n").unwrap();

        // Should not detect as Kotlin (no .kt files)
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn no_kotlin_project() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn uses_directory_name_as_project_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle.kts"),
            "plugins { kotlin(\"jvm\") }\n",
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            dir.path().file_name().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn multiple_kotlin_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.gradle"),
            "plugins { id 'org.jetbrains.kotlin.jvm' }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Main.kt"), "fun main() {}\n").unwrap();
        std::fs::write(dir.path().join("Util.kt"), "fun util() {}\n").unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Kotlin);
    }
}
