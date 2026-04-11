//! Directory scanning with harvest/yield — one bad entry doesn't kill the scan.

use crate::error::{FileKind, FsError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A directory entry with cached metadata.
///
/// **UTF-8 contract:** `name` is always valid UTF-8. Non-UTF-8 entries are reported
/// as errors during scan and excluded from entries. This is an API guarantee.
///
/// For non-symlinks: `kind` comes from `DirEntry::file_type()` (no extra syscall).
/// For symlinks: `kind` comes from `fs::metadata()` (follows the symlink to get
/// the TARGET's type).
#[derive(Debug)]
pub struct DirEntry {
    /// Filename (always valid UTF-8).
    pub name: String,
    /// Full path to the entry.
    pub path: PathBuf,
    /// The resolved kind. For symlinks, the TARGET's type.
    pub kind: FileKind,
    /// True if this entry is a symlink (orthogonal to kind).
    pub is_symlink: bool,
    /// File extension (lowercase, without dot).
    pub extension: Option<String>,
}

impl DirEntry {
    #[must_use]
    /// True if this entry is a regular file (or symlink to one).
    pub fn is_file(&self) -> bool {
        self.kind == FileKind::File
    }
}

/// Result of scanning a directory.
///
/// Harvest/yield: entries and errors collected separately.
/// One bad entry doesn't kill the scan.
pub struct DirScan {
    entries: Vec<DirEntry>,
    errors: Vec<FsError>,
    by_name: HashMap<String, usize>,
    by_ext: HashMap<String, Vec<usize>>,
}

impl DirScan {
    #[must_use]
    /// All scanned entries (sorted by name).
    pub fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    #[must_use]
    /// Errors encountered during scan (harvest/yield — one bad entry doesn't fail the scan).
    pub fn errors(&self) -> &[FsError] {
        &self.errors
    }

    /// Move collected scan errors out. For boundary translation in o8v-project.
    pub fn take_errors(&mut self) -> Vec<FsError> {
        std::mem::take(&mut self.errors)
    }

    #[must_use]
    /// Look up a single entry by exact filename. O(1) via HashMap.
    pub fn by_name(&self, name: &str) -> Option<&DirEntry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }

    #[must_use]
    /// Entry indices with the given extension. O(1) via HashMap. No allocation.
    pub fn by_extension(&self, ext: &str) -> &[usize] {
        self.by_ext.get(ext).map_or(&[], Vec::as_slice)
    }

    /// Iterate entries with the given extension. No allocation.
    pub fn entries_with_extension(&self, ext: &str) -> impl Iterator<Item = &DirEntry> {
        let indices = self.by_extension(ext);
        indices.iter().map(move |&i| &self.entries[i])
    }

    #[must_use]
    /// True if an entry with this name exists. O(1) via HashMap.
    pub fn has_entry(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }
}

/// Scan a directory, building a `DirScan` with indexes.
///
/// Follows the design's scan pipeline:
/// 1. `read_dir(root)` — Err → `FsError::DirectoryUnreadable`
/// 2. For each entry: harvest Ok entries, collect Err as `FsError`
/// 3. Stop if entries exceed `max_dir_entries`, report as error
/// 4. For symlinks: resolve target type via `fs::metadata()`
/// 5. Build name + extension indexes
pub(crate) fn scan_directory(root: &Path, max_dir_entries: usize) -> Result<DirScan, FsError> {
    let read_dir = std::fs::read_dir(root).map_err(|e| FsError::DirectoryUnreadable {
        path: root.to_path_buf(),
        cause: e,
    })?;

    let mut entries = Vec::new();
    let mut errors = Vec::new();

    for result in read_dir {
        // Check limit before processing next entry.
        if entries.len() >= max_dir_entries {
            errors.push(FsError::TooManyEntries {
                path: root.to_path_buf(),
                count: entries.len(),
                limit: max_dir_entries,
            });
            break;
        }

        let entry = match result {
            Ok(e) => e,
            Err(e) => {
                errors.push(FsError::Io {
                    path: root.to_path_buf(),
                    cause: e,
                });
                continue;
            }
        };

        // Get file type — this does NOT follow symlinks.
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                errors.push(FsError::Io {
                    path: entry.path(),
                    cause: e,
                });
                continue;
            }
        };

        // UTF-8 contract: non-UTF-8 filenames are errors, not silently skipped.
        let name = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(os_str) => {
                let lossy_name = os_str.to_string_lossy();
                errors.push(FsError::InvalidContent {
                    path: entry.path(),
                    cause: format!("non-UTF-8 filename skipped: {}", lossy_name),
                });
                continue;
            }
        };

        let is_symlink = file_type.is_symlink();

        // Resolve the kind. For symlinks, follow to get the target's type.
        let kind = if is_symlink {
            match std::fs::metadata(entry.path()) {
                Ok(meta) => file_type_to_kind(&meta.file_type()),
                Err(e) => {
                    // Dangling symlink or permission error — collect as error, skip entry.
                    errors.push(crate::error::classify_io_error(&entry.path(), e));
                    continue;
                }
            }
        } else {
            file_type_to_kind(&file_type)
        };

        let extension = std::path::Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        entries.push(DirEntry {
            name,
            path: entry.path(),
            kind,
            is_symlink,
            extension,
        });
    }

    // Sort by name — deterministic order regardless of filesystem.
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Build indexes (after sort — indices match sorted positions).
    let mut by_name = HashMap::with_capacity(entries.len());
    let mut by_ext: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, entry) in entries.iter().enumerate() {
        by_name.insert(entry.name.clone(), i);
        if let Some(ref ext) = entry.extension {
            by_ext.entry(ext.clone()).or_default().push(i);
        }
    }

    Ok(DirScan {
        entries,
        errors,
        by_name,
        by_ext,
    })
}

/// Convert a `std::fs::FileType` to our `FileKind`.
fn file_type_to_kind(ft: &std::fs::FileType) -> FileKind {
    if ft.is_file() {
        FileKind::File
    } else if ft.is_dir() {
        FileKind::Directory
    } else {
        // Unix-specific: check for FIFO, socket, device.
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if ft.is_fifo() {
                return FileKind::Fifo;
            }
            if ft.is_socket() {
                return FileKind::Socket;
            }
            if ft.is_block_device() || ft.is_char_device() {
                return FileKind::Device;
            }
        }
        FileKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_directory_entry_limit() {
        // Create a temporary directory
        let tmpdir = TempDir::new().expect("failed to create temp dir");
        let dir_path = tmpdir.path();

        // Create 10 test files
        for i in 0..10 {
            let file_path = dir_path.join(format!("file_{:02}.txt", i));
            fs::write(&file_path, format!("content {}", i)).expect("failed to write file");
        }

        // Scan with a limit of 5 entries
        let result = scan_directory(dir_path, 5);
        assert!(result.is_ok());

        let scan = result.unwrap();

        // Should only have 5 entries (limit was hit)
        assert_eq!(scan.entries().len(), 5);

        // Should have an error about too many entries
        let errors = scan.errors();
        assert_eq!(errors.len(), 1);

        match &errors[0] {
            FsError::TooManyEntries { path, count, limit } => {
                assert_eq!(path, dir_path);
                assert_eq!(*count, 5);
                assert_eq!(*limit, 5);
            }
            _ => panic!("expected TooManyEntries error, got {:?}", errors[0]),
        }
    }

    #[test]
    fn test_directory_below_limit() {
        // Create a temporary directory
        let tmpdir = TempDir::new().expect("failed to create temp dir");
        let dir_path = tmpdir.path();

        // Create 3 test files
        for i in 0..3 {
            let file_path = dir_path.join(format!("file_{:02}.txt", i));
            fs::write(&file_path, format!("content {}", i)).expect("failed to write file");
        }

        // Scan with a limit of 5 entries (above actual count)
        let result = scan_directory(dir_path, 5);
        assert!(result.is_ok());

        let scan = result.unwrap();

        // Should have all 3 entries
        assert_eq!(scan.entries().len(), 3);

        // Should have no errors
        assert_eq!(scan.errors().len(), 0);
    }
}
