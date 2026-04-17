//! Shared types for npm-based manifests (`package.json`).
//!
//! Used by both the JavaScript and TypeScript detectors.
//! The manifest format is the same — the detectors differ in
//! whether `tsconfig.json` or a `typescript` dependency is required.

use o8v_core::project::ProjectKind;
use serde::Deserialize;
use std::collections::HashMap;

/// Parsed `package.json` manifest.
#[derive(Deserialize)]
pub struct PackageJson {
    pub name: Option<String>,
    pub version: Option<String>,
    pub workspaces: Option<Workspaces>,
    /// Runtime dependencies. Values are version specifiers or URLs — we only
    /// care about the keys, so `serde_json::Value` avoids parse failures on
    /// unusual (but valid) specifier formats like git URLs or workspace refs.
    #[serde(default)]
    pub dependencies: HashMap<String, serde_json::Value>,
    /// Development dependencies.
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: HashMap<String, serde_json::Value>,
}

impl PackageJson {
    /// Returns true if `typescript` appears in `dependencies` or `devDependencies`.
    ///
    /// Used to detect TypeScript projects that do not have a root-level
    /// `tsconfig.json` — common in monorepos and build systems (e.g. the
    /// TypeScript compiler itself stores tsconfig in `src/`, not the root).
    pub fn has_typescript_dep(&self) -> bool {
        self.dependencies.contains_key("typescript")
            || self.dev_dependencies.contains_key("typescript")
    }
}

/// npm uses an array, yarn uses an object with `packages` key.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Workspaces {
    /// npm/yarn array style: `"workspaces": ["packages/*"]`
    Array(Vec<String>),
    /// Yarn object style: `"workspaces": { "packages": ["packages/*"], "nohoist": [...] }`
    Object(WorkspacesObject),
}

#[derive(Deserialize)]
pub struct WorkspacesObject {
    pub packages: Option<Vec<String>>,
}

impl Workspaces {
    /// Convert to `ProjectKind`.
    pub fn into_kind(self) -> ProjectKind {
        match self {
            Self::Array(members) => {
                // Empty workspaces array is not a compound project.
                if members.is_empty() {
                    ProjectKind::Standalone
                } else {
                    ProjectKind::Compound { members }
                }
            }
            Self::Object(obj) => obj.packages.map_or(ProjectKind::Standalone, |members| {
                // Empty packages array is not a compound project.
                if members.is_empty() {
                    ProjectKind::Standalone
                } else {
                    ProjectKind::Compound { members }
                }
            }),
        }
    }
}
