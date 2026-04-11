//! Technology stack identification.

use serde::{Deserialize, Serialize};

/// Technology stack detected from a manifest file.
///
/// See the [glossary](crate#glossary) for what "stack" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum Stack {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Deno,
    DotNet,
    Ruby,
    Java,
    Kotlin,
    Swift,
    Terraform,
    Dockerfile,
    Helm,
    Kustomize,
    Erlang,
}

impl Stack {
    /// Human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Deno => "deno",
            Self::DotNet => "dotnet",
            Self::Ruby => "ruby",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Terraform => "terraform",
            Self::Dockerfile => "dockerfile",
            Self::Helm => "helm",
            Self::Kustomize => "kustomize",
            Self::Erlang => "erlang",
        }
    }
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl std::str::FromStr for Stack {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rust" => Ok(Self::Rust),
            "javascript" => Ok(Self::JavaScript),
            "typescript" => Ok(Self::TypeScript),
            "python" => Ok(Self::Python),
            "go" => Ok(Self::Go),
            "deno" => Ok(Self::Deno),
            "dotnet" => Ok(Self::DotNet),
            "ruby" => Ok(Self::Ruby),
            "java" => Ok(Self::Java),
            "kotlin" => Ok(Self::Kotlin),
            "swift" => Ok(Self::Swift),
            "terraform" => Ok(Self::Terraform),
            "dockerfile" => Ok(Self::Dockerfile),
            "helm" => Ok(Self::Helm),
            "kustomize" => Ok(Self::Kustomize),
            "erlang" => Ok(Self::Erlang),
            _ => Err(format!("unknown stack: \"{s}\"")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_label() {
        assert_eq!(format!("{}", Stack::Rust), "rust");
        assert_eq!(format!("{}", Stack::JavaScript), "javascript");
        assert_eq!(format!("{}", Stack::DotNet), "dotnet");
    }

    #[test]
    fn from_str_roundtrip() {
        for stack in [
            Stack::Rust,
            Stack::JavaScript,
            Stack::TypeScript,
            Stack::Python,
            Stack::Go,
            Stack::Deno,
            Stack::DotNet,
            Stack::Ruby,
            Stack::Java,
            Stack::Kotlin,
            Stack::Swift,
            Stack::Terraform,
            Stack::Dockerfile,
            Stack::Helm,
            Stack::Kustomize,
            Stack::Erlang,
        ] {
            let label = stack.label();
            let parsed: Stack = label.parse().unwrap();
            assert_eq!(parsed, stack);
        }
    }

    #[test]
    fn from_str_unknown() {
        let result: Result<Stack, _> = "cobol".parse();
        assert!(result.is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let json = serde_json::to_string(&Stack::JavaScript).unwrap();
        assert_eq!(json, "\"javascript\"");
        let parsed: Stack = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Stack::JavaScript);
    }
}
