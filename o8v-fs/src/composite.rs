//! Composite operations — encode the patterns from 39 AI errors.
//!
//! Every detector used `read_manifest` in the old code — that logic now
//! lives here as `read_checked`, `read_by_ext`, and `validate_entry`.

use crate::config::FsConfig;
use crate::error::FsError;
use crate::file::GuardedFile;
use crate::guard::guarded_read;
use crate::scan::{DirEntry, DirScan};
use std::path::Path;

/// Read a file by name from a scan. Full guard pipeline.
///
/// 1. Lookup by name in scan → Ok(None) if not found
/// 2. Check is_file → Err(NotRegularFile) if directory/device/etc
/// 3. guarded_read with all guards
///
/// Encodes fixes for bugs #64, #74, #122, #127, #131.
pub(crate) fn read_checked(
    scan: &DirScan,
    name: &str,
    root: &Path,
    config: &FsConfig,
) -> Result<Option<GuardedFile>, FsError> {
    let entry = match scan.by_name(name) {
        Some(e) => e,
        None => return Ok(None),
    };

    if !entry.is_file() {
        return Err(FsError::NotRegularFile {
            path: entry.path.clone(),
            kind: entry.kind,
        });
    }

    let file = guarded_read(&entry.path, root, config)?;
    Ok(Some(file))
}

/// Read a file by extension. Returns the unique file matching the extension.
///
/// - 0 matches → Ok(None)
/// - 1 match (file) → read it
/// - 2+ matches → Err(Ambiguous)
///
/// Encodes the read_manifest_by_ext pattern from scan.rs.
pub(crate) fn read_by_ext(
    scan: &DirScan,
    ext: &str,
    root: &Path,
    config: &FsConfig,
) -> Result<Option<GuardedFile>, FsError> {
    let indices = scan.by_extension(ext);
    if indices.is_empty() {
        return Ok(None);
    }

    // Filter to files only (reject directories with matching extension).
    let file_entries: Vec<&crate::scan::DirEntry> = indices
        .iter()
        .map(|&i| &scan.entries()[i])
        .filter(|e| e.is_file())
        .collect();

    match file_entries.len() {
        0 => Ok(None),
        1 => {
            let entry = file_entries[0];
            let file = guarded_read(&entry.path, root, config)?;
            Ok(Some(file))
        }
        _ => Err(FsError::Ambiguous {
            root: root.to_path_buf(),
            ext: ext.to_string(),
            files: file_entries.iter().map(|e| e.name.clone()).collect(),
        }),
    }
}

/// Validate that a name exists and is a regular file, without reading content.
///
/// For cheap existence checks like "has tsconfig.json".
///
/// For symlinks: resolves and checks containment before returning.
/// A symlink to a file outside the project root returns Err(SymlinkEscape).
pub(crate) fn validate_entry<'s>(
    scan: &'s DirScan,
    name: &str,
    root: &Path,
) -> Result<Option<&'s DirEntry>, FsError> {
    let entry = match scan.by_name(name) {
        Some(e) => e,
        None => return Ok(None),
    };

    if !entry.is_file() {
        return Err(FsError::NotRegularFile {
            path: entry.path.clone(),
            kind: entry.kind,
        });
    }

    // Re-check symlink status with a fresh lstat rather than the flag captured
    // at scan time (MEDIUM-4). A regular file could be replaced with an escape
    // symlink between scan and this call; the stale is_symlink flag would miss it.
    let live_meta = std::fs::symlink_metadata(&entry.path)
        .map_err(|e| crate::error::classify_io_error(&entry.path, e))?;
    if live_meta.is_symlink() {
        let canonical = std::fs::canonicalize(&entry.path)
            .map_err(|e| crate::error::classify_io_error(&entry.path, e))?;
        if !canonical.starts_with(root) {
            // Do NOT log canonical — that leaks the resolved symlink target (bug #143).
            tracing::debug!(
                path = %entry.path.display(),
                root = %root.display(),
                "symlink escapes project root in validate_entry"
            );
            return Err(FsError::SymlinkEscape {
                path: entry.path.clone(),
            });
        }
    }

    Ok(Some(entry))
}
