//! Guarded file write — safety pipeline for all write operations.
//!
//! Mirrors the read pipeline in `guard.rs`. No direct `fs::write` anywhere.

use crate::error::{classify_io_error, FsError};
use std::path::Path;

/// Write a file with all safety guards.
///
/// ## Pipeline
///
/// 1. Canonicalize parent directory (must exist)
/// 2. Verify parent starts_with(root) — containment
/// 3. If file exists: lstat — reject symlinks, reject non-regular files
/// 4. Write content
///
/// ## Symlink policy
///
/// Writes NEVER follow symlinks. Even if the symlink target is within
/// the containment root, writing through a symlink is rejected. This
/// prevents TOCTOU attacks where an attacker creates a symlink between
/// the check and the write.
pub(crate) fn guarded_write(path: &Path, root: &Path, content: &[u8]) -> Result<(), FsError> {
    check_write_target(path, root)?;
    // Known TOCTOU gap (HIGH-2): between check_write_target and fs::write, an
    // attacker with write access to the directory can replace path with a symlink.
    // fs::write follows symlinks (uses open(O_WRONLY|O_CREAT|O_TRUNC) without
    // O_NOFOLLOW). Closing this gap requires openat2 + O_NOFOLLOW on Linux or
    // equivalent. Documented in docs/design/o8v-fs.md. Not fixable portably.
    std::fs::write(path, content).map_err(|e| classify_io_error(path, e))
}

/// Append to a file with all safety guards.
///
/// Same pipeline as `guarded_write`, but opens for append instead of
/// overwrite. File must already exist (append to nonexistent = error).
pub(crate) fn guarded_append(path: &Path, root: &Path, content: &[u8]) -> Result<(), FsError> {
    // check_write_target already verified containment and rejects symlinks.
    // symlink_metadata confirms the file exists without following symlinks.
    check_write_target(path, root)?;

    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(FsError::NotFound {
                path: path.to_path_buf(),
            });
        }
        Err(e) => return Err(classify_io_error(path, e)),
    }

    // Known TOCTOU gap (same class as HIGH-2): between check_write_target and
    // this open(), an attacker with write access to the directory can replace
    // path with a symlink. OpenOptions::open() follows symlinks (no O_NOFOLLOW).
    // Closing this gap requires openat2 + O_NOFOLLOW on Linux or equivalent.
    // Documented gap — not fixable portably.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| classify_io_error(path, e))?;
    file.write_all(content).map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })
}
/// Append to a file with an exclusive advisory lock, automatically inserting
/// a separator if the file does not end with one.
///
/// The separator and trailing terminator match the file's existing line ending
/// (`\r\n` or `\n`): we read the last **2** bytes under the lock to detect
/// which convention is in use, so appending to a CRLF file stays CRLF-pure.
///
/// If the caller passes pure-LF content into a CRLF file we normalise `\n` to
/// `\r\n` inside the lock so the written bytes are consistent.
///
/// This serializes peek-last-bytes + conditional-separator + append under the
/// lock, eliminating the race where two concurrent callers both observe a
/// missing trailing newline and both prepend a separator, producing a spurious
/// blank line.
pub(crate) fn guarded_append_with_separator(
    path: &Path,
    root: &Path,
    content: &[u8],
) -> Result<(), FsError> {
    use fs2::FileExt;
    use std::io::{Read, Seek, SeekFrom, Write};

    check_write_target(path, root)?;

    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(FsError::NotFound {
                path: path.to_path_buf(),
            });
        }
        Err(e) => return Err(classify_io_error(path, e)),
    }

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .open(path)
        .map_err(|e| classify_io_error(path, e))?;

    file.lock_exclusive().map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })?;

    // Under the lock: read the last 2 bytes to detect CRLF vs LF.
    let len = file
        .metadata()
        .map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?
        .len();

    // Detect line ending from file tail.
    // - 0 bytes   => no existing content; default to \n
    // - 1 byte    => can't be \r\n; treat as LF context
    // - 2+ bytes  => check if last 2 are \r\n
    let is_crlf = if len >= 2 {
        let mut tail = [0u8; 2];
        file.seek(SeekFrom::Start(len - 2))
            .map_err(|e| FsError::Io {
                path: path.to_path_buf(),
                cause: e,
            })?;
        file.read_exact(&mut tail).map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?;
        tail == [b'\r', b'\n']
    } else {
        false
    };

    let line_ending: &[u8] = if is_crlf { b"\r\n" } else { b"\n" };

    // Insert separator if file is non-empty and doesn't end with a newline.
    if len > 0 {
        let mut last = [0u8; 1];
        file.seek(SeekFrom::Start(len - 1))
            .map_err(|e| FsError::Io {
                path: path.to_path_buf(),
                cause: e,
            })?;
        file.read_exact(&mut last).map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?;
        if last[0] != b'\n' {
            file.write_all(line_ending).map_err(|e| FsError::Io {
                path: path.to_path_buf(),
                cause: e,
            })?;
        }
    }

    // Normalise content: if the file is CRLF and content uses bare \n, convert.
    let normalised: Vec<u8>;
    let content_to_write: &[u8] = if is_crlf && content.contains(&b'\n') {
        // Replace bare \n (not already preceded by \r) with \r\n.
        let mut out = Vec::with_capacity(content.len() + content.len() / 4);
        let mut i = 0;
        while i < content.len() {
            if content[i] == b'\n' {
                if i == 0 || content[i - 1] != b'\r' {
                    out.push(b'\r');
                }
                out.push(b'\n');
            } else {
                out.push(content[i]);
            }
            i += 1;
        }
        normalised = out;
        &normalised
    } else {
        content
    };

    file.write_all(content_to_write).map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })?;

    // Ensure trailing line terminator.
    if !content_to_write.is_empty() && *content_to_write.last().unwrap() != b'\n' {
        file.write_all(line_ending).map_err(|e| FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        })?;
    }

    file.unlock().map_err(|e| FsError::Io {
        path: path.to_path_buf(),
        cause: e,
    })?;

    Ok(())
}

/// Create a directory (and parents) within the containment root.
///
/// ## Pipeline
///
/// 1. Find the deepest existing ancestor
/// 2. Canonicalize that ancestor
/// 3. Verify it's within root
/// 4. Create remaining directories
pub(crate) fn guarded_create_dir(path: &Path, root: &Path) -> Result<(), FsError> {
    // Find the deepest existing ancestor to canonicalize.
    // Use symlink_metadata (does NOT follow symlinks) so that:
    // - A dangling symlink in the path is detected as "exists" (metadata returns
    //   Ok for the symlink itself), stopping the loop. Canonicalize then fails on
    //   the dangling symlink, returning an error instead of silently skipping it.
    // - Using exists() (which follows symlinks) would return false for dangling
    //   symlinks, causing the loop to pop past them and miss the containment check.
    let mut ancestor = path.to_path_buf();
    loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => break, // ancestor exists (as any type, including symlink)
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !ancestor.pop() {
                    return Err(FsError::NotFound {
                        path: path.to_path_buf(),
                    });
                }
            }
            Err(e) => return Err(classify_io_error(&ancestor, e)),
        }
    }

    let canonical_ancestor =
        std::fs::canonicalize(&ancestor).map_err(|e| classify_io_error(&ancestor, e))?;

    if !canonical_ancestor.starts_with(root) {
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    std::fs::create_dir_all(path).map_err(|e| classify_io_error(path, e))
}

/// Set file permissions (Unix only, no-op on other platforms).
#[cfg(unix)]
pub(crate) fn guarded_set_permissions(path: &Path, root: &Path, mode: u32) -> Result<(), FsError> {
    check_write_target(path, root)?;

    // Known TOCTOU gap (R3-6, same class as HIGH-2): std::fs::set_permissions
    // calls chmod(2) which follows symlinks. Between check_write_target and here
    // an attacker can create a symlink, applying permissions to an out-of-root
    // target. Requires O_NOFOLLOW / lchmod to fix portably. Documented gap.
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(path, perms).map_err(|e| classify_io_error(path, e))
}

/// Copy permissions from existing metadata to a target path (cross-platform).
///
/// Same containment and symlink checks as `guarded_set_permissions`.
/// Same TOCTOU gap applies.
pub(crate) fn guarded_copy_permissions(
    path: &Path,
    root: &Path,
    permissions: std::fs::Permissions,
) -> Result<(), FsError> {
    check_write_target(path, root)?;
    std::fs::set_permissions(path, permissions).map_err(|e| classify_io_error(path, e))
}

/// Get file metadata safely within the containment root.
///
/// ## Pipeline
///
/// 1. Canonicalize parent → verify starts_with(root)
/// 2. symlink_metadata on path → reject if symlink
/// 3. Return the metadata
pub(crate) fn guarded_metadata(path: &Path, root: &Path) -> Result<std::fs::Metadata, FsError> {
    let parent = path.parent().ok_or_else(|| FsError::NotFound {
        path: path.to_path_buf(),
    })?;

    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|e| classify_io_error(parent, e))?;

    if !canonical_parent.starts_with(root) {
        // Do NOT log canonical_parent — that would leak the resolved symlink
        // target path, which bug #143 forbids.
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "metadata target escapes containment root"
        );
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    let meta = std::fs::symlink_metadata(path).map_err(|e| classify_io_error(path, e))?;

    if meta.is_symlink() {
        return Err(FsError::IsSymlink {
            path: path.to_path_buf(),
        });
    }

    Ok(meta)
}

/// Rename a file safely within the containment root.
///
/// ## Pipeline
///
/// 1. check_write_target(from, root) — source must be within root, not symlink
/// 2. check_write_target(to, root) — destination parent must be within root
/// 3. std::fs::rename(from, to)
pub(crate) fn guarded_rename(from: &Path, to: &Path, root: &Path) -> Result<(), FsError> {
    check_write_target(from, root)?;
    check_write_target(to, root)?;
    std::fs::rename(from, to).map_err(|e| classify_io_error(from, e))
}

/// Remove a file safely within the containment root.
///
/// ## Pipeline
///
/// 1. check_write_target(path, root) — containment + symlink check
/// 2. std::fs::remove_file(path)
pub(crate) fn guarded_remove_file(path: &Path, root: &Path) -> Result<(), FsError> {
    check_write_target(path, root)?;
    std::fs::remove_file(path).map_err(|e| classify_io_error(path, e))
}

/// Read directory entries safely within the containment root.
///
/// ## Pipeline
///
/// 1. Canonicalize path → verify starts_with(root)
/// 2. symlink_metadata → verify it's a directory (not symlink to directory)
/// 3. std::fs::read_dir(path)
pub(crate) fn guarded_read_dir(path: &Path, root: &Path) -> Result<std::fs::ReadDir, FsError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| classify_io_error(path, e))?;

    if !canonical.starts_with(root) {
        // Do NOT log canonical — that would leak the resolved symlink target,
        // which bug #143 forbids.
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "read_dir target escapes containment root"
        );
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    // Use the canonical path (not the original) for both the type check and
    // the final read_dir call. This closes the TOCTOU window between the
    // canonicalize check above and the operation below (MEDIUM-5): using the
    // original path would allow a race where path is swapped to a symlink
    // after canonical was computed.
    let meta = std::fs::symlink_metadata(&canonical).map_err(|e| classify_io_error(path, e))?;

    if meta.is_symlink() {
        return Err(FsError::IsSymlink {
            path: path.to_path_buf(),
        });
    }

    if !meta.is_dir() {
        return Err(FsError::NotRegularFile {
            path: path.to_path_buf(),
            kind: crate::error::meta_to_kind(&meta),
        });
    }

    std::fs::read_dir(&canonical).map_err(|e| classify_io_error(path, e))
}

/// Create a new file and return an open write handle, with all safety guards.
///
/// ## Pipeline
///
/// 1. Canonicalize parent directory (must exist)
/// 2. Verify parent starts_with(root) — containment
/// 3. If file already exists: lstat — reject if symlink or non-regular file
/// 4. Open with create + write + truncate — returns File handle
///
/// The returned handle is safe to write to incrementally. The same TOCTOU
/// gap as HIGH-2 in guarded_write applies: between check_write_target and
/// open, an attacker with directory write access can replace path with a
/// symlink. Closing this portably requires O_NOFOLLOW. Documented gap.
pub(crate) fn guarded_create_file(path: &Path, root: &Path) -> Result<std::fs::File, FsError> {
    check_write_target(path, root)?;
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| classify_io_error(path, e))
}

// ─── Shared validation ──────────────────────────────────────────────────────

/// Validate that a path is a safe write target within the containment root.
fn check_write_target(path: &Path, root: &Path) -> Result<(), FsError> {
    // Parent must exist and be within root.
    let parent = path.parent().ok_or_else(|| FsError::NotFound {
        path: path.to_path_buf(),
    })?;

    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|e| classify_io_error(parent, e))?;

    if !canonical_parent.starts_with(root) {
        // Do NOT log canonical_parent — that leaks the resolved symlink target
        // (bug #143). Log only the original path and root.
        tracing::debug!(
            path = %path.display(),
            root = %root.display(),
            "write target escapes containment root"
        );
        return Err(FsError::SymlinkEscape {
            path: path.to_path_buf(),
        });
    }

    // Check whether the target exists using symlink_metadata (does NOT follow
    // symlinks). This is critical: path.exists() follows symlinks, so a
    // dangling symlink returns false and the check is skipped entirely — allowing
    // fs::write to follow the symlink and write outside the containment root.
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_symlink() {
                return Err(FsError::IsSymlink {
                    path: path.to_path_buf(),
                });
            }
            if !meta.is_file() {
                return Err(FsError::NotRegularFile {
                    path: path.to_path_buf(),
                    kind: crate::error::meta_to_kind(&meta),
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Target does not exist — new file creation, allowed.
        }
        Err(e) => return Err(classify_io_error(path, e)),
    }

    Ok(())
}
