//! Guarded file — a file read through all safety guards.

use std::path::{Path, PathBuf};

/// A file read through all safety guards.
///
/// Contains the path and content. Content has BOM stripped if configured.
/// Consumers parse `content()` themselves — parsing policy belongs in
/// detectors, not in the filesystem crate.
#[derive(Debug)]
pub struct GuardedFile {
    path: PathBuf,
    content: String,
}

impl GuardedFile {
    /// Create a new GuardedFile (crate-internal).
    pub(crate) fn new(path: PathBuf, content: String) -> Self {
        Self { path, content }
    }

    #[must_use]
    /// Path to the file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    /// Raw file content (BOM-stripped if configured).
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Truncate parse error messages to avoid leaking file content.
///
/// Serde errors can include snippets of the parsed file — if the file is
/// symlinked to something sensitive, those snippets should not propagate.
pub fn truncate_error(error: &str, hint: &str) -> String {
    const MAX_ERROR_LEN: usize = 200;
    if error.len() <= MAX_ERROR_LEN {
        format!("{error} — {hint}")
    } else {
        format!(
            "{}... (truncated) — {hint}",
            &error[..error.floor_char_boundary(MAX_ERROR_LEN)]
        )
    }
}
