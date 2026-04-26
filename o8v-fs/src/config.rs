//! Filesystem safety configuration.

/// Maximum file size: 10 MB.
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum directory entries: 100K entries per directory.
pub const MAX_DIR_ENTRIES: usize = 100_000;

/// Configuration for filesystem safety guards.
pub struct FsConfig {
    /// Maximum file size in bytes. Default: 10 MB.
    pub max_file_size: u64,
    /// Strip UTF-8 BOM (U+FEFF) from file content. Default: true.
    pub strip_bom: bool,
    /// Maximum number of entries to scan in a single directory. Default: 100K.
    pub max_dir_entries: usize,
}

impl Default for FsConfig {
    fn default() -> Self {
        Self {
            max_file_size: MAX_FILE_SIZE,
            strip_bom: true,
            max_dir_entries: MAX_DIR_ENTRIES,
        }
    }
}
