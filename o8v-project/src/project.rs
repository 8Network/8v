//! Project — a detected project with validated fields.

use crate::error::ProjectError;
use o8v_core::project::{ProjectRoot, Stack};

/// Whether the project is standalone or a compound project with members.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ProjectKind {
    Standalone,
    Compound { members: Vec<String> },
}

/// A detected project.
///
/// `Serialize` is supported for reporting. `Deserialize` is intentionally omitted —
/// construct via [`Project::new`] to enforce invariants (non-empty name, valid workspace).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Project {
    path: ProjectRoot,
    name: String,
    version: Option<String>,
    stack: Stack,
    kind: ProjectKind,
}

impl Project {
    /// Create a new project. Name must not be empty.
    ///
    /// # Errors
    ///
    /// Returns `ProjectError` if the name is empty, contains leading/trailing
    /// whitespace, contains control characters, or if compound members are
    /// empty or contain control characters.
    pub fn new(
        path: ProjectRoot,
        name: String,
        version: Option<String>,
        stack: Stack,
        kind: ProjectKind,
    ) -> Result<Self, ProjectError> {
        if name.is_empty() {
            return Err(ProjectError::EmptyName);
        }
        if name != name.trim() {
            return Err(ProjectError::WhitespaceName(name));
        }
        if has_control_chars(&name) {
            return Err(ProjectError::ControlCharacters(name));
        }
        if let ProjectKind::Compound { ref members } = kind {
            if members.is_empty() {
                return Err(ProjectError::EmptyCompound(stack));
            }
            if members.iter().any(|m| m.trim().is_empty()) {
                return Err(ProjectError::EmptyCompound(stack));
            }
            if let Some(m) = members.iter().find(|m| has_control_chars(m)) {
                return Err(ProjectError::ControlCharacters(m.clone()));
            }
        }

        // Validate version — same control char policy as name
        if let Some(ref v) = version {
            if has_control_chars(v) {
                return Err(ProjectError::ControlCharacters(v.clone()));
            }
        }

        // Normalize empty version to None
        let version = version.filter(|v| !v.trim().is_empty());

        Ok(Self {
            path,
            name,
            version,
            stack,
            kind,
        })
    }

    /// The validated project root path.
    #[must_use]
    pub const fn path(&self) -> &ProjectRoot {
        &self.path
    }

    /// The project name (e.g. from `Cargo.toml` or `package.json`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The project version, if the manifest specifies one.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The detected technology stack.
    #[must_use]
    pub const fn stack(&self) -> Stack {
        self.stack
    }

    /// Whether this project is standalone or a compound project.
    #[must_use]
    pub const fn kind(&self) -> &ProjectKind {
        &self.kind
    }
}

/// Check for control characters that could cause terminal/log injection.
/// Rejects: ALL C0 controls (0x00-0x1F including tab), DEL (0x7F).
/// Tabs are rejected because they distort CLI tables, TSV logs, and
/// copy-paste output. No manifest format uses tabs in project names.
fn has_control_chars(s: &str) -> bool {
    s.chars().any(|c| {
        let code = c as u32;
        code < 0x20 || code == 0x7F
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let p = Project::new(
            path,
            "my-app".into(),
            Some("1.0.0".into()),
            Stack::Rust,
            ProjectKind::Standalone,
        )
        .unwrap();
        assert_eq!(p.name(), "my-app");
        assert_eq!(p.version(), Some("1.0.0"));
        assert_eq!(p.stack(), Stack::Rust);
    }

    #[test]
    fn empty_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            String::new(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::EmptyName)));
    }

    #[test]
    fn whitespace_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "  my-app  ".into(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::WhitespaceName(_))));
    }

    #[test]
    fn empty_compound_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "ws".into(),
            None,
            Stack::Rust,
            ProjectKind::Compound { members: vec![] },
        );
        assert!(matches!(
            result,
            Err(ProjectError::EmptyCompound(Stack::Rust))
        ));
    }

    #[test]
    fn empty_version_normalized_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let p = Project::new(
            path,
            "app".into(),
            Some(String::new()),
            Stack::Rust,
            ProjectKind::Standalone,
        )
        .unwrap();
        assert_eq!(
            p.version(),
            None,
            "empty version should be normalized to None"
        );
    }

    #[test]
    fn whitespace_version_normalized_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let p = Project::new(
            path,
            "app".into(),
            Some("  ".into()),
            Stack::Rust,
            ProjectKind::Standalone,
        )
        .unwrap();
        assert_eq!(
            p.version(),
            None,
            "whitespace version should be normalized to None"
        );
    }

    #[test]
    fn control_chars_in_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "bad\x1b[31mname".into(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::ControlCharacters(_))));
    }

    #[test]
    fn newline_in_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "bad\nname".into(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::ControlCharacters(_))));
    }

    #[test]
    fn control_chars_in_member_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "ws".into(),
            None,
            Stack::Rust,
            ProjectKind::Compound {
                members: vec!["ok".into(), "bad\x00member".into()],
            },
        );
        assert!(matches!(result, Err(ProjectError::ControlCharacters(_))));
    }

    #[test]
    fn tab_in_name_rejected() {
        // Tabs distort CLI tables, TSV logs, and copy-paste output.
        // No manifest format uses tabs in project names.
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "has\ttab".into(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::ControlCharacters(_))));
    }

    #[test]
    fn control_chars_in_version_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "app".into(),
            Some("1.0.0\x1b[31m".into()),
            Stack::Rust,
            ProjectKind::Standalone,
        );
        assert!(matches!(result, Err(ProjectError::ControlCharacters(_))));
    }

    #[test]
    fn whitespace_only_member_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let result = Project::new(
            path,
            "ws".into(),
            None,
            Stack::Rust,
            ProjectKind::Compound {
                members: vec!["ok".into(), "  ".into()],
            },
        );
        assert!(matches!(
            result,
            Err(ProjectError::EmptyCompound(Stack::Rust))
        ));
    }

    #[test]
    fn path_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        let expected = path.clone();
        let p = Project::new(
            path,
            "app".into(),
            None,
            Stack::Rust,
            ProjectKind::Standalone,
        )
        .unwrap();
        assert_eq!(p.path(), &expected);
    }
}
