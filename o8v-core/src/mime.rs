// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! File-kind classification by extension.
//!
//! Three categories, distinguished by what an AI agent can do with the content:
//! - `Text`: UTF-8 readable — source code, markup, config, docs
//! - `ReadableBinary`: PDF and images — needs base64 + MIME for multimodal models
//! - `OpaqueBinary`: archives, executables, object files — no useful reading

/// File kind, derived from extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Text,
    ReadableBinary,
    OpaqueBinary,
}

/// Classify a file by its extension (without the leading dot, case-insensitive).
///
/// Unknown extensions fall through to `Text` — the UTF-8 read path will
/// either succeed (e.g., an unrecognized config format) or fail, at which
/// point the caller decides what to do.
pub fn detect_kind(ext: &str) -> FileKind {
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        // Readable binary — AI can meaningfully consume these as base64.
        "pdf" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "ico" => {
            FileKind::ReadableBinary
        }
        // Opaque binary — no useful reading.
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "exe" | "dll" | "so"
        | "dylib" | "a" | "o" | "wasm" | "class" | "jar" => FileKind::OpaqueBinary,
        // Everything else is assumed to be text.
        _ => FileKind::Text,
    }
}

/// Return the MIME type for a known extension, or `None` if unknown.
///
/// Only populated for extensions the AI can meaningfully use — images, PDFs,
/// and common text formats that matter for multimodal routing.
pub fn mime_for_ext(ext: &str) -> Option<&'static str> {
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tiff" | "tif" => Some("image/tiff"),
        "ico" => Some("image/x-icon"),
        "svg" => Some("image/svg+xml"),
        "zip" => Some("application/zip"),
        "tar" => Some("application/x-tar"),
        "gz" | "tgz" => Some("application/gzip"),
        "exe" | "dll" => Some("application/vnd.microsoft.portable-executable"),
        "wasm" => Some("application/wasm"),
        "jar" => Some("application/java-archive"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_is_readable_binary() {
        assert_eq!(detect_kind("pdf"), FileKind::ReadableBinary);
        assert_eq!(mime_for_ext("pdf"), Some("application/pdf"));
    }

    #[test]
    fn images_are_readable_binary() {
        for ext in [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "ico",
        ] {
            assert_eq!(detect_kind(ext), FileKind::ReadableBinary, "{ext}");
            assert!(mime_for_ext(ext).is_some(), "{ext}");
        }
    }

    #[test]
    fn archives_and_executables_are_opaque() {
        for ext in [
            "zip", "tar", "gz", "7z", "rar", "exe", "dll", "so", "dylib", "a", "o", "wasm",
            "class", "jar",
        ] {
            assert_eq!(detect_kind(ext), FileKind::OpaqueBinary, "{ext}");
        }
    }

    #[test]
    fn svg_is_text() {
        // SVG is XML — AI reads as text, not base64.
        assert_eq!(detect_kind("svg"), FileKind::Text);
    }

    #[test]
    fn code_files_are_text() {
        for ext in ["rs", "py", "ts", "go", "java", "md", "toml", "yaml", "json"] {
            assert_eq!(detect_kind(ext), FileKind::Text, "{ext}");
        }
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(detect_kind("PDF"), FileKind::ReadableBinary);
        assert_eq!(detect_kind("PNG"), FileKind::ReadableBinary);
        assert_eq!(detect_kind("ZIP"), FileKind::OpaqueBinary);
    }

    #[test]
    fn unknown_ext_is_text() {
        assert_eq!(detect_kind("xyz"), FileKind::Text);
        assert_eq!(detect_kind(""), FileKind::Text);
    }
}
