// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use crate::workspace::to_io;
use o8v_fs::FsConfig;
use std::path::Path;

// ─── Sentinel format ─────────────────────────────────────────────────────────

const SENTINEL_BEGIN_PREFIX: &str = "<!-- 8v:begin v";
const SENTINEL_END: &str = "<!-- 8v:end -->";

/// Error type for sentinel parsing.
#[derive(Debug, PartialEq)]
pub(super) enum SentinelError {
    /// A begin sentinel was found but no end sentinel exists — malformed state.
    MissingEnd { version: String },
}

impl std::fmt::Display for SentinelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SentinelError::MissingEnd { version } => write!(
                f,
                "malformed 8v block: found '<!-- 8v:begin v{version} -->' but no '<!-- 8v:end -->' \
                 — file is in an inconsistent state. Remove the partial block manually and re-run 8v init."
            ),
        }
    }
}

/// Returns `Some((begin_byte, end_byte, version))` where `begin_byte..end_byte` covers the entire
/// block including both sentinels and a trailing newline (if present), or `None` if no begin
/// sentinel is found. Returns `Err(SentinelError::MissingEnd)` if begin is found but end is not.
fn find_legacy_marker_bounds(text: &str) -> Option<(usize, usize)> {
    let marker = "# 8v\n";
    let start = text.find(marker)?;
    let rest_start = start + marker.len();
    let rest = &text[rest_start..];
    let end = rest
        .find("\n#")
        .map(|pos| rest_start + pos + 1)
        .unwrap_or(text.len());
    Some((start, end))
}

pub(super) fn find_sentinel_bounds(
    text: &str,
) -> Result<Option<(usize, usize, String)>, SentinelError> {
    let Some(begin_pos) = text.find(SENTINEL_BEGIN_PREFIX) else {
        return Ok(None);
    };

    // Extract the version: text after "<!-- 8v:begin v" up to " -->"
    let after_prefix = &text[begin_pos + SENTINEL_BEGIN_PREFIX.len()..];
    let version_end = after_prefix
        .find(" -->")
        .ok_or_else(|| SentinelError::MissingEnd {
            version: "<unknown>".to_string(),
        })?;
    let version = after_prefix[..version_end].to_string();

    // Find the end sentinel — must appear after the begin line
    let begin_line_end = text[begin_pos..]
        .find('\n')
        .map(|off| begin_pos + off + 1)
        .unwrap_or(text.len());

    let rest = &text[begin_line_end..];
    let end_offset = rest
        .find(SENTINEL_END)
        .ok_or_else(|| SentinelError::MissingEnd {
            version: version.clone(),
        })?;

    // Include the end sentinel line itself (and trailing newline if present)
    let end_sentinel_end = begin_line_end + end_offset + SENTINEL_END.len();
    // Consume optional trailing newline
    let end_byte = if text.as_bytes().get(end_sentinel_end) == Some(&b'\n') {
        end_sentinel_end + 1
    } else {
        end_sentinel_end
    };

    Ok(Some((begin_pos, end_byte, version)))
}

/// Build the full versioned block string (begin sentinel + content + end sentinel).
fn make_block(version: &str, content: &str) -> String {
    format!("{SENTINEL_BEGIN_PREFIX}{version} -->\n{content}\n{SENTINEL_END}\n")
}

/// Write or upgrade the versioned 8v block in the file at `path`.
///
/// - Not present → append fresh block.
/// - Present, same version → print "already current", no write.
/// - Present, different version → replace the block in-place.
/// - Present, begin without end → return Err (malformed).
pub(super) fn upsert_versioned_block(
    path: &Path,
    root: &o8v_fs::ContainmentRoot,
    current_version: &str,
) -> std::io::Result<UpsertOutcome> {
    match o8v_fs::safe_exists(path, root) {
        Ok(true) => {}
        Ok(false) => {
            // File does not exist — create with fresh block
            let block = make_block(current_version, AI_SECTION);
            o8v_fs::safe_write(path, root, block.as_bytes()).map_err(to_io)?;
            return Ok(UpsertOutcome::Written);
        }
        Err(e) => return Err(to_io(e)),
    }

    let guarded = o8v_fs::safe_read(path, root, &FsConfig::default()).map_err(to_io)?;
    let content = guarded.content().to_string();

    match find_sentinel_bounds(&content) {
        Err(SentinelError::MissingEnd { version }) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "malformed 8v block in {}: found '<!-- 8v:begin v{version} -->' but no \
                     '<!-- 8v:end -->' — file is in an inconsistent state. \
                     Remove the partial block manually and re-run `8v init`.",
                path.display()
            ),
        )),
        Ok(None) => {
            let block = make_block(current_version, AI_SECTION);
            let new_content = if let Some((start, end)) = find_legacy_marker_bounds(&content) {
                format!("{}{}{}", &content[..start], block, &content[end..])
            } else {
                let separator = if content.ends_with('\n') || content.is_empty() {
                    ""
                } else {
                    "\n"
                };
                format!("{content}{separator}{block}")
            };
            o8v_fs::safe_write(path, root, new_content.as_bytes()).map_err(to_io)?;
            Ok(UpsertOutcome::Written)
        }
        Ok(Some((begin, end, found_version))) => {
            if found_version == current_version {
                return Ok(UpsertOutcome::AlreadyCurrent);
            }
            // Replace the block
            let old_version = found_version.clone();
            let new_block = make_block(current_version, AI_SECTION);
            let new_content = format!("{}{}{}", &content[..begin], new_block, &content[end..]);
            o8v_fs::safe_write(path, root, new_content.as_bytes()).map_err(to_io)?;
            Ok(UpsertOutcome::Upgraded { old_version })
        }
    }
}

#[derive(Debug)]
pub(super) enum UpsertOutcome {
    Written,
    AlreadyCurrent,
    Upgraded { old_version: String },
}

/// Check whether a file has a current-version 8v block (versioned sentinel).
/// Returns `Ok(true)` if found with current version, `Ok(false)` if absent or different version,
/// `Err` if malformed (begin without end).
pub(super) fn file_has_current_block(
    path: &Path,
    root: &o8v_fs::ContainmentRoot,
    current_version: &str,
) -> Result<bool, SentinelError> {
    match o8v_fs::safe_exists(path, root) {
        Ok(true) => {}
        _ => return Ok(false),
    }
    match o8v_fs::safe_read(path, root, &FsConfig::default()) {
        Ok(guarded) => {
            let content = guarded.content();
            match find_sentinel_bounds(content) {
                Ok(Some((_, _, v))) => Ok(v == current_version),
                Ok(None) => Ok(false),
                Err(e) => Err(e),
            }
        }
        Err(_) => Ok(false),
    }
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

    // ─── find_sentinel_bounds unit tests ─────────────────────────────────────

    #[test]
    fn sentinel_bounds_none_when_absent() {
        let text = "# Normal content\n\nNo sentinel here.\n";
        assert_eq!(find_sentinel_bounds(text).unwrap(), None);
    }

    #[test]
    fn sentinel_bounds_finds_block() {
        let text = "# Preamble\n\n<!-- 8v:begin v1.2.3 -->\ncontent\n<!-- 8v:end -->\n\n# Post\n";
        let result = find_sentinel_bounds(text).unwrap().unwrap();
        assert_eq!(result.2, "1.2.3");
        // begin points to the start of the begin sentinel
        assert!(text[result.0..].starts_with("<!-- 8v:begin"));
        // end is past the end sentinel line
        assert!(!text[..result.1].ends_with("<!-- 8v:end -->"));
        // Everything between begin..end contains the full block
        let block = &text[result.0..result.1];
        assert!(block.contains("<!-- 8v:begin v1.2.3 -->"));
        assert!(block.contains("<!-- 8v:end -->"));
        assert!(block.contains("content"));
    }

    #[test]
    fn sentinel_bounds_error_on_missing_end() {
        let text = "# Pre\n\n<!-- 8v:begin v0.1.0 -->\ncontent\n# No end\n";
        let err = find_sentinel_bounds(text).unwrap_err();
        assert_eq!(
            err,
            SentinelError::MissingEnd {
                version: "0.1.0".to_string()
            }
        );
    }

    #[test]
    fn sentinel_bounds_preamble_postamble_preserved() {
        let preamble = "# Preamble\n\n";
        let postamble = "\n# Postamble\n";
        let block = "<!-- 8v:begin v0.0.1 -->\nOLD\n<!-- 8v:end -->\n";
        let text = format!("{preamble}{block}{postamble}");
        let (begin, end, version) = find_sentinel_bounds(&text).unwrap().unwrap();
        assert_eq!(version, "0.0.1");
        assert_eq!(&text[..begin], preamble);
        assert_eq!(&text[end..], postamble);
    }

    // ─── upsert_versioned_block integration tests ────────────────────────────

    #[test]
    fn upsert_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let outcome = upsert_versioned_block(&path, &containment_root, "1.0.0").unwrap();
        assert!(matches!(outcome, UpsertOutcome::Written));

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!-- 8v:begin v1.0.0 -->"));
        assert!(content.contains("<!-- 8v:end -->"));
        assert!(content.contains(AI_SECTION));
    }

    #[test]
    fn upsert_appends_to_existing_file_no_sentinel() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("CLAUDE.md");
        fs::write(&path, "# My Project\n\nExisting content.\n").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let outcome = upsert_versioned_block(&path, &containment_root, "0.5.0").unwrap();
        assert!(matches!(outcome, UpsertOutcome::Written));

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# My Project\n"));
        assert!(content.contains("<!-- 8v:begin v0.5.0 -->"));
        assert!(content.contains("<!-- 8v:end -->"));
    }

    #[test]
    fn upsert_noop_when_already_current() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        let original = "<!-- 8v:begin v1.0.0 -->\nexisting\n<!-- 8v:end -->\n";
        fs::write(&path, original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let outcome = upsert_versioned_block(&path, &containment_root, "1.0.0").unwrap();
        assert!(matches!(outcome, UpsertOutcome::AlreadyCurrent));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn upsert_replaces_outdated_block() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        let original = "# Pre\n\n<!-- 8v:begin v0.0.1 -->\nOLD\n<!-- 8v:end -->\n\n# Post\n";
        fs::write(&path, original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let outcome = upsert_versioned_block(&path, &containment_root, "1.2.3").unwrap();
        assert!(
            matches!(outcome, UpsertOutcome::Upgraded { old_version } if old_version == "0.0.1")
        );

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# Pre\n\n"));
        assert!(content.contains("<!-- 8v:begin v1.2.3 -->"));
        assert!(!content.contains("<!-- 8v:begin v0.0.1 -->"));
        assert!(!content.contains("OLD"));
        assert!(content.ends_with("\n# Post\n"));
    }

    #[test]
    fn upsert_errors_on_malformed_no_end() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        fs::write(&path, "<!-- 8v:begin v0.1.0 -->\nno end sentinel\n").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let err = upsert_versioned_block(&path, &containment_root, "1.0.0").unwrap_err();
        assert!(
            err.to_string().contains("malformed"),
            "error must mention 'malformed': {err}"
        );
    }

    // ─── Legacy marker support (old "# 8v\n" marker) ────────────────────────

    #[test]
    fn upsert_replaces_legacy_marker_with_versioned_block() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        fs::write(&path, "# 8v\n\nOld unversioned content.\n").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        let outcome = upsert_versioned_block(&path, &containment_root, "1.0.0").unwrap();
        assert!(matches!(outcome, UpsertOutcome::Written));
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!-- 8v:begin v1.0.0 -->"));
        // Old block must be gone — no duplicate
        assert!(!content.contains("Old unversioned content."));
        // No double-headings — exactly one # 8v (inside the new sentinel block)
        assert_eq!(content.matches("# 8v").count(), 1);
    }

    #[test]
    fn upsert_legacy_preserves_surrounding_content() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let path = root.join("AGENTS.md");
        fs::write(&path, "# Project\n\nSome intro.\n\n# 8v\n\nOld 8v stuff.\n").unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        upsert_versioned_block(&path, &containment_root, "1.0.0").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        // Prefix preserved
        assert!(content.starts_with("# Project\n\nSome intro.\n\n"));
        // New sentinel present
        assert!(content.contains("<!-- 8v:begin v1.0.0 -->"));
        // Old content gone
        assert!(!content.contains("Old 8v stuff."));
    }
}
