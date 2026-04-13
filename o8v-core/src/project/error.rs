//! Error types for project detection.
//!
//! Three error types for three phases:
//!
//! - [`PathError`]: The directory path itself is invalid (doesn't exist,
//!   not a directory, can't canonicalize). Happens **before** detection starts.
//! - [`DetectError`]: Something went wrong during detection (can't list directory,
//!   can't read manifest, manifest is invalid). Happens **during** detection.
//! - [`ProjectError`]: The extracted data violates project invariants (empty name,
//!   whitespace in name, empty workspace). Happens **when constructing** the result.

use std::path::PathBuf;

/// Error when creating a [`crate::ProjectRoot`].
///
/// These errors mean the input path cannot be used for detection at all.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PathError {
    /// The path does not exist on disk.
    #[error("path not found: {path}")]
    NotFound { path: PathBuf },

    /// The path exists but is not a directory.
    #[error("not a directory: {path}")]
    NotDirectory { path: PathBuf },

    /// Cannot resolve relative path to absolute.
    #[error("cannot resolve absolute path for {path}: {cause}")]
    CannotResolve {
        path: PathBuf,
        #[source]
        cause: std::io::Error,
    },

    /// The directory name is not valid UTF-8 or the path is a filesystem root.
    /// Project directories must have a UTF-8 name component for fallback naming.
    #[error("directory has no valid UTF-8 name: {path}")]
    InvalidDirectoryName { path: PathBuf },
}

/// Error during stack detection.
///
/// These errors are collected in [`crate::DetectResult`] alongside successful
/// detections. One detector's error does not prevent other detectors from running.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DetectError {
    /// The project directory itself could not be listed.
    /// This prevents all detection — no detector can run.
    #[error("cannot scan directory {path}: {cause}")]
    DirectoryUnreadable {
        path: PathBuf,
        #[source]
        cause: std::io::Error,
    },

    /// A manifest file exists but could not be read (permissions, I/O error).
    #[error("cannot read {path}: {cause}")]
    ManifestUnreadable {
        path: PathBuf,
        #[source]
        cause: std::io::Error,
    },

    /// A manifest file was read but its content is invalid
    /// (parse error, wrong types, missing required fields).
    #[error("invalid manifest {path}: {cause}")]
    ManifestInvalid { path: PathBuf, cause: String },

    /// Filesystem error from o8v-fs (symlink escape, FIFO, size limit, etc.).
    /// Preserves the full FsError — never collapses PermissionDenied into
    /// ManifestUnreadable (bug #22).
    #[error("{0}")]
    Fs(#[from] o8v_fs::FsError),
}

impl DetectError {
    /// Machine-readable error kind string for structured output.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::DirectoryUnreadable { .. } => "directory_unreadable",
            Self::ManifestUnreadable { .. } => "manifest_unreadable",
            Self::ManifestInvalid { .. } => "manifest_invalid",
            Self::Fs(fs_err) => fs_err.kind(),
        }
    }

    /// The primary path associated with this error.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::DirectoryUnreadable { path, .. }
            | Self::ManifestUnreadable { path, .. }
            | Self::ManifestInvalid { path, .. } => path,
            Self::Fs(fs_err) => fs_err.path(),
        }
    }
}

/// Error when constructing a [`crate::Project`].
///
/// These represent invariant violations in the extracted data —
/// the manifest parsed successfully but the values don't make a valid project.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// Name is empty (zero-length string).
    #[error("project name cannot be empty")]
    EmptyName,

    /// Name contains leading or trailing whitespace.
    #[error("project name has leading or trailing whitespace: \"{0}\"")]
    WhitespaceName(String),

    /// Compound project declared with no members.
    #[error("{0} compound project has no members")]
    EmptyCompound(super::stack::Stack),

    /// Name or member contains control characters (terminal/log injection risk).
    #[error("contains control characters: {0:?}")]
    ControlCharacters(String),
}
