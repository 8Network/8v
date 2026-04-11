//! .NET project detection from `.sln`, `.slnx`, and `.csproj` files.
//!
//! ## Why
//!
//! .NET detection prioritizes `.slnx` > `.sln` > `.csproj`. Solutions
//! (`.slnx`/`.sln`) are workspaces containing project references. When no
//! solution exists, a standalone `.csproj` is detected.
//!
//! `.slnx` is the VS 2022 XML-based solution format — parsed with `quick-xml`.
//! `.sln` is the classic Visual Studio text format — parsed with string splitting.
//! `.csproj` is the `MSBuild` project file — parsed with `quick-xml`.
//!
//! Project name comes from XML elements (`RootNamespace`, `AssemblyName`)
//! or falls back to the filename — matching `MSBuild`'s own convention.
//!
//! ## Known limitations
//!
//! - `.sln` parser assumes standard Visual Studio text format.
//!   Project names containing escaped quotes may not parse correctly.
//! - Only `RootNamespace`, `AssemblyName`, and `Version` are extracted from `.csproj`.
//! - Nested or conditional `PropertyGroups` are not distinguished.
//! - `.fsproj` and `.vbproj` are not yet detected (same XML schema — addable).

use super::Detect;
use crate::path::ProjectRoot;
use crate::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem, GuardedFile};
use serde::Deserialize;

pub struct DotNet;

/// GUID identifying solution folders in .sln files (not real projects).
const SOLUTION_FOLDER_GUID: &str = "2150E333-8FDC-42A3-9474-1A3956D46DE8";

// ─── .csproj typed model ───────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename = "Project")]
struct CsprojProject {
    #[serde(rename = "PropertyGroup", default)]
    property_groups: Vec<PropertyGroup>,
}

#[derive(Deserialize, Default)]
struct PropertyGroup {
    #[serde(rename = "RootNamespace")]
    root_namespace: Option<String>,
    #[serde(rename = "AssemblyName")]
    assembly_name: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
}

impl CsprojProject {
    fn root_namespace(&self) -> Option<&str> {
        self.property_groups
            .iter()
            .filter_map(|pg| pg.root_namespace.as_deref())
            .find(|s| !s.is_empty())
    }

    fn assembly_name(&self) -> Option<&str> {
        self.property_groups
            .iter()
            .filter_map(|pg| pg.assembly_name.as_deref())
            .find(|s| !s.is_empty())
    }

    fn version(&self) -> Option<&str> {
        self.property_groups
            .iter()
            .filter_map(|pg| pg.version.as_deref())
            .find(|s| !s.is_empty())
    }
}

// ─── .slnx typed model (VS 2022 17.10+ XML solution format) ───────────────

#[derive(Deserialize)]
#[serde(rename = "Solution")]
struct SlnxSolution {
    #[serde(rename = "Project", default)]
    projects: Vec<SlnxProject>,
    #[serde(rename = "Folder", default)]
    folders: Vec<SlnxFolder>,
}

#[derive(Deserialize)]
struct SlnxProject {
    #[serde(rename = "@Path")]
    path: String,
}

/// VS 2022 solution folders — contain nested `<Project>` and `<Folder>` elements.
#[derive(Deserialize)]
struct SlnxFolder {
    #[serde(rename = "Project", default)]
    projects: Vec<SlnxProject>,
    #[serde(rename = "Folder", default)]
    folders: Vec<SlnxFolder>,
}

impl SlnxSolution {
    /// Collect all projects, including those nested inside `<Folder>` containers.
    /// VS 2022 organizes projects into folders — top-level-only parsing misses them.
    fn all_projects(&self) -> Result<Vec<&SlnxProject>, String> {
        let mut result = Vec::new();
        for p in &self.projects {
            result.push(p);
        }
        for f in &self.folders {
            collect_folder_projects(f, &mut result, 0)?;
        }
        Ok(result)
    }

    /// Extract project names from paths (file stem of each project path).
    /// Handles both `/` and `\` path separators since .slnx uses Windows-style paths.
    /// Returns an error if any project path is malformed (empty or no file stem).
    fn member_names(&self) -> Result<Vec<String>, String> {
        let all = self.all_projects()?;
        let mut names = Vec::with_capacity(all.len());
        for p in all {
            // .slnx paths use backslashes (Windows): "src\App\App.csproj"
            // Split on both separators to get the filename on any OS
            let filename = p.path.rsplit(['/', '\\']).next().filter(|s| !s.is_empty());
            let name = filename.and_then(|f| {
                std::path::Path::new(f)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            });
            match name {
                Some(n) => names.push(n),
                None => {
                    return Err(format!(
                        "malformed project path in .slnx: {:?} — cannot extract project name",
                        p.path
                    ));
                }
            }
        }
        Ok(names)
    }
}

fn collect_folder_projects<'a>(
    folder: &'a SlnxFolder,
    result: &mut Vec<&'a SlnxProject>,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err(
            "folder nesting exceeds maximum depth of 64 — possible malformed or malicious .slnx"
                .to_string(),
        );
    }
    for p in &folder.projects {
        result.push(p);
    }
    for f in &folder.folders {
        collect_folder_projects(f, result, depth + 1)?;
    }
    Ok(())
}

// ─── Detection ─────────────────────────────────────────────────────────────

impl Detect for DotNet {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Priority: .slnx > .sln > .csproj
        // All reads go through fs.read_by_ext — same guards as every other detector.

        // 1. .slnx (XML solution format)
        if let Some(file) = fs.read_by_ext(scan, "slnx")? {
            let solution: SlnxSolution = parse_xml(&file)?;
            // Scan guarantees all indexed filenames are valid UTF-8,
            // and files with extensions always have a stem.
            let name = file
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("scan guarantees UTF-8 filenames")
                .to_string();

            let members =
                solution
                    .member_names()
                    .map_err(|cause| DetectError::ManifestInvalid {
                        path: file.path().to_path_buf(),
                        cause,
                    })?;
            // If no real projects found (only folders), don't detect as a workspace.
            if members.is_empty() {
                return Ok(None);
            }
            return Project::new(
                root.clone(),
                name,
                None,
                Stack::DotNet,
                ProjectKind::Compound { members },
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: e.to_string(),
            });
        }

        // 2. .sln (classic text format)
        if let Some(file) = fs.read_by_ext(scan, "sln")? {
            let name = file
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("scan guarantees UTF-8 filenames")
                .to_string();

            let members = parse_sln_projects(file.content(), file.path())?;
            // If no real projects found (only solution folders), don't detect as a workspace.
            if members.is_empty() {
                return Ok(None);
            }
            return Project::new(
                root.clone(),
                name,
                None,
                Stack::DotNet,
                ProjectKind::Compound { members },
            )
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: e.to_string(),
            });
        }

        // 3. .csproj
        let Some(file) = fs.read_by_ext(scan, "csproj")? else {
            return Ok(None);
        };

        let project_xml: CsprojProject = parse_xml(&file)?;

        // Name priority: RootNamespace > AssemblyName > filename stem.
        // The filename fallback always succeeds — scan guarantees UTF-8 filenames.
        let name = project_xml
            .root_namespace()
            .or_else(|| project_xml.assembly_name())
            .or_else(|| file.path().file_stem().and_then(|s| s.to_str()))
            .expect("scan guarantees UTF-8 filenames — file_stem always available")
            .trim()
            .to_string();

        let version = project_xml.version().map(|s| s.trim().to_string());

        Project::new(
            root.clone(),
            name,
            version,
            Stack::DotNet,
            ProjectKind::Standalone,
        )
        .map(Some)
        .map_err(|e| DetectError::ManifestInvalid {
            path: file.path().to_path_buf(),
            cause: e.to_string(),
        })
    }
}

/// Parse XML content from a GuardedFile. XML is .NET-specific, not in o8v-fs.
fn parse_xml<T: serde::de::DeserializeOwned>(file: &GuardedFile) -> Result<T, DetectError> {
    quick_xml::de::from_str(file.content()).map_err(|e| DetectError::ManifestInvalid {
        path: file.path().to_path_buf(),
        cause: crate::truncate_error(&format!("{e}"), "check the format of your XML project file"),
    })
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Parse project names from a classic `.sln` file.
///
/// Iterates lines looking for `Project(...)` declarations.
/// Skips solution folders (identified by well-known GUID).
/// Returns an error if a Project line exists but can't be parsed.
fn parse_sln_projects(
    content: &str,
    sln_path: &std::path::Path,
) -> Result<Vec<String>, DetectError> {
    let mut names = Vec::new();
    for line in content.lines() {
        if !is_project_line(line) {
            continue;
        }
        if is_solution_folder(line) {
            continue;
        }
        let name = extract_project_name(line, sln_path)?;
        names.push(name);
    }
    Ok(names)
}

/// True if this line declares a project in the .sln format.
fn is_project_line(line: &str) -> bool {
    line.starts_with("Project(")
}

/// True if this project line is a solution folder (not a real project).
/// Solution folders use the well-known GUID `2150E333-...`.
fn is_solution_folder(line: &str) -> bool {
    line.to_ascii_uppercase().contains(SOLUTION_FOLDER_GUID)
}

/// Extract the project name from a `.sln` Project line.
///
/// Format: `Project("{TypeGUID}") = "Name", "Path", "{ProjectGUID}"`
/// Splits on first `=`, then extracts the first quoted string.
fn extract_project_name(line: &str, sln_path: &std::path::Path) -> Result<String, DetectError> {
    // Truncate line in error messages to prevent log amplification from malicious .sln files
    let display_line = if line.len() > 200 {
        format!("{}... (truncated)", &line[..line.floor_char_boundary(200)])
    } else {
        line.to_string()
    };

    let (_, after_eq) = line
        .split_once('=')
        .ok_or_else(|| DetectError::ManifestInvalid {
            path: sln_path.to_path_buf(),
            cause: format!("malformed Project line in .sln (no '='): {display_line}"),
        })?;
    let name = after_eq
        .split('"')
        .nth(1)
        .ok_or_else(|| DetectError::ManifestInvalid {
            path: sln_path.to_path_buf(),
            cause: format!("malformed Project line in .sln (no quoted name): {display_line}"),
        })?;
    Ok(name.to_string())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        DotNet.detect(&fs, &scan, &root)
    }

    #[test]
    fn csproj_standalone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("MyApp.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><RootNamespace>MyApp</RootNamespace><Version>3.0.0</Version></PropertyGroup></Project>",
        ).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "MyApp");
        assert_eq!(project.version(), Some("3.0.0"));
        assert_eq!(project.stack(), Stack::DotNet);
    }

    #[test]
    fn csproj_falls_back_to_filename() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("MyApp.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>",
        ).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "MyApp");
    }

    #[test]
    fn csproj_invalid_xml_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Bad.csproj"), "this is not xml at all!!!").unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn sln_extracts_projects() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Test.sln"),
            r#"
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "App", "src\App\App.csproj", "{A1}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "App.Tests", "tests\App.Tests.csproj", "{A2}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Solution Items", "Solution Items", "{A3}"
EndProject
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "Test");
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["App", "App.Tests"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn sln_name_with_equals() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Test.sln"),
            r#"
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "A=B", "src\AB\AB.csproj", "{GUID}"
EndProject
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["A=B"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn slnx_extracts_projects() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("MyApp.slnx"),
            r#"
<Solution>
  <Project Path="src\App\App.csproj" />
  <Project Path="tests\App.Tests\App.Tests.csproj" />
  <Folder Name="docs">
    <File Path="README.md" />
  </Folder>
</Solution>
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "MyApp");
        assert_eq!(project.stack(), Stack::DotNet);
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["App", "App.Tests"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn slnx_takes_priority_over_sln() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("MyApp.slnx"),
            r#"
<Solution>
  <Project Path="src\App\App.csproj" />
</Solution>
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("MyApp.sln"),
            r#"
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "OldApp", "src\OldApp\OldApp.csproj", "{A1}"
EndProject
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        // .slnx wins — member is "App" not "OldApp"
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["App"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn slnx_malformed_project_path_is_error() {
        // A project path with no file stem (e.g. just a separator or empty)
        // must produce an error — not silently drop the entry.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Bad.slnx"),
            r#"
<Solution>
  <Project Path="src\App\App.csproj" />
  <Project Path="\" />
</Solution>
"#,
        )
        .unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("malformed project path"),
            "expected malformed path error, got: {msg}"
        );
    }

    #[test]
    fn slnx_folder_nested_projects() {
        // Real VS 2022 solutions organize projects into Folder containers.
        // Projects nested inside Folder must be discovered — not just top-level.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Polly.slnx"),
            r#"
<Solution>
  <Folder Name="/src/">
    <Project Path="src\Polly.Core\Polly.Core.csproj" />
    <Project Path="src\Polly\Polly.csproj" />
  </Folder>
  <Folder Name="/test/">
    <Project Path="test\Polly.Core.Tests\Polly.Core.Tests.csproj" />
  </Folder>
  <Folder Name="/eng/">
    <Folder Name="/eng/build/">
      <Project Path="eng\build\Build.csproj" />
    </Folder>
  </Folder>
</Solution>
"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "Polly");
        assert_eq!(project.stack(), Stack::DotNet);
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members.len(), 4, "should find all 4 projects: {members:?}");
            assert!(members.contains(&"Polly.Core".to_string()));
            assert!(members.contains(&"Polly".to_string()));
            assert!(members.contains(&"Polly.Core.Tests".to_string()));
            assert!(members.contains(&"Build".to_string()));
        } else {
            panic!("expected workspace, got {:?}", project.kind());
        }
    }

    #[test]
    fn slnx_invalid_xml_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Bad.slnx"), "not xml!!!").unwrap();

        assert!(matches!(
            detect_in(dir.path()),
            Err(DetectError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn no_dotnet_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn sln_with_crlf_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Test.sln"),
            "Microsoft Visual Studio Solution File\r\nProject(\"{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}\") = \"App\", \"src\\App\\App.csproj\", \"{A1}\"\r\nEndProject\r\n",
        ).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["App"], "CRLF line endings should be handled");
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn sln_malformed_no_equals() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Bad.sln"),
            "Project(\"{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}\") BROKEN LINE\nEndProject\n",
        )
        .unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        assert!(format!("{err}").contains("malformed") || format!("{err}").contains("no '='"));
    }

    #[test]
    fn sln_malformed_no_quoted_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Bad.sln"),
            "Project(\"{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}\") = no quotes here\nEndProject\n",
        )
        .unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        assert!(
            format!("{err}").contains("malformed") || format!("{err}").contains("no quoted name")
        );
    }

    #[test]
    fn sln_long_line_truncated_in_error() {
        let dir = tempfile::tempdir().unwrap();
        let long_line = format!(
            "Project(\"{{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}}\") BROKEN {}\n",
            "x".repeat(300)
        );
        std::fs::write(dir.path().join("Bad.sln"), &long_line).unwrap();

        let err = detect_in(dir.path()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("truncated"),
            "long malformed line should be truncated in error: {msg}"
        );
    }

    #[test]
    fn sln_with_only_solution_folders_returns_none() {
        // Bug regression test: a .sln with only solution folders (not real projects)
        // should return None (no project detected), not a zero-member workspace.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Test.sln"),
            r#"
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Solution Items", "Solution Items", "{A1}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Docs", "Docs", "{A2}"
EndProject
"#,
        )
        .unwrap();

        let result = detect_in(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "sln with only solution folders should return None, not a zero-member workspace"
        );
    }

    #[test]
    fn slnx_with_only_folders_returns_none() {
        // Bug regression test: a .slnx with only folders (no projects)
        // should return None (no project detected), not a zero-member workspace.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Empty.slnx"),
            r#"
<Solution>
  <Folder Name="docs">
    <File Path="README.md" />
  </Folder>
</Solution>
"#,
        )
        .unwrap();

        let result = detect_in(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "slnx with only folders should return None, not a zero-member workspace"
        );
    }
}
