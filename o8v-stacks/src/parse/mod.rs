//! Tool output parsers — turn raw tool output into structured diagnostics.
//!
//! Each tool has a parser module with a `parse()` function.
//! All parsers use [`normalize_path`] for consistent path containment.
//!
//! ## ParseStatus Convention
//!
//! Parsers return `ParseStatus` alongside diagnostics. The meaning:
//!
//! - **`Parsed`**: The parser understood the output format. Zero diagnostics
//!   means the tool's output was clean, not that parsing failed.
//!   - JSON parsers: at least one valid JSON object was deserialized, OR stdout was empty.
//!   - Text parsers (tsc, dotnet, rustfmt): always `Parsed` — the parser scanned
//!     every line. Zero matches means clean output.
//!
//! - **`Unparsed`**: The parser could not understand the output at all.
//!   - JSON parsers: top-level parse failed (not valid JSON).
//!   - Text parsers: should never return `Unparsed` — they always scan.
//!
//! `enrich()` uses this distinction: `Failed + Parsed + 0 diagnostics` means
//! the tool's output format may have changed (force raw fallback), while
//! `Failed + Unparsed` means the output was never understood (already shows raw).

pub mod biome;
pub mod cargo;
pub mod deno;
pub mod dotnet;
pub mod eslint;
pub mod govet;
pub mod hadolint;
pub mod helm;
pub mod javac;
pub mod ktlint;
pub mod kustomize;
pub mod mypy;
pub mod oxlint;
pub mod prettier;
pub mod rebar_compile;
pub mod rebar_dialyzer;
pub mod rebar_xref;
pub mod rubocop;
pub mod ruff;
pub mod rustfmt;
pub mod shellcheck;
pub mod staticcheck;
pub mod swiftlint;
pub mod tflint;
pub mod tsc;

use o8v_core::diagnostic::Location;

/// Normalize a tool-emitted path into a [`Location`].
///
/// 1. If the path is relative and stays within the project → `File`
/// 2. If the path is absolute and under the project root → `File` (stripped)
/// 3. Everything else → `Absolute`
///
/// "Within the project" means no `..` component anywhere in the path.
/// `src/../../outside.rs` escapes even though it doesn't start with `..`.
#[must_use]
pub fn normalize_path(path: &str, project_root: &std::path::Path) -> Location {
    // Empty path is not a valid file location.
    if path.is_empty() {
        return Location::Absolute(String::new());
    }

    // URLs (Deno remote modules, jsr:, npm:, https://) are not file paths.
    // On Unix, `https://deno.land/...` looks relative to Path — catch it early.
    if is_url(path) {
        return Location::Absolute(path.to_string());
    }

    // Detect Windows absolute paths on Unix (e.g. C:\foo\bar.rs from cross-platform logs).
    // On Unix these look relative to Path, but they're not project-relative files.
    if is_windows_absolute(path) {
        return Location::Absolute(path.to_string());
    }

    let p = std::path::Path::new(path);

    if p.is_relative() {
        if escapes(path) {
            return Location::Absolute(path.to_string());
        }
        return Location::File(path.to_string());
    }

    if let Ok(relative) = p.strip_prefix(project_root) {
        let rel = relative.to_string_lossy().to_string();
        if escapes(&rel) {
            return Location::Absolute(path.to_string());
        }
        return Location::File(rel);
    }

    tracing::debug!(path, "path not under project root, using absolute location");
    Location::Absolute(path.to_string())
}

/// Find `(line,col)` in a diagnostic line.
///
/// Searches backwards so filenames with `(` (e.g. `handler(1).ts`) work.
/// Returns `(file_path, line, col, close_paren_byte_position)`.
#[must_use]
pub fn find_location(line: &str) -> Option<(&str, u32, u32, usize)> {
    let bytes = line.as_bytes();
    let mut pos = line.len();
    while pos > 0 {
        pos -= 1;
        if bytes[pos] != b')' {
            continue;
        }
        let close = pos;
        let open = line[..close].rfind('(')?;
        let loc_str = &line[open + 1..close];
        let mut parts = loc_str.split(',');
        let line_num: u32 = match parts.next() {
            Some(s) => match s.trim().parse() {
                Ok(n) => n,
                Err(_) => continue,
            },
            None => continue,
        };
        let col_num: u32 = match parts.next() {
            Some(s) => match s.trim().parse() {
                Ok(n) => n,
                Err(_) => continue,
            },
            None => continue,
        };
        if parts.next().is_some() {
            continue; // extra commas → not (line,col)
        }
        // Diagnostic format: file(N,N): error ...
        // The ) must be followed by ': ' to be a location, not message content.
        // Without this, (3,4) inside an error message would match.
        if !(close + 1 < line.len() && bytes[close + 1] == b':') {
            continue;
        }
        let file = &line[..open];
        if file.is_empty() {
            continue;
        }
        return Some((file, line_num, col_num, close));
    }
    None
}

/// True if a path contains any `..` component — it escapes its root.
fn escapes(path: &str) -> bool {
    std::path::Path::new(path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// True if the string looks like a URL or module specifier, not a file path.
/// Covers: `https://`, `http://`, `jsr:`, `npm:`, `node:`, `data:`.
/// Rejects single-letter schemes to avoid matching Windows drive letters (`C:\`).
/// True if the string is a Windows absolute path (e.g. `C:\foo\bar.rs`).
/// On Unix these look relative to `std::path::Path` but they're not
/// project-relative files — they should be classified as `Absolute`.
fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn is_url(path: &str) -> bool {
    if let Some(colon) = path.find(':') {
        let scheme = &path[..colon];
        // Must be >1 char (rejects "C:" drive letters), all ascii-alpha.
        if scheme.len() > 1 && scheme.bytes().all(|b| b.is_ascii_alphabetic()) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn relative_in_project() {
        assert_eq!(
            normalize_path("src/main.rs", Path::new("/project")),
            Location::File("src/main.rs".to_string())
        );
    }

    #[test]
    fn dotdot_prefix_escapes() {
        assert_eq!(
            normalize_path("../outside.rs", Path::new("/project")),
            Location::Absolute("../outside.rs".to_string())
        );
    }

    #[test]
    fn embedded_dotdot_escapes() {
        assert_eq!(
            normalize_path("src/../../outside.rs", Path::new("/project")),
            Location::Absolute("src/../../outside.rs".to_string())
        );
    }

    #[test]
    fn absolute_under_root_stripped() {
        assert_eq!(
            normalize_path("/project/src/lib.rs", Path::new("/project")),
            Location::File("src/lib.rs".to_string())
        );
    }

    #[test]
    fn absolute_outside_root() {
        assert_eq!(
            normalize_path("/other/file.rs", Path::new("/project")),
            Location::Absolute("/other/file.rs".to_string())
        );
    }

    #[test]
    fn stripped_relative_with_dotdot_escapes() {
        // /project/../outside → strip /project → ../outside → escapes
        assert_eq!(
            normalize_path("/project/../outside.rs", Path::new("/project")),
            Location::Absolute("/project/../outside.rs".to_string())
        );
    }

    #[test]
    fn https_url_is_absolute() {
        assert_eq!(
            normalize_path("https://deno.land/std/fs/mod.ts", Path::new("/project")),
            Location::Absolute("https://deno.land/std/fs/mod.ts".to_string())
        );
    }

    #[test]
    fn jsr_specifier_is_absolute() {
        assert_eq!(
            normalize_path("jsr:@std/path", Path::new("/project")),
            Location::Absolute("jsr:@std/path".to_string())
        );
    }

    #[test]
    fn npm_specifier_is_absolute() {
        assert_eq!(
            normalize_path("npm:express@4", Path::new("/project")),
            Location::Absolute("npm:express@4".to_string())
        );
    }

    #[test]
    fn node_builtin_is_absolute() {
        assert_eq!(
            normalize_path("node:fs", Path::new("/project")),
            Location::Absolute("node:fs".to_string())
        );
    }

    #[test]
    fn windows_drive_is_absolute() {
        // C:\foo is a Windows absolute path — not a project-relative file.
        // On Unix it looks relative to std::path::Path, but is_windows_absolute
        // catches it before the relative-path branch.
        let loc = normalize_path("C:\\foo\\bar.rs", Path::new("/project"));
        assert_eq!(loc, Location::Absolute("C:\\foo\\bar.rs".to_string()));
    }

    #[test]
    fn windows_drive_forward_slash_is_absolute() {
        let loc = normalize_path("C:/foo/bar.rs", Path::new("/project"));
        assert_eq!(loc, Location::Absolute("C:/foo/bar.rs".to_string()));
    }

    #[test]
    fn empty_path_is_absolute() {
        assert_eq!(
            normalize_path("", Path::new("/project")),
            Location::Absolute(String::new())
        );
    }

    // ─── find_location tests ────────────────────────────────────────────

    #[test]
    fn find_location_basic() {
        let (file, line, col, _) =
            super::find_location("file.ts(10,5): error TS2304: msg").unwrap();
        assert_eq!(file, "file.ts");
        assert_eq!(line, 10);
        assert_eq!(col, 5);
    }

    #[test]
    fn find_location_rejects_parens_in_message() {
        // H1: (3,4) in the message must NOT match — only (N,N): counts.
        let line = "file.ts(10,5): error TS2345: Argument of type '(3,4)' is not assignable";
        let (file, line_num, col, _) = super::find_location(line).unwrap();
        assert_eq!(file, "file.ts");
        assert_eq!(line_num, 10);
        assert_eq!(col, 5);
    }

    #[test]
    fn find_location_filename_with_parens() {
        let (file, line, col, _) =
            super::find_location("handler(1).ts(3,5): error TS2304: msg").unwrap();
        assert_eq!(file, "handler(1).ts");
        assert_eq!(line, 3);
        assert_eq!(col, 5);
    }

    #[test]
    fn find_location_no_match() {
        assert!(super::find_location("no location here").is_none());
    }
}
