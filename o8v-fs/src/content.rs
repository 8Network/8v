// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the MIT License. See LICENSE file in this crate's directory.

//! File content utilities: binary detection, line counting, glob matching.
//!
//! These are filesystem domain operations used when inspecting file content.

use crate::{ContainmentRoot, FsConfig};
use std::path::Path;

// ─── Glob matching ────────────────────────────────────────────────────────────

/// Simple glob matching: supports `*` (any chars) and `?` (single char).
///
/// Does not support `**` (recursive) — operates on a single path component.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_match_chars(&pat, &txt)
}

/// Recursive character-level glob matcher.
///
/// Called by [`glob_match`] after converting to char slices.
pub fn glob_match_chars(pat: &[char], txt: &[char]) -> bool {
    match (pat.first(), txt.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // `*` matches zero or more characters
            glob_match_chars(&pat[1..], txt)
                || (!txt.is_empty() && glob_match_chars(pat, &txt[1..]))
        }
        (Some('?'), Some(_)) => glob_match_chars(&pat[1..], &txt[1..]),
        (Some(p), Some(t)) if p == t => glob_match_chars(&pat[1..], &txt[1..]),
        _ => false,
    }
}

// ─── Binary extension detection ───────────────────────────────────────────────

/// Detect likely binary files by extension — avoids reading the file.
///
/// Returns `true` if the file extension indicates a binary format. Used to
/// skip binary files without reading their contents.
pub fn is_binary_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "ico"
            | "webp"
            | "svg"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "o"
            | "a"
            | "lib"
            | "wasm"
            | "class"
            | "pyc"
            | "pyo"
            | "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "mp3"
            | "mp4"
            | "wav"
            | "avi"
            | "mov"
            | "mkv"
            | "bin"
            | "dat"
            | "db"
            | "sqlite"
    )
}

// ─── Line counting and binary detection ───────────────────────────────────────

/// Result of reading a file for LOC and binary detection in one pass.
#[derive(Debug)]
pub struct LineCountResult {
    /// Line count. `None` if the file is binary, large, or unreadable.
    pub loc: Option<u64>,
    /// `true` if a NUL byte was found in the file content.
    pub is_binary: bool,
    /// `true` if the file exceeds `max_file_size` and was not read.
    pub is_large: bool,
}

/// Count lines and detect binary in a single read pass using o8v_fs containment.
///
/// - Returns `is_large = true` and skips reading if the file exceeds `max_file_size`.
/// - Returns `is_binary = true` if a NUL byte is found — `loc` is then `None`.
/// - Returns `loc = None` on any read error.
pub fn count_lines_and_detect_binary(
    path: &Path,
    containment: &ContainmentRoot,
    config: &FsConfig,
    max_file_size: u64,
) -> LineCountResult {
    // Check size before reading to guard against large files
    let size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => {
            return LineCountResult {
                loc: None,
                is_binary: false,
                is_large: false,
            }
        }
    };

    if size > max_file_size {
        return LineCountResult {
            loc: None,
            is_binary: false,
            is_large: true,
        };
    }

    let guarded = match crate::safe_read(path, containment, config) {
        Ok(g) => g,
        Err(_) => {
            return LineCountResult {
                loc: None,
                is_binary: false,
                is_large: false,
            }
        }
    };

    let content = guarded.content();

    // Binary detection: NUL byte in content
    if content.as_bytes().contains(&0u8) {
        return LineCountResult {
            loc: None,
            is_binary: true,
            is_large: false,
        };
    }

    let newline_count = content.bytes().filter(|&b| b == b'\n').count() as u64;
    // If file has content but no trailing newline, count it as one extra line
    let loc = if !content.is_empty() && !content.ends_with('\n') {
        newline_count + 1
    } else {
        newline_count
    };

    LineCountResult {
        loc: Some(loc),
        is_binary: false,
        is_large: false,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ─── glob_match ───────────────────────────────────────────────────────────

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("foo.rs", "foo.rs"));
    }

    #[test]
    fn glob_exact_no_match() {
        assert!(!glob_match("foo.rs", "bar.rs"));
    }

    #[test]
    fn glob_star_prefix() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "foo_bar.rs"));
    }

    #[test]
    fn glob_star_suffix() {
        assert!(glob_match("main*", "main.rs"));
        assert!(glob_match("main*", "main_test.go"));
    }

    #[test]
    fn glob_star_infix() {
        assert!(glob_match("*_test*", "foo_test.go"));
        assert!(glob_match("*_test*", "my_test_helper.rs"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(glob_match("foo.?s", "foo.rs"));
        assert!(glob_match("foo.?s", "foo.ts"));
        assert!(!glob_match("foo.?s", "foo.rs2"));
    }

    #[test]
    fn glob_star_matches_empty() {
        assert!(glob_match("*.rs", ".rs"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn glob_empty_pattern_empty_text() {
        assert!(glob_match("", ""));
    }

    #[test]
    fn glob_empty_pattern_nonempty_text() {
        assert!(!glob_match("", "foo"));
    }

    // ─── is_binary_extension ─────────────────────────────────────────────────

    #[test]
    fn binary_ext_png() {
        assert!(is_binary_extension(Path::new("image.png")));
    }

    #[test]
    fn binary_ext_pdf() {
        assert!(is_binary_extension(Path::new("doc.pdf")));
    }

    #[test]
    fn binary_ext_rs_is_not_binary() {
        assert!(!is_binary_extension(Path::new("main.rs")));
    }

    #[test]
    fn binary_ext_no_extension() {
        assert!(!is_binary_extension(Path::new("Makefile")));
    }

    #[test]
    fn binary_ext_case_insensitive() {
        assert!(is_binary_extension(Path::new("image.PNG")));
        assert!(is_binary_extension(Path::new("image.Png")));
    }

    #[test]
    fn binary_ext_wasm() {
        assert!(is_binary_extension(Path::new("module.wasm")));
    }
}
