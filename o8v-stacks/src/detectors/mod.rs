//! Stack detectors and detection types.
//!
//! Each stack (Rust, TypeScript, etc.) implements [`Detect`]. Adding a stack
//! means adding a file and registering it in [`detectors()`].

mod deno;
mod dockerfile;
mod dotnet;
mod erlang;
mod go;
mod helm;
mod java;
mod javascript;
mod kotlin;
mod kustomize;
pub mod npm;
mod python;
mod ruby;
mod rust;
mod swift;
mod terraform;
mod typescript;

use o8v_core::project::{DetectError, Project, ProjectRoot};
use o8v_fs::{DirScan, FileSystem};

/// Detect a project from a pre-scanned directory.
///
/// Each stack (Rust, TypeScript, etc.) implements this trait.
/// Returns `Ok(None)` if this stack is not present.
/// Returns `Err` if the manifest exists but is invalid.
///
/// `fs` provides guarded file reads. `scan` is the pre-built directory index.
/// `root` is the validated project path for constructing `Project` instances.
///
/// See the [detection contract](crate#detection-contract) for details.
pub trait Detect {
    fn detect(
        &self,
        fs: &dyn FileSystem,
        scan: &DirScan,
        root: &ProjectRoot,
    ) -> Result<Option<Project>, DetectError>;
}

/// Result of running all detectors on a directory.
///
/// Contains both successful detections and errors. A single `detect_all` call
/// can find multiple projects (e.g. Rust + TypeScript in the same directory)
/// and also report errors (e.g. invalid `pyproject.toml` alongside valid `Cargo.toml`).
#[derive(Debug)]
pub struct DetectResult {
    pub(crate) projects: Vec<Project>,
    pub(crate) errors: Vec<DetectError>,
}

impl DetectResult {
    /// Detected projects.
    #[must_use]
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Errors encountered during detection.
    #[must_use]
    pub fn errors(&self) -> &[DetectError] {
        &self.errors
    }

    /// True if no errors occurred.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// True if no projects were found and no errors occurred.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.projects.is_empty() && self.errors.is_empty()
    }

    /// Consume the result and return its owned parts.
    ///
    /// When you need to move both projects and errors out of the result,
    /// use this instead of accessing fields directly.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Project>, Vec<DetectError>) {
        (self.projects, self.errors)
    }
}

/// All built-in detectors — zero allocation.
/// Order matters: TypeScript runs before JavaScript so tsconfig.json
/// projects are claimed by TypeScript, not JavaScript.
pub fn detectors() -> [&'static dyn Detect; 16] {
    [
        &rust::Rust,
        &typescript::TypeScript,
        &javascript::JavaScript, // after TypeScript — skips when tsconfig exists
        &python::Python,
        &go::Go,
        &deno::Deno,
        &dotnet::DotNet,
        &ruby::Ruby,
        &java::Java,
        &kotlin::Kotlin,
        &swift::Swift,
        &erlang::Erlang,
        &terraform::Terraform,
        &dockerfile::Dockerfile,
        &helm::Helm,
        &kustomize::Kustomize,
    ]
}
