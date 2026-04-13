// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_fs::FsConfig;
use o8v::workspace::to_io;
use std::path::Path;

/// Append `section` to a file if `marker` is not already present.
/// Creates the file if it doesn't exist. All writes go through o8v_fs guards.
pub(super) fn append_section_if_missing(
    path: &Path,
    marker: &str,
    section: &str,
    root: &o8v_fs::ContainmentRoot,
) -> std::io::Result<()> {
    match o8v_fs::safe_exists(path, root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(path, root, &FsConfig::default()).map_err(to_io)?;
            let content = guarded.content();
            if content.contains(marker) {
                return Ok(());
            }
            o8v_fs::safe_append(path, root, section.as_bytes()).map_err(to_io)?;
        }
        Ok(false) => {
            o8v_fs::safe_write(path, root, section.as_bytes()).map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }
    Ok(())
}

pub(super) const AI_SECTION: &str = include_str!("ai_section.txt");

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> PathBuf {
        std::fs::canonicalize(dir.path()).unwrap()
    }

    #[test]
    fn append_section_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("CLAUDE.md");
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        append_section_if_missing(&path, "# 8v\n", AI_SECTION, &containment_root).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# 8v"));
    }

    #[test]
    fn append_section_appends_when_marker_absent() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("CLAUDE.md");
        fs::write(&path, "# My Project\n\nSome instructions.\n").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        append_section_if_missing(&path, "# 8v\n", AI_SECTION, &containment_root).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# My Project"));
        assert!(content.contains("# 8v"));
    }

    #[test]
    fn append_section_skips_when_marker_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("CLAUDE.md");
        let original = "# 8v\n\nSome existing content.\n";
        fs::write(&path, original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        append_section_if_missing(&path, "# 8v\n", AI_SECTION, &containment_root).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, original, "file should not be modified");
    }

    #[test]
    fn append_section_handles_file_without_trailing_newline() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("CLAUDE.md");
        fs::write(&path, "# Project\nNo trailing newline").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        append_section_if_missing(&path, "# 8v\n", AI_SECTION, &containment_root).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Project"));
        assert!(content.contains("# 8v"));
    }
}
