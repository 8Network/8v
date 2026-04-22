//! Guarded file read — the 9-step safety pipeline.
//!
//! Every file read goes through this. No direct `fs::read_to_string` anywhere.

use crate::config::FsConfig;
use crate::error::{classify_io_error, FileKind, FsError};
use crate::file::GuardedFile;
use std::path::Path;

/// Read a file with all safety guards.
///
/// ## Pipeline
///
/// 1. Canonicalize path
/// 2. Verify canonical path starts_with(root) — containment
/// 3. metadata() — not file? → NotRegularFile
/// 4. File::open(canonical)
/// 5. metadata() on open fd — type changed? → RaceCondition (TOCTOU)
/// 6. Check size against limit
/// 7. Read content from fd
/// 8. Strip BOM if configured
/// 9. Return GuardedFile
///
/// ## Known TOCTOU limitation
///
/// Step 5 re-checks file type but NOT containment. A swap between steps 2
/// and 4 is not caught. Eliminating this requires openat(2)/O_NOFOLLOW/fstat.
pub(crate) fn guarded_read(
    path: &Path,
    root: &Path,
    config: &FsConfig,
) -> Result<GuardedFile, FsError> {
    use std::io::Read;

    // Step 1: Canonicalize
    let canonical = std::fs::canonicalize(path).map_err(|e| classify_io_error(path, e))?;

    // Step 2: Containment check
    if !canonical.starts_with(root) {
        // Do NOT log `canonical` (the resolved path) — that would leak the
        // symlink target, which is exactly what bug #143 forbids.
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "path escapes project root"
        );
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    // Step 3: Pre-open type check (prevents FIFO blocking)
    let pre_meta = std::fs::metadata(&canonical).map_err(|e| classify_io_error(path, e))?;
    if !pre_meta.is_file() {
        return Err(FsError::NotRegularFile {
            path: path.to_path_buf(),
            kind: meta_to_kind(&pre_meta),
        });
    }

    // Step 4: Open
    let file = std::fs::File::open(&canonical).map_err(|e| classify_io_error(path, e))?;

    // Step 5: Re-check on fd (TOCTOU narrowing)
    let fd_meta = file.metadata().map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })?;
    if !fd_meta.is_file() {
        return Err(FsError::RaceCondition {
            path: path.to_path_buf(),
        });
    }

    // Step 6: Size check
    if fd_meta.len() > config.max_file_size {
        return Err(FsError::TooLarge {
            path: path.to_path_buf(),
            size: fd_meta.len(),
            limit: config.max_file_size,
        });
    }

    // Step 7: Read content
    let capacity = usize::try_from(fd_meta.len())
        .expect("file size bounded by max_file_size (10MB) — always fits in usize");
    let mut content = String::with_capacity(capacity);
    std::io::BufReader::new(file)
        .read_to_string(&mut content)
        .map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?;

    // Step 8: Strip BOM
    if config.strip_bom && content.starts_with('\u{FEFF}') {
        content.drain(..'\u{FEFF}'.len_utf8());
    }

    // Step 9: Return
    Ok(GuardedFile::new(path.to_path_buf(), content))
}

/// Convert metadata to FileKind for error reporting.
fn meta_to_kind(meta: &std::fs::Metadata) -> FileKind {
    crate::error::meta_to_kind(meta)
}

/// Read a file as raw bytes with all safety guards.
///
/// Mirrors `guarded_read` steps 1-6 (canonicalize, containment, type check,
/// TOCTOU narrowing, size check) then reads bytes instead of a UTF-8 string.
/// No BOM handling — the caller gets exactly what's on disk.
pub(crate) fn guarded_read_bytes(
    path: &Path,
    root: &Path,
    config: &FsConfig,
) -> Result<Vec<u8>, FsError> {
    use std::io::Read;

    let canonical = std::fs::canonicalize(path).map_err(|e| classify_io_error(path, e))?;

    if !canonical.starts_with(root) {
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "path escapes project root"
        );
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    let pre_meta = std::fs::metadata(&canonical).map_err(|e| classify_io_error(path, e))?;
    if !pre_meta.is_file() {
        return Err(FsError::NotRegularFile {
            path: path.to_path_buf(),
            kind: meta_to_kind(&pre_meta),
        });
    }

    let file = std::fs::File::open(&canonical).map_err(|e| classify_io_error(path, e))?;

    let fd_meta = file.metadata().map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })?;
    if !fd_meta.is_file() {
        return Err(FsError::RaceCondition {
            path: path.to_path_buf(),
        });
    }

    if fd_meta.len() > config.max_file_size {
        return Err(FsError::TooLarge {
            path: path.to_path_buf(),
            size: fd_meta.len(),
            limit: config.max_file_size,
        });
    }

    let capacity = usize::try_from(fd_meta.len())
        .expect("file size bounded by max_file_size — always fits in usize");
    let mut bytes = Vec::with_capacity(capacity);
    std::io::BufReader::new(file)
        .read_to_end(&mut bytes)
        .map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?;

    Ok(bytes)
}
