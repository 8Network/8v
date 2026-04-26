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

/// Minimum width or height (in pixels) an image must have before we hand it
/// to a multimodal model as an `ImageContent` block.
///
/// Anthropic's Vision API rejects images below ~4×4 with an opaque 400. A
/// rejected image poisons the entire turn because the tool call has already
/// resolved by the time the API sees the payload — the agent can't recover.
/// We gate conservatively at 8×8.
pub const MIN_IMAGE_DIMENSION: u32 = 8;

/// Parse `(width, height)` from the header bytes of common image formats.
///
/// Returns `None` if the format is unknown or the header is truncated /
/// malformed. Callers that get `None` should treat the file as opaque and
/// deliver it as text+base64 rather than risk an API rejection.
pub fn image_dimensions(bytes: &[u8], mime: &str) -> Option<(u32, u32)> {
    match mime {
        "image/png" => png_dimensions(bytes),
        "image/jpeg" => jpeg_dimensions(bytes),
        "image/gif" => gif_dimensions(bytes),
        "image/bmp" => bmp_dimensions(bytes),
        "image/webp" => webp_dimensions(bytes),
        _ => None,
    }
}

/// PNG: 8-byte signature, then the IHDR chunk. IHDR is always first and has
/// width (u32 BE) at offset 16, height (u32 BE) at offset 20.
fn png_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if b.len() < 24 || &b[0..8] != SIG || &b[12..16] != b"IHDR" {
        return None;
    }
    Some((read_u32_be(&b[16..20])?, read_u32_be(&b[20..24])?))
}

/// JPEG: scan segments until a Start-of-Frame marker (SOF0..SOF3, SOF5..SOF7,
/// SOF9..SOF11, SOF13..SOF15). Height is at offset +5, width at +7 (u16 BE).
fn jpeg_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 8 < b.len() {
        if b[i] != 0xFF {
            return None;
        }
        // Skip fill bytes (0xFF padding).
        while i < b.len() && b[i] == 0xFF {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        let marker = b[i];
        i += 1;
        // SOI/EOI/TEM/RSTn have no payload.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        // Start-of-Frame markers carry width/height.
        let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if i + 7 > b.len() {
            return None;
        }
        if is_sof {
            let h = u16::from_be_bytes([b[i + 3], b[i + 4]]) as u32;
            let w = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            return Some((w, h));
        }
        // Skip past the segment: next 2 bytes are the length (includes itself).
        let seg_len = u16::from_be_bytes([b[i], b[i + 1]]) as usize;
        if seg_len < 2 {
            return None;
        }
        i += seg_len;
    }
    None
}

/// GIF: `GIF87a` / `GIF89a`, then logical screen width (u16 LE) and height
/// (u16 LE) at offsets 6 and 8.
fn gif_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 10 {
        return None;
    }
    if &b[0..6] != b"GIF87a" && &b[0..6] != b"GIF89a" {
        return None;
    }
    let w = u16::from_le_bytes([b[6], b[7]]) as u32;
    let h = u16::from_le_bytes([b[8], b[9]]) as u32;
    Some((w, h))
}

/// BMP: `BM` signature, DIB width at offset 18 (i32 LE), height at offset 22
/// (i32 LE; negative = top-down — use absolute value).
fn bmp_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 26 || &b[0..2] != b"BM" {
        return None;
    }
    let w = i32::from_le_bytes([b[18], b[19], b[20], b[21]]);
    let h = i32::from_le_bytes([b[22], b[23], b[24], b[25]]);
    Some((w.unsigned_abs(), h.unsigned_abs()))
}

/// WebP: `RIFF....WEBP` header, then a VP8/VP8L/VP8X chunk carrying
/// dimensions. Three encodings in the wild; each puts width/height in a
/// different place.
fn webp_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 20 || &b[0..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    match &b[12..16] {
        // Lossy (VP8): 10 bytes into the chunk payload sits a 3-byte frame
        // tag, then width/height as u16 LE (14-bit each, low bits).
        b"VP8 " => {
            if b.len() < 30 {
                return None;
            }
            let w = u16::from_le_bytes([b[26], b[27]]) as u32 & 0x3FFF;
            let h = u16::from_le_bytes([b[28], b[29]]) as u32 & 0x3FFF;
            Some((w, h))
        }
        // Lossless (VP8L): 1-byte signature (0x2F), then 14-bit width-1 and
        // 14-bit height-1 packed little-endian.
        b"VP8L" => {
            if b.len() < 25 || b[20] != 0x2F {
                return None;
            }
            let bits = u32::from_le_bytes([b[21], b[22], b[23], b[24]]);
            let w = (bits & 0x3FFF) + 1;
            let h = ((bits >> 14) & 0x3FFF) + 1;
            Some((w, h))
        }
        // Extended (VP8X): width-1 as u24 LE at +24, height-1 as u24 LE at +27.
        b"VP8X" => {
            if b.len() < 30 {
                return None;
            }
            let w = (u32::from(b[24]) | (u32::from(b[25]) << 8) | (u32::from(b[26]) << 16)) + 1;
            let h = (u32::from(b[27]) | (u32::from(b[28]) << 8) | (u32::from(b[29]) << 16)) + 1;
            Some((w, h))
        }
        _ => None,
    }
}

fn read_u32_be(b: &[u8]) -> Option<u32> {
    (b.len() >= 4).then(|| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
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

    // ── image_dimensions ─────────────────────────────────────────────────

    #[test]
    fn png_1x1_parses() {
        // Minimal 1×1 PNG (canonical fixture).
        let bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        ];
        assert_eq!(image_dimensions(bytes, "image/png"), Some((1, 1)));
    }

    #[test]
    fn png_16x16_parses() {
        let bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10,
        ];
        assert_eq!(image_dimensions(bytes, "image/png"), Some((16, 16)));
    }

    #[test]
    fn gif_dimensions_little_endian() {
        // GIF89a, width=0x1234 LE, height=0x5678 LE
        let bytes: &[u8] = &[b'G', b'I', b'F', b'8', b'9', b'a', 0x34, 0x12, 0x78, 0x56];
        assert_eq!(image_dimensions(bytes, "image/gif"), Some((0x1234, 0x5678)));
    }

    #[test]
    fn bmp_dimensions() {
        // BM + 16 junk bytes, then width=100 LE (i32) at offset 18, height=50 at 22.
        let mut bytes = vec![b'B', b'M'];
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&100i32.to_le_bytes());
        bytes.extend_from_slice(&50i32.to_le_bytes());
        assert_eq!(image_dimensions(&bytes, "image/bmp"), Some((100, 50)));
    }

    #[test]
    fn bmp_negative_height_is_absolute() {
        // BMPs with negative height signal top-down row order; we report the
        // unsigned magnitude — pixels are pixels either way.
        let mut bytes = vec![b'B', b'M'];
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&100i32.to_le_bytes());
        bytes.extend_from_slice(&(-50i32).to_le_bytes());
        assert_eq!(image_dimensions(&bytes, "image/bmp"), Some((100, 50)));
    }

    #[test]
    fn jpeg_sof0_parses() {
        // SOI, then SOF0 marker with height=200, width=300.
        let bytes: &[u8] = &[
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, // SOF0
            0x00, 0x11, // segment length (17)
            0x08, // precision
            0x00, 200, // height
            0x01, 0x2C, // width = 300
            0x03, // components (garbage follows)
        ];
        assert_eq!(image_dimensions(bytes, "image/jpeg"), Some((300, 200)));
    }

    #[test]
    fn webp_vp8l_parses() {
        // RIFF...WEBP VP8L; width=16, height=16 packed as (w-1) | ((h-1)<<14).
        let packed: u32 = (16 - 1) | ((16 - 1) << 14);
        let mut bytes: Vec<u8> = vec![];
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&[0; 4]); // file size, ignored
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(b"VP8L");
        bytes.extend_from_slice(&[0; 4]); // chunk size, ignored
        bytes.push(0x2F); // VP8L signature
        bytes.extend_from_slice(&packed.to_le_bytes());
        assert_eq!(image_dimensions(&bytes, "image/webp"), Some((16, 16)));
    }

    #[test]
    fn unknown_mime_returns_none() {
        assert_eq!(image_dimensions(&[0; 100], "application/pdf"), None);
        assert_eq!(image_dimensions(&[0; 100], "image/tiff"), None);
    }

    #[test]
    fn truncated_header_returns_none() {
        assert_eq!(image_dimensions(&[0x89, 0x50], "image/png"), None);
        assert_eq!(image_dimensions(&[], "image/jpeg"), None);
    }

    #[test]
    fn non_png_bytes_with_png_mime_return_none() {
        // A JPEG magic byte sequence claimed to be PNG — we don't trust MIME,
        // we trust the magic. Returning None causes the caller to downgrade
        // to text, which is the safe default.
        assert_eq!(image_dimensions(&[0xFF, 0xD8, 0xFF], "image/png"), None);
    }
}
