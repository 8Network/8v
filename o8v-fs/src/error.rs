//! Error types — every filesystem failure mode gets its own variant.

use std::path::PathBuf;

/// What kind of filesystem entry this is.
///
/// Cached from `file_type()` at scan time. For symlinks, the TARGET's type
/// (via `fs::metadata()`, not the symlink itself).
///
/// Symlink is not a variant — it's tracked separately by `DirEntry::is_symlink`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
    Fifo,
    Socket,
    Device,
    Other,
}

impl std::fmt::Display for FileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => f.write_str("file"),
            Self::Directory => f.write_str("directory"),
            Self::Fifo => f.write_str("FIFO/pipe"),
            Self::Socket => f.write_str("socket"),
            Self::Device => f.write_str("device"),
            Self::Other => f.write_str("unknown"),
        }
    }
}

/// Every filesystem failure mode gets its own variant.
///
/// No silent fallbacks. No swallowed errors. No "not found" for PermissionDenied.
#[derive(Debug)]
#[non_exhaustive]
pub enum FsError {
    /// Path does not exist.
    NotFound { path: PathBuf },
    /// Permission denied (distinct from NotFound — never conflated, bug #22).
    PermissionDenied { path: PathBuf },
    /// Symlink resolves outside the containment boundary.
    /// Error message omits resolved target (security, bug #143).
    SymlinkEscape { path: PathBuf },
    /// Path is outside the containment boundary (not a symlink issue).
    /// Used when an absolute or resolved path simply does not start with
    /// the containment root — no symlink is involved.
    ContainmentViolation { path: PathBuf },
    /// Not a regular file: directory, FIFO, device, socket, etc.
    NotRegularFile { path: PathBuf, kind: FileKind },
    /// File exceeds the configured size limit.
    TooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
    /// File type changed between stat and open (TOCTOU race).
    RaceCondition { path: PathBuf },
    /// IO error that doesn't fit another category.
    Io {
        path: PathBuf,
        cause: std::io::Error,
    },
    /// Content parsing failed. Message truncated to prevent leaking content (bug #134).
    InvalidContent { path: PathBuf, cause: String },
    /// Multiple files match a single-file query (ambiguity).
    Ambiguous {
        root: PathBuf,
        ext: String,
        files: Vec<String>,
    },
    /// Directory itself could not be read.
    DirectoryUnreadable {
        path: PathBuf,
        cause: std::io::Error,
    },
    /// Path is a symlink — writes never follow symlinks.
    IsSymlink { path: PathBuf },
    /// Directory has too many entries (exceeds configured limit).
    TooManyEntries {
        path: PathBuf,
        count: usize,
        limit: usize,
    },
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path } => write!(f, "not found: {}", path.display()),
            Self::PermissionDenied { path } => {
                write!(f, "permission denied: {}", path.display())
            }
            Self::SymlinkEscape { path } => {
                write!(f, "symlink escapes project directory: {}", path.display())
            }
            Self::ContainmentViolation { path } => {
                write!(f, "path escapes project directory: {}", path.display())
            }
            Self::NotRegularFile { path, kind } => {
                write!(f, "not a regular file ({}): {}", kind, path.display())
            }
            Self::TooLarge { path, size, limit } => {
                write!(
                    f,
                    "file too large ({size} bytes, limit {limit}): {}",
                    path.display()
                )
            }
            Self::RaceCondition { path } => {
                write!(
                    f,
                    "file changed type between check and open: {}",
                    path.display()
                )
            }
            Self::Io { path, cause } => {
                write!(f, "I/O error on {}: {cause}", path.display())
            }
            Self::InvalidContent { path, cause } => {
                const MAX_ERROR_LEN: usize = 200;
                let truncated = if cause.len() > MAX_ERROR_LEN {
                    // floor_char_boundary finds the largest char boundary ≤ the
                    // given index, preventing a panic when slicing mid-char.
                    format!("{}...", &cause[..cause.floor_char_boundary(MAX_ERROR_LEN)])
                } else {
                    cause.to_string()
                };
                write!(f, "invalid content in {}: {truncated}", path.display())
            }
            Self::Ambiguous { root, ext, files } => {
                write!(
                    f,
                    "multiple .{ext} files in {}: {}",
                    root.display(),
                    files.join(", ")
                )
            }
            Self::DirectoryUnreadable { path, cause } => {
                write!(f, "cannot read directory {}: {cause}", path.display())
            }
            Self::IsSymlink { path } => {
                write!(
                    f,
                    "refusing to write: path is a symlink: {}",
                    path.display()
                )
            }
            Self::TooManyEntries { path, count, limit } => {
                write!(
                    f,
                    "directory has too many entries ({count}), limit is {limit}: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for FsError {}

impl FsError {
    /// Machine-readable error kind string for structured output (JSON, etc.).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::PermissionDenied { .. } => "permission_denied",
            Self::SymlinkEscape { .. } => "symlink_escape",
            Self::ContainmentViolation { .. } => "containment_violation",
            Self::NotRegularFile { .. } => "not_regular_file",
            Self::TooLarge { .. } => "too_large",
            Self::RaceCondition { .. } => "race_condition",
            Self::Io { .. } => "io_error",
            Self::InvalidContent { .. } => "manifest_invalid",
            Self::Ambiguous { .. } => "ambiguous",
            Self::IsSymlink { .. } => "is_symlink",
            Self::DirectoryUnreadable { .. } => "directory_unreadable",
            Self::TooManyEntries { .. } => "too_many_entries",
        }
    }

    /// The primary path associated with this error.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::NotFound { path }
            | Self::PermissionDenied { path }
            | Self::SymlinkEscape { path }
            | Self::ContainmentViolation { path }
            | Self::NotRegularFile { path, .. }
            | Self::TooLarge { path, .. }
            | Self::RaceCondition { path }
            | Self::Io { path, .. }
            | Self::InvalidContent { path, .. }
            | Self::IsSymlink { path }
            | Self::DirectoryUnreadable { path, .. }
            | Self::TooManyEntries { path, .. } => path,
            Self::Ambiguous { root, .. } => root,
        }
    }
}

/// Classify an `io::Error` into the appropriate `FsError` variant.
pub fn classify_io_error(path: &std::path::Path, e: std::io::Error) -> FsError {
    match e.kind() {
        std::io::ErrorKind::NotFound => FsError::NotFound {
            path: path.to_path_buf(),
        },
        std::io::ErrorKind::PermissionDenied => FsError::PermissionDenied {
            path: path.to_path_buf(),
        },
        _ => FsError::Io {
            path: path.to_path_buf(),
            cause: e,
        },
    }
}

/// Convert metadata to FileKind for error reporting.
pub(crate) fn meta_to_kind(meta: &std::fs::Metadata) -> FileKind {
    if meta.is_dir() {
        FileKind::Directory
    } else if meta.is_file() {
        FileKind::File
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            let ft = meta.file_type();
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
