//! Deno project detection from `deno.json` / `deno.jsonc`.
//!
//! ## Why
//!
//! Deno is a separate stack from JavaScript/TypeScript because it has its
//! own manifest (`deno.json`), its own module system (URL imports),
//! its own permissions model, and its own toolchain.
//!
//! `deno.json` is preferred over `deno.jsonc`. Both support JSONC syntax
//! (comments and trailing commas) — `deno init` generates `deno.json` with
//! comments. Detection strips line/block comments before parsing either file.
//!
//! ## Coexistence
//!
//! A directory can have both `deno.json` and `package.json`. Both detectors
//! will fire — the project is reported as both Deno and JavaScript (or TypeScript).
//! This is correct: the user genuinely has two ecosystems in the same directory.
//!
//! ## Known limitations
//!
//! - Deno workspaces (`"workspace"` field) are detected but the format
//!   is still evolving in Deno.

use super::Detect;
use o8v_core::project::ProjectRoot;
use o8v_core::project::{DetectError, Project, ProjectKind, Stack};
use o8v_fs::{DirScan, FileSystem, GuardedFile};
use serde::Deserialize;

pub struct Deno;

// ─── Manifest types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DenoJson {
    name: Option<String>,
    version: Option<String>,
    workspace: Option<DenoWorkspace>,
    // Project-indicating fields — if ALL are absent, this is a config-only file.
    tasks: Option<serde_json::Value>,
    imports: Option<serde_json::Value>,
    #[serde(rename = "importMap")]
    import_map: Option<String>,
    lock: Option<serde_json::Value>,
    exports: Option<serde_json::Value>,
}

impl DenoJson {
    /// True if this file has at least one field that indicates a project
    /// (not just tooling config like compilerOptions/lint/fmt).
    fn is_project(&self) -> bool {
        self.name.is_some()
            || self.tasks.is_some()
            || self.imports.is_some()
            || self.import_map.is_some()
            || self.exports.is_some()
            || self.lock.is_some()
            || self.workspace.is_some()
    }
}

/// Deno workspace can be an array of member paths or an object with `members`.
#[derive(Deserialize)]
#[serde(untagged)]
enum DenoWorkspace {
    /// Simple array: `"workspace": ["./packages/a"]`
    Array(Vec<String>),
    /// Object with members: `"workspace": { "members": ["./packages/a"] }`
    Object { members: Vec<String> },
}

// ─── Detection ─────────────────────────────────────────────────────────────

impl Detect for Deno {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError> {
        // Prefer deno.json over deno.jsonc
        let file = match fs.read_checked(scan, "deno.json")? {
            Some(m) => m,
            None => match fs.read_checked(scan, "deno.jsonc")? {
                Some(m) => m,
                None => return Ok(None),
            },
        };

        // Both deno.json and deno.jsonc support JSONC syntax (comments, trailing
        // commas). `deno init` generates deno.json with comments, so we must
        // strip them for both extensions — not just .jsonc.
        let manifest: DenoJson = parse_jsonc(&file)?;

        // Config-only deno.json (only compilerOptions/lint/fmt) is not a project.
        // Return None to avoid false-positive detection and unnecessary deno check runs.
        if !manifest.is_project() {
            return Ok(None);
        }

        let name = manifest
            .name
            .unwrap_or_else(|| fs.dir_name().unwrap_or("unknown").trim().to_string());

        let version = manifest.version;

        let kind = match manifest.workspace {
            Some(DenoWorkspace::Array(members) | DenoWorkspace::Object { members }) => {
                // Empty workspace arrays should not create Compound projects
                if members.is_empty() {
                    ProjectKind::Standalone
                } else {
                    ProjectKind::Compound { members }
                }
            }
            None => ProjectKind::Standalone,
        };

        Project::new(root.clone(), name, version, Stack::Deno, kind)
            .map(Some)
            .map_err(|e| DetectError::ManifestInvalid {
                path: file.path().to_path_buf(),
                cause: e.to_string(),
            })
    }
}

fn parse_jsonc<T: serde::de::DeserializeOwned>(file: &GuardedFile) -> Result<T, DetectError> {
    let stripped =
        strip_jsonc_comments(file.content()).map_err(|e| DetectError::ManifestInvalid {
            path: file.path().to_path_buf(),
            cause: e.to_string(),
        })?;
    // JSONC also supports trailing commas — remove them before JSON parsing
    let cleaned = strip_trailing_commas(&stripped);
    serde_json::from_str(&cleaned).map_err(|e| DetectError::ManifestInvalid {
        path: file.path().to_path_buf(),
        cause: e.to_string(),
    })
}

fn strip_jsonc_comments(input: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaping = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev = '\0';
                    let mut closed = false;
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                        }
                        if prev == '*' && next == '/' {
                            closed = true;
                            break;
                        }
                        prev = next;
                    }
                    if !closed {
                        return Err("unterminated block comment /* without closing */");
                    }
                    continue;
                }
                _ => {}
            }
        }

        output.push(ch);
    }

    Ok(output)
}

/// Remove trailing commas before `]` and `}` — JSONC allows them, JSON does not.
fn strip_trailing_commas(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    let mut in_string = false;
    let mut escaping = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == ',' {
            // Look ahead past whitespace for ] or } — without allocating.
            // Previous version collected remaining chars into a String per comma (O(n²)).
            let mut lookahead = chars.clone();
            let is_trailing = loop {
                match lookahead.next() {
                    Some(c) if c.is_ascii_whitespace() => {}
                    Some(']' | '}') => break true,
                    _ => break false,
                }
            };
            if is_trailing {
                continue;
            }
        }

        output.push(ch);
    }

    output
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::project::ProjectRoot;
    use std::path::Path;

    fn detect_in(dir: &Path) -> Result<Option<Project>, DetectError> {
        let root = ProjectRoot::new(dir).unwrap();
        let fs = o8v_fs::SafeFs::new(dir, o8v_fs::FsConfig::default()).unwrap();
        let scan = fs.scan().unwrap();
        Deno.detect(&fs, &scan, &root)
    }

    #[test]
    fn detects_deno_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{"name": "@scope/myapp", "version": "1.0.0"}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "@scope/myapp");
        assert_eq!(project.version(), Some("1.0.0"));
        assert_eq!(project.stack(), Stack::Deno);
        assert!(matches!(project.kind(), ProjectKind::Standalone));
    }

    #[test]
    fn detects_deno_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  // fixture comment
  "name": "myapp",
  "version": "2.0.0"
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "myapp");
        assert_eq!(project.stack(), Stack::Deno);
    }

    #[test]
    fn jsonc_comment_stripping_preserves_urls() {
        let stripped = strip_jsonc_comments(
            r#"{
  "imports": {
    "std/": "https://deno.land/std@0.224.0/"
  }
}"#,
        );
        let stripped = stripped.unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(
            manifest["imports"]["std/"],
            "https://deno.land/std@0.224.0/"
        );
    }

    #[test]
    fn deno_json_preferred_over_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deno.json"), r#"{"name": "from-json"}"#).unwrap();
        std::fs::write(dir.path().join("deno.jsonc"), r#"{"name": "from-jsonc"}"#).unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(
            project.name(),
            "from-json",
            "deno.json should take priority"
        );
    }

    #[test]
    fn detects_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{"name": "mono", "workspace": ["./packages/a", "./packages/b"]}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["./packages/a", "./packages/b"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn no_name_falls_back_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        // Must have a project-indicating field (tasks) to be detected.
        // version alone is not enough.
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{"version": "1.0.0", "tasks": {"start": "deno run main.ts"}}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        let expected = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(
            project.name(),
            expected,
            "should use directory name when name absent"
        );
    }

    #[test]
    fn no_deno_json() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_in(dir.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deno.json"), "not json!!!").unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn name_wrong_type() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deno.json"), r#"{"name": 42}"#).unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn detects_workspace_object_format() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{"name": "mono", "workspace": {"members": ["./packages/a", "./packages/b"]}}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["./packages/a", "./packages/b"]);
        } else {
            panic!("expected workspace from object format");
        }
    }

    #[test]
    fn empty_deno_json_is_config_only() {
        // An empty deno.json {} is a config-only file (compilerOptions, lint, fmt).
        // It should NOT be detected as a project.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deno.json"), "{}").unwrap();

        let result = detect_in(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "empty deno.json should not be detected as a project"
        );
    }

    #[test]
    fn config_only_compiler_options_is_not_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{"compilerOptions": {"strict": true}, "lint": {"rules": {"tags": ["recommended"]}}}"#,
        )
        .unwrap();
        let result = detect_in(dir.path()).unwrap();
        assert!(
            result.is_none(),
            "compilerOptions-only deno.json should not be a project"
        );
    }

    #[test]
    fn exports_only_is_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("deno.json"), r#"{"exports": "./mod.ts"}"#).unwrap();
        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.stack(), Stack::Deno);
    }

    #[test]
    fn deno_json_with_comments() {
        // deno init generates deno.json with JSONC comments — not just .jsonc
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{
  // Generated by deno init
  "name": "fresh-app",
  "version": "1.0.0",
  "tasks": {
    "dev": "deno run -A --watch main.ts"
  }
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "fresh-app");
        assert_eq!(project.version(), Some("1.0.0"));
    }

    #[test]
    fn deno_json_with_block_comment_and_trailing_comma() {
        // deno.json supports full JSONC: block comments + trailing commas
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            r#"{
  /* App config */
  "name": "app",
  "version": "2.0.0",
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "app");
        assert_eq!(project.version(), Some("2.0.0"));
    }

    #[test]
    fn jsonc_block_comment() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  /* block comment */
  "name": "blocked",
  "version": "1.0.0"
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "blocked");
    }

    #[test]
    fn jsonc_unterminated_block_comment_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  /* unterminated
  "name": "bad"
}"#,
        )
        .unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn jsonc_trailing_commas() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  "name": "app",
  "version": "1.0.0",
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "app");
        assert_eq!(project.version(), Some("1.0.0"));
    }

    #[test]
    fn jsonc_trailing_comma_in_array() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  "name": "mono",
  "workspace": ["./a", "./b",]
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        if let ProjectKind::Compound { members } = project.kind() {
            assert_eq!(members, &["./a", "./b"]);
        } else {
            panic!("expected workspace");
        }
    }

    #[test]
    fn control_chars_in_deno_name_is_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.json"),
            "{\"name\": \"bad\\u001bname\", \"version\": \"1.0.0\"}",
        )
        .unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn invalid_jsonc_after_comment_strip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  // this is valid jsonc but invalid json after strip
  "name": not_a_string
}"#,
        )
        .unwrap();

        assert!(detect_in(dir.path()).is_err());
    }

    #[test]
    fn jsonc_escaped_quote_in_string() {
        // Tests escape handling: backslash inside JSON strings must not break comment detection
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("deno.jsonc"),
            r#"{
  "name": "has\"escaped\"quotes",
  "version": "1.0.0"
}"#,
        )
        .unwrap();

        let project = detect_in(dir.path()).unwrap().unwrap();
        assert_eq!(project.name(), "has\"escaped\"quotes");
    }

    #[test]
    fn jsonc_lone_slash_not_comment() {
        // A lone `/` outside a string, not followed by `/` or `*`, should be preserved
        let result = strip_jsonc_comments("{ / }").unwrap();
        assert_eq!(result, "{ / }");
    }

    #[test]
    fn trailing_comma_escaped_quote_in_string() {
        // Escape handling in strip_trailing_commas: comma after escaped quote in string
        let input = r#"{"key": "val\"ue",}"#;
        let result = strip_trailing_commas(input);
        // The trailing comma before } should be removed
        assert_eq!(result, r#"{"key": "val\"ue"}"#);
    }
}
