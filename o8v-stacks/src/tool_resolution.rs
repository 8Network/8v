//! Tool resolution utilities — finding tools in project-specific locations.

use o8v_fs::{FileSystem, FsConfig, SafeFs};

/// Scan a project directory using the standard scan policy.
///
/// Single source of truth for how `o8v-core` scans projects for files.
/// All stacks that need to discover files use this — never `SafeFs::new()`
/// directly — so any policy change (exclusions, symlink handling) applies everywhere.
pub(crate) fn scan_project(project_dir: &o8v_fs::ContainmentRoot) -> Option<o8v_fs::DirScan> {
    match SafeFs::new(project_dir.as_path(), FsConfig::default()) {
        Ok(fs) => match fs.scan() {
            Ok(scan) => Some(scan),
            Err(e) => {
                tracing::warn!(error = %e, "failed to scan project directory");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to create SafeFs for project scan");
            None
        }
    }
}

/// Walk up from `start` to find `program` in `node_modules/.bin`.
///
/// Searches recursively up the directory tree from `start` until the root.
/// Returns the first executable found, or `None` if not found.
pub(crate) fn find_node_bin(
    start: &o8v_fs::ContainmentRoot,
    program: &str,
) -> Option<std::path::PathBuf> {
    let mut dir: &std::path::Path = start.as_path();
    loop {
        let candidate = dir.join("node_modules/.bin").join(program);
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}
