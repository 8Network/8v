// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! General-purpose utilities — time and identity helpers.

use std::path::Path;
use std::time::SystemTime;

/// Resolves a path string to an absolute `PathBuf`.
///
/// If `path` is already absolute, returns it as-is.
/// Otherwise, joins it with the current working directory.
pub(crate) fn resolve_path(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .map_err(|e| format!("cannot determine working directory: {e}"))
    }
}

/// Returns the current time as Unix milliseconds.
///
/// Returns 0 if the system clock is before the Unix epoch (debug-logged).
pub(crate) fn unix_ms() -> u64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(e) => {
            tracing::debug!(error = ?e, "unix_ms: could not get current unix timestamp");
            0
        }
    }
}

/// Generates a UUID v4 string.
///
/// Uses `/dev/urandom` for entropy; falls back to a timestamp-based value if
/// `/dev/urandom` is unavailable.
pub(crate) fn new_uuid() -> String {
    use std::io::Read;
    let mut bytes = [0u8; 16];

    // Try to fill bytes from /dev/urandom.
    // If open or read fails, fall through to the timestamp-based fallback.
    let used_urandom = match std::fs::File::open("/dev/urandom") {
        Ok(mut f) => match f.read_exact(&mut bytes) {
            Ok(()) => true,
            Err(e) => {
                tracing::debug!("new_uuid: /dev/urandom read failed: {e}");
                false
            }
        },
        Err(e) => {
            tracing::debug!("new_uuid: /dev/urandom open failed: {e}");
            false
        }
    };

    if !used_urandom {
        // Timestamp-based fallback — not cryptographically random but avoids collisions
        // across restarts when /dev/urandom is unavailable.
        let now = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => d,
            Err(_) => std::time::Duration::from_secs(0),
        };
        bytes[0..8].copy_from_slice(&now.as_secs().to_le_bytes());
        let nanos = now.subsec_nanos().to_le_bytes();
        bytes[8..12].copy_from_slice(&nanos);
        bytes[12..16].copy_from_slice(&nanos);
    }

    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Makes `path` relative to `root`. Falls back to the full path string if not under root.
pub(crate) fn relative_to(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// Parse a semver version string, trimming surrounding whitespace.
pub(crate) fn parse_version(s: &str) -> Result<semver::Version, String> {
    semver::Version::parse(s.trim()).map_err(|e| format!("invalid version: {e}"))
}

/// Get the current executable path (canonicalized, symlinks resolved).
pub(crate) fn get_current_exe() -> Result<std::path::PathBuf, String> {
    std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("cannot locate current binary: {e}"))
}

/// Checks whether `path` matches the extension filter (if set).
pub(crate) fn matches_extension(path: &Path, ext_filter: Option<&str>) -> bool {
    match ext_filter {
        None => true,
        Some(ext) => path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_uuid_format() {
        let uuid = new_uuid();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.chars().filter(|c| *c == '-').count(), 4);
        assert!(uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn test_unix_ms_is_positive() {
        assert!(unix_ms() > 0);
    }
}
