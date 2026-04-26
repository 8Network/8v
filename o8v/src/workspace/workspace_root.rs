// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `WorkspaceRoot` — the trust boundary for all file I/O.
//!
//! For CLI: resolved by walking up from CWD to find `.git/` or `.8v/`.
//! For MCP: the client-provided root directory.
//!
//! All file-access commands get this from CommandContext Extensions.
//! No command creates its own ContainmentRoot from CWD.

use o8v_fs::ContainmentRoot;
use std::path::{Path, PathBuf};

/// The trust boundary — all file I/O is contained within this root.
///
/// One per session. Goes into CommandContext Extensions. Commands get it
/// from context, never from `std::env::current_dir()`.
#[derive(Clone)]
pub struct WorkspaceRoot {
    containment: ContainmentRoot,
}

impl WorkspaceRoot {
    /// Create a workspace root at the given path.
    ///
    /// The path must be a valid directory. It becomes the containment
    /// boundary for all file operations in this session.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, o8v_fs::FsError> {
        let containment = ContainmentRoot::new(path)?;
        Ok(Self { containment })
    }

    /// The containment root for safe_* filesystem operations.
    pub fn containment(&self) -> &ContainmentRoot {
        &self.containment
    }

    /// The workspace root path.
    pub fn as_path(&self) -> &Path {
        self.containment.as_path()
    }

    /// Resolve a path (relative or absolute) within the workspace.
    ///
    /// Relative paths are joined against the workspace root.
    /// Absolute paths are returned as-is.
    /// Containment validation happens at the safe_* call site, not here.
    pub fn resolve(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.containment.as_path().join(p)
        }
    }
}
