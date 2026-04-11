//! SafeFs — the primary implementation of `FileSystem`.

use crate::composite;
use crate::config::FsConfig;
use crate::error::FsError;
use crate::file::GuardedFile;
use crate::fs_trait::FileSystem;
use crate::guard::guarded_read;
use crate::root::ContainmentRoot;
use crate::scan::{scan_directory, DirEntry, DirScan};
use std::path::Path;

/// Safe filesystem access within a containment boundary.
///
/// All operations verify paths stay within the root. Symlinks are resolved
/// and checked. File types are verified before opening. Size limits enforced.
pub struct SafeFs {
    root: ContainmentRoot,
    config: FsConfig,
}

impl SafeFs {
    /// Create a new SafeFs rooted at the given directory.
    ///
    /// The root path is canonicalized at construction time.
    ///
    /// # Errors
    /// Returns `FsError` if the path doesn't exist, isn't a directory,
    /// or can't be canonicalized.
    pub fn new(root: impl AsRef<Path>, config: FsConfig) -> Result<Self, FsError> {
        Ok(Self {
            root: ContainmentRoot::new(root)?,
            config,
        })
    }
}

impl FileSystem for SafeFs {
    fn root(&self) -> &Path {
        self.root.as_path()
    }

    fn dir_name(&self) -> Option<&str> {
        self.root
            .as_path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::trim)
    }

    fn scan(&self) -> Result<DirScan, FsError> {
        scan_directory(self.root.as_path(), self.config.max_dir_entries)
    }

    fn read_checked(&self, scan: &DirScan, name: &str) -> Result<Option<GuardedFile>, FsError> {
        composite::read_checked(scan, name, self.root.as_path(), &self.config)
    }

    fn read_by_ext(&self, scan: &DirScan, ext: &str) -> Result<Option<GuardedFile>, FsError> {
        composite::read_by_ext(scan, ext, self.root.as_path(), &self.config)
    }

    fn validate_entry<'s>(
        &self,
        scan: &'s DirScan,
        name: &str,
    ) -> Result<Option<&'s DirEntry>, FsError> {
        composite::validate_entry(scan, name, self.root.as_path())
    }

    fn read_file(&self, path: &Path) -> Result<GuardedFile, FsError> {
        guarded_read(path, self.root.as_path(), &self.config)
    }
}
