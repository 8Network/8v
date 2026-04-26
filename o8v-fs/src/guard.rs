//! Guarded file read — the 9-step safety pipeline.
//!
//! Every file read goes through this. No direct `fs::read_to_string` anywhere.

use crate::config::FsConfig;
use crate::error::{classify_io_error, FileKind, FsError};
use crate::file::GuardedFile;
use std::path::Path;

/// Normalize a path lexically (without following symlinks) by processing
/// `.` and `..` components.  The result is an absolute path that may not
/// exist on disk.
fn lexical_normalize(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// Returns `true` if a symlink component **within the provided path** caused
/// the path to resolve outside `root`.
///
/// The logic: canonicalize the path up to the point where we would follow the
/// final component (using the parent directory canonical path + the filename).
/// If the parent resolves inside `root`, but the full canonical path is
/// outside, then a symlink inside `root` caused the escape.
///
/// This avoids lexical normalization issues with OS-level symlinks such as
/// macOS `/var` → `/private/var` which would cause false negatives when root
/// is canonical (`/private/var/...`) but a lexically-normalized path still
/// refers to `/var/...`.
fn symlink_caused_escape(path: &Path, root: &Path, canonical: &Path) -> bool {
    // Resolve the parent directory of `path` through symlinks.  This handles
    // OS-level symlinks (e.g. macOS /var → /private/var) without treating
    // user-placed symlinks in the final component as "part of the path".
    let parent = path.parent().unwrap_or(path);
    let canonical_parent = match std::fs::canonicalize(parent) {
        Ok(p) => p,
        // If parent doesn't exist (e.g. dangling intermediate path), fall
        // back to lexical check — if lexical is also outside root, it's a
        // plain violation.
        Err(_) => {
            let abs = if path.is_absolute() {
                lexical_normalize(path)
            } else if let Ok(cwd) = std::env::current_dir() {
                lexical_normalize(&cwd.join(path))
            } else {
                return false;
            };
            return abs.starts_with(root) && !canonical.starts_with(root);
        }
    };

    // If the parent directory itself is outside root, no symlink inside root
    // caused the escape — the path is plainly outside.
    if !canonical_parent.starts_with(root) {
        return false;
    }

    // Parent is inside root but full canonical is outside → a symlink (the
    // final component, or a symlink chain from the final component) escaped.
    !canonical.starts_with(root)
}

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

    // Step 0: Pre-canonicalize type check — prevents FIFO/socket/directory blocking.
    // symlink_metadata() = lstat(), does NOT follow symlinks, does NOT open the fd.
    // Safe to call on FIFOs (unlike canonicalize which calls realpath/open).
    // Allow regular files and symlinks through; symlinks may point to regular files
    // and will be validated again after canonicalize.
    {
        let pre = std::fs::symlink_metadata(path).map_err(|e| classify_io_error(path, e))?;
        if !pre.is_file() && !pre.file_type().is_symlink() {
            return Err(FsError::NotRegularFile {
                path: path.to_path_buf(),
                kind: meta_to_kind(&pre),
            });
        }
    }

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
        // Distinguish: a symlink whose target escapes → SymlinkEscape.
        // A plain path that simply lives outside root → ContainmentViolation.
        if symlink_caused_escape(path, root, &canonical) {
            return Err(FsError::SymlinkEscape {
                path: path.to_path_buf(),
            });
        }
        return Err(FsError::ContainmentViolation {
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

    // Step 0: Pre-canonicalize type check (mirrors guarded_read Step 0).
    {
        let pre = std::fs::symlink_metadata(path).map_err(|e| classify_io_error(path, e))?;
        if !pre.is_file() && !pre.file_type().is_symlink() {
            return Err(FsError::NotRegularFile {
                path: path.to_path_buf(),
                kind: meta_to_kind(&pre),
            });
        }
    }

    let canonical = std::fs::canonicalize(path).map_err(|e| classify_io_error(path, e))?;

    if !canonical.starts_with(root) {
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "path escapes project root"
        );
        if symlink_caused_escape(path, root, &canonical) {
            return Err(FsError::SymlinkEscape {
                path: path.to_path_buf(),
            });
        }
        return Err(FsError::ContainmentViolation {
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
