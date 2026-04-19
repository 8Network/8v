// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Temp-file correlation helpers for run-ID tracking across PreToolUse /
//! PostToolUse hook pairs.
//!
//! All functions accept the storage directory as a parameter — no global state
//! and no env reads — so they are fully testable without touching `~/.8v/`.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use ulid::Ulid;

/// Returns the temp file path for a given session/tool-use pair.
///
/// Path: `<storage>/hook_run/<session_id>/<tool_use_id>`
pub fn temp_path(storage: &Path, session_id: &str, tool_use_id: &str) -> PathBuf {
    storage.join("hook_run").join(session_id).join(tool_use_id)
}

/// Creates parent directories and writes `"<run_id>\n<pre_ms>"` to `path`.
pub fn write_run_record(path: &Path, run_id: &str, pre_ms: u64) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = format!("{run_id}\n{pre_ms}");
    fs::write(path, contents)
}

/// Reads and parses a run record written by [`write_run_record`].
///
/// Returns `(run_id, pre_ms)` on success.
/// Returns `Err` with `ErrorKind::NotFound` if the file is absent.
/// Returns `Err` with `ErrorKind::InvalidData` if the file is malformed.
pub fn read_run_record(path: &Path) -> io::Result<(String, u64)> {
    let contents = fs::read_to_string(path)?;
    let mut lines = contents.splitn(2, '\n');
    let run_id = lines
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "run_id line is empty"))?
        .to_string();
    let pre_ms_str = lines
        .next()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "pre_ms line is missing"))?;
    let pre_ms = pre_ms_str
        .trim()
        .parse::<u64>()
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    Ok((run_id, pre_ms))
}

/// Deletes the temp file at `path`. Ignores `NotFound` (best-effort).
pub fn delete_run_record(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Mints a new run ID as a ULID string (26 Crockford base32 characters).
pub fn mint_run_id() -> String {
    Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- temp_path ---

    #[test]
    fn temp_path_is_stable_and_correct() {
        let dir = TempDir::new().unwrap();
        let p1 = temp_path(dir.path(), "ses_abc", "tool_001");
        let p2 = temp_path(dir.path(), "ses_abc", "tool_001");
        assert_eq!(p1, p2, "same inputs must produce identical paths");
        assert!(
            p1.to_str().unwrap().ends_with("hook_run/ses_abc/tool_001"),
            "path must end with hook_run/<session>/<tool_use>: {:?}",
            p1
        );
    }

    #[test]
    fn temp_path_different_ids_produce_different_paths() {
        let dir = TempDir::new().unwrap();
        let p1 = temp_path(dir.path(), "ses_abc", "tool_001");
        let p2 = temp_path(dir.path(), "ses_abc", "tool_002");
        assert_ne!(p1, p2);
    }

    // --- write_run_record / read_run_record round-trip ---

    #[test]
    fn write_then_read_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(dir.path(), "ses_round", "tool_rt");

        write_run_record(&path, "run_abc123", 1_700_000_000_000).expect("write must succeed");

        let (run_id, pre_ms) = read_run_record(&path).expect("read must succeed");
        assert_eq!(run_id, "run_abc123");
        assert_eq!(pre_ms, 1_700_000_000_000);
    }

    #[test]
    fn write_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        // Path with multiple non-existent parent directories.
        let path = dir.path().join("hook_run").join("ses_new").join("tool_new");

        write_run_record(&path, "run_xyz", 42).expect("write must create parents");
        assert!(path.exists(), "file must exist after write");
    }

    // --- read_run_record missing file ---

    #[test]
    fn read_missing_file_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent");

        let err = read_run_record(&path).expect_err("must return error for missing file");
        assert_eq!(
            err.kind(),
            ErrorKind::NotFound,
            "error kind must be NotFound, got: {:?}",
            err.kind()
        );
    }

    // --- delete_run_record ---

    #[test]
    fn delete_run_record_removes_file() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(dir.path(), "ses_del", "tool_del");
        write_run_record(&path, "run_del", 0).unwrap();
        assert!(path.exists());

        delete_run_record(&path).expect("delete must succeed");
        assert!(!path.exists(), "file must be removed after delete");
    }

    #[test]
    fn delete_run_record_ignores_not_found() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does_not_exist");

        // Must not return an error.
        delete_run_record(&path).expect("delete of missing file must be a no-op");
    }

    // --- mint_run_id ---

    #[test]
    fn mint_run_id_produces_26_char_ulid() {
        let id = mint_run_id();
        assert_eq!(
            id.len(),
            26,
            "ULID must be exactly 26 characters, got: {id:?}"
        );
    }

    #[test]
    fn mint_run_id_produces_unique_ids() {
        let id1 = mint_run_id();
        let id2 = mint_run_id();
        assert_ne!(id1, id2, "consecutive mints must differ");
    }

    #[test]
    fn mint_run_id_contains_only_crockford_base32_chars() {
        // Crockford base32 alphabet: 0-9, A-Z (uppercase only, specific chars excluded)
        // ULID crate produces uppercase Crockford base32.
        let id = mint_run_id();
        assert!(
            id.chars().all(|c| c.is_ascii_alphanumeric()),
            "ULID must contain only alphanumeric chars, got: {id:?}"
        );
    }
}
