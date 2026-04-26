//! FileSystem trait — the integration seam for detectors and tests.

use crate::error::FsError;
use crate::file::GuardedFile;
use crate::scan::{DirEntry, DirScan};
use std::path::Path;

/// Trait for filesystem abstraction.
///
/// Includes both primitive operations (`read_file`, `scan`) and composite
/// operations (`read_checked`, `read_by_ext`, `validate_entry`) that encode
/// the guard + type-check + read pipeline.
///
/// Real implementation: [`SafeFs`](crate::SafeFs). Test implementation: mock.
pub trait FileSystem {
    /// The containment root.
    fn root(&self) -> &Path;

    /// The directory name of the root (for fallback naming).
    /// Trimmed — trailing whitespace does not propagate (bug #254).
    fn dir_name(&self) -> Option<&str>;

    /// Scan the root directory once. Returns entries + errors.
    fn scan(&self) -> Result<DirScan, FsError>;

    /// Read a file by name from a pre-built scan. Full guard pipeline.
    /// Returns `Ok(None)` if the name doesn't exist.
    /// Returns `Err` if the name exists but is not a regular file, or read fails.
    fn read_checked(&self, scan: &DirScan, name: &str) -> Result<Option<GuardedFile>, FsError>;

    /// Read a file by extension. Returns the unique file with that extension.
    /// If multiple files match, returns `Err(Ambiguous)`.
    fn read_by_ext(&self, scan: &DirScan, ext: &str) -> Result<Option<GuardedFile>, FsError>;

    /// Validate that a name exists and is a regular file, without reading content.
    /// For symlinks: resolves and checks containment before returning.
    fn validate_entry<'s>(
        &self,
        scan: &'s DirScan,
        name: &str,
    ) -> Result<Option<&'s DirEntry>, FsError>;

    /// Low-level: read a file by path with all guards.
    fn read_file(&self, path: &Path) -> Result<GuardedFile, FsError>;
}
