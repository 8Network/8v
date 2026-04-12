// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the MIT License. See LICENSE file in this crate's directory.

//! # o8v-fs
//!
//! Safe filesystem access within a containment boundary. Every read and write
//! goes through guards: symlink containment, file type verification, size
//! limits, TOCTOU narrowing, BOM stripping.
//!
//! ```text
//! o8v-fs  →  o8v-project  →  o8v-core  →  o8v-stacks  →  o8v-check  →  o8v(cli)
//!                                            ↑
//!                                       o8v-process
//! ```
//!
//! ## Example
//!
//! ```no_run
//! use o8v_fs::{SafeFs, FsConfig, FileSystem};
//!
//! let fs = SafeFs::new("./my-project", FsConfig::default()).unwrap();
//! let scan = fs.scan().unwrap();
//! if let Some(cargo) = fs.read_checked(&scan, "Cargo.toml").unwrap() {
//!     println!("found: {}", cargo.content());
//! }
//! ```

mod composite;
pub mod config;
pub mod content;
pub mod error;
pub mod file;
mod fs_trait;
mod guard;
mod root;
mod safe_fs;
pub mod scan;
mod write_guard;

pub use config::FsConfig;
pub use content::{
    count_lines_and_detect_binary, glob_match, glob_match_chars, is_binary_extension,
    LineCountResult,
};
pub use error::{classify_io_error, FileKind, FsError};
pub use file::{truncate_error, GuardedFile};
pub use fs_trait::FileSystem;
pub use root::ContainmentRoot;
pub use safe_fs::SafeFs;
pub use scan::{DirEntry, DirScan};

// ─── Standalone safe write operations ───────────────────────────────────────
//
// These don't require a SafeFs instance. Use when writing to multiple roots
// (e.g. project directory + ~/.8v/) or when SafeFs scoping is unnecessary.

/// Read a file safely within a containment boundary.
///
/// Full guard pipeline: canonicalize, containment, type check, size limit,
/// TOCTOU narrowing, BOM stripping.
pub fn safe_read(
    path: &std::path::Path,
    root: &ContainmentRoot,
    config: &FsConfig,
) -> Result<GuardedFile, FsError> {
    guard::guarded_read(path, root.as_path(), config)
}

/// Check if a path exists safely (no symlink following for the final component).
///
/// Returns `Err(IsSymlink)` if the path is a symlink (dangling or not — this
/// function never follows the final path component). Returns `Err(SymlinkEscape)`
/// if the path escapes the containment root.
///
/// Relative paths are resolved relative to `root`. A bare filename like
/// `"Cargo.toml"` is equivalent to passing `root.join("Cargo.toml")`.
pub fn safe_exists(path: &std::path::Path, root: &ContainmentRoot) -> Result<bool, FsError> {
    let canonical_root = root.as_path();

    // Resolve relative paths against root. A bare filename like "Cargo.toml"
    // is interpreted as root/Cargo.toml. This avoids a false SymlinkEscape
    // error when parent() returns Some("") for single-component relative paths,
    // and ensures the containment check always has an absolute path to work with.
    let resolved;
    let path = if path.is_absolute() {
        path
    } else {
        resolved = canonical_root.join(path);
        &resolved
    };

    if let Some(parent) = path.parent() {
        // parent is now always part of an absolute path (either the caller's or
        // the root-joined one above), so parent.as_os_str() is never empty here.

        // Walk up from parent to find the deepest existing ancestor, then
        // canonicalize it to check containment. This handles both the common
        // case where parent exists and the case where it doesn't (previously
        // the containment check was silently skipped in the latter case).
        let mut ancestor = parent.to_path_buf();
        loop {
            if ancestor.as_os_str().is_empty() {
                // Walked past filesystem root — path escapes containment.
                return Err(FsError::SymlinkEscape {
                    path: path.to_path_buf(),
                });
            }
            if ancestor.exists() {
                let canonical_ancestor = std::fs::canonicalize(&ancestor)
                    .map_err(|e| error::classify_io_error(&ancestor, e))?;
                if !canonical_ancestor.starts_with(canonical_root) {
                    return Err(FsError::SymlinkEscape {
                        path: path.to_path_buf(),
                    });
                }
                break;
            }
            if !ancestor.pop() {
                return Err(FsError::SymlinkEscape {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    // Use symlink_metadata to not follow symlinks.
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_symlink() {
                return Err(FsError::IsSymlink {
                    path: path.to_path_buf(),
                });
            }
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(error::classify_io_error(path, e)),
    }
}

/// Write a file safely within a containment boundary.
///
/// Rejects symlinks, verifies containment, checks file type.
/// See `write_guard::guarded_write` for the full pipeline.
pub fn safe_write(
    path: &std::path::Path,
    root: &ContainmentRoot,
    content: &[u8],
) -> Result<(), FsError> {
    write_guard::guarded_write(path, root.as_path(), content)
}

/// Create a new file and return an open write handle, safely within a containment boundary.
///
/// Same pipeline as `safe_write`: containment check, symlink rejection.
/// Returns a `File` handle for incremental writes (write_all / flush).
/// If the file already exists it is truncated.
pub fn safe_create_file(
    path: &std::path::Path,
    root: &ContainmentRoot,
) -> Result<std::fs::File, FsError> {
    write_guard::guarded_create_file(path, root.as_path())
}

/// Append to a file safely within a containment boundary.
///
/// Same guards as `safe_write`. File must exist.
pub fn safe_append(
    path: &std::path::Path,
    root: &ContainmentRoot,
    content: &[u8],
) -> Result<(), FsError> {
    write_guard::guarded_append(path, root.as_path(), content)
}

/// Create a directory (and parents) safely within a containment boundary.
pub fn safe_create_dir(path: &std::path::Path, root: &ContainmentRoot) -> Result<(), FsError> {
    write_guard::guarded_create_dir(path, root.as_path())
}

/// Set file permissions safely within a containment boundary (Unix only).
#[cfg(unix)]
pub fn safe_set_permissions(
    path: &std::path::Path,
    root: &ContainmentRoot,
    mode: u32,
) -> Result<(), FsError> {
    write_guard::guarded_set_permissions(path, root.as_path(), mode)
}

/// Get file metadata safely within a containment boundary.
///
/// Rejects symlinks, verifies parent containment.
pub fn safe_metadata(
    path: &std::path::Path,
    root: &ContainmentRoot,
) -> Result<std::fs::Metadata, FsError> {
    write_guard::guarded_metadata(path, root.as_path())
}

/// Rename a file safely within a containment boundary.
///
/// Both source and destination must be within root. Rejects symlinks.
pub fn safe_rename(
    from: &std::path::Path,
    to: &std::path::Path,
    root: &ContainmentRoot,
) -> Result<(), FsError> {
    write_guard::guarded_rename(from, to, root.as_path())
}

/// Remove a file safely within a containment boundary.
///
/// Rejects symlinks, verifies containment before removal.
pub fn safe_remove_file(path: &std::path::Path, root: &ContainmentRoot) -> Result<(), FsError> {
    write_guard::guarded_remove_file(path, root.as_path())
}

/// Read directory entries safely within a containment boundary.
///
/// Rejects symlinks to directories, verifies containment.
pub fn safe_read_dir(
    path: &std::path::Path,
    root: &ContainmentRoot,
) -> Result<std::fs::ReadDir, FsError> {
    write_guard::guarded_read_dir(path, root.as_path())
}
