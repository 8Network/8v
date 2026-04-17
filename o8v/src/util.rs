// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! General-purpose utilities — path helpers.

use std::path::Path;

/// Makes `path` relative to `root`. Falls back to the full path string if not under root.
pub(crate) fn relative_to(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
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
