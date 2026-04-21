// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP root discovery — extract root URI from MCP client.

use rmcp::{Peer, RoleServer};
use url::Url;

/// Resolve and validate a path string against the containment root.
///
/// Produces an absolute path anchored to the MCP root. Relative paths ("src", ".")
/// are joined with the containment root so the caller never receives a path that
/// could be silently resolved against the process CWD.
///
/// Mutates the string in-place on success. Returns `Err(String)` on containment
/// violation or symlink escape.
pub(crate) fn resolve_path(
    path: &mut String,
    containment_root: &o8v_fs::ContainmentRoot,
) -> Result<(), String> {
    let resolved = containment_root.resolve(path);
    let absolute = if resolved.is_absolute() {
        resolved
    } else {
        containment_root.as_path().join(&resolved)
    };
    match containment_root.contains(&absolute) {
        Ok(_exists) => {
            *path = absolute.to_string_lossy().into_owned();
            Ok(())
        }
        Err(e) => Err(path_validation_error(e, &absolute, containment_root)),
    }
}

/// Resolve a slice of path strings against the containment root.
///
/// Each entry may carry a `path:N-M` range suffix. The suffix is stripped before
/// resolution and re-appended afterward so the workspace resolver never sees it.
pub(crate) fn resolve_paths(
    paths: &mut [String],
    containment_root: &o8v_fs::ContainmentRoot,
) -> Result<(), String> {
    for entry in paths.iter_mut() {
        // Strip trailing `:N-M` range suffix before resolving.
        let (base, suffix) = split_range_suffix(entry);
        let mut base_owned = base.to_string();
        resolve_path(&mut base_owned, containment_root)?;
        *entry = if let Some(s) = suffix {
            format!("{base_owned}{s}")
        } else {
            base_owned
        };
    }
    Ok(())
}

/// Split `"path:N-M"` into `("path", Some(":N-M"))` or `("path", None)`.
///
/// Only splits on the last colon when it is followed by `digits-digits`.
fn split_range_suffix(input: &str) -> (&str, Option<&str>) {
    if let Some(colon_pos) = input.rfind(':') {
        let after = &input[colon_pos + 1..];
        if let Some(dash_pos) = after.find('-') {
            let start_ok = after[..dash_pos].chars().all(|c| c.is_ascii_digit())
                && !after[..dash_pos].is_empty();
            let end_ok = after[dash_pos + 1..].chars().all(|c| c.is_ascii_digit())
                && !after[dash_pos + 1..].is_empty();
            if start_ok && end_ok {
                return (&input[..colon_pos], Some(&input[colon_pos..]));
            }
        }
    }
    (input, None)
}

/// Resolve an optional path. No-op when `None`.
pub(crate) fn resolve_optional_path(
    path: &mut Option<String>,
    containment_root: &o8v_fs::ContainmentRoot,
) -> Result<(), String> {
    match path {
        Some(p) => resolve_path(p, containment_root),
        None => Ok(()),
    }
}

/// Return error for path validation failures with contextual messaging.
fn path_validation_error(
    e: o8v_fs::FsError,
    requested: &std::path::Path,
    containment_root: &o8v_fs::ContainmentRoot,
) -> String {
    match e {
        o8v_fs::FsError::SymlinkEscape { .. } | o8v_fs::FsError::ContainmentViolation { .. } => format!(
            "error: path must be inside the current workspace\n  requested: {}\n  workspace: {}\n  hint: cd into a workspace, or pass a path inside the current one",
            requested.display(),
            containment_root.as_path().display(),
        ),
        o8v_fs::FsError::IsSymlink { .. } => "error: path is a symlink".to_string(),
        _ => format!("error: cannot validate path: {e}"),
    }
}

/// Resolve the root URI from the MCP client, returning the directory path.
/// Returns None if the client doesn't provide roots or if URI parsing fails.
pub(super) async fn get_root_directory(client: &Peer<RoleServer>) -> Option<String> {
    match client.list_roots().await {
        Ok(result) => result
            .roots
            .first()
            .and_then(|root| match Url::parse(&root.uri) {
                Ok(u) => match u.to_file_path() {
                    Ok(p) => Some(p.to_string_lossy().to_string()),
                    Err(e) => {
                        tracing::debug!(error = ?e, "mcp path: could not convert URI to file path");
                        None
                    }
                },
                Err(e) => {
                    tracing::debug!(error = ?e, "mcp path: could not parse root URI");
                    None
                }
            }),
        Err(e) => {
            tracing::debug!(error = ?e, "mcp path: could not list MCP roots");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_containment_root(dir: &TempDir) -> o8v_fs::ContainmentRoot {
        let root_path = std::fs::canonicalize(dir.path()).unwrap();
        o8v_fs::ContainmentRoot::new(&root_path).unwrap()
    }

    // --- F8 regression: path-outside-root error must be actionable ---

    /// Before the F8 fix, passing a path outside the workspace returned the
    /// cryptic message "error: path escapes root directory" with no context.
    /// After the fix it must include "workspace", the requested path, and a
    /// hint. This test FAILS on pre-fix code (old message lacked those fields)
    /// and passes on fixed code.
    #[test]
    fn outside_path_error_contains_workspace_hint() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);

        // Use an absolute path that cannot be inside the temp dir workspace.
        // On macOS /tmp is a symlink → /private/tmp, so any /tmp/… path from
        // a differently-rooted workspace triggers FsError::SymlinkEscape.
        // We use /nonexistent_8v_test_path which is guaranteed to be outside.
        let mut path = "/nonexistent_8v_test_path_outside_workspace".to_string();
        let result = resolve_path(&mut path, &root);

        assert!(result.is_err(), "expected Err for out-of-root path");
        let msg = result.unwrap_err();

        assert!(
            msg.contains("workspace"),
            "error message missing 'workspace': {msg}"
        );
        assert!(
            msg.contains("requested"),
            "error message missing 'requested': {msg}"
        );
        assert!(msg.contains("hint"), "error message missing 'hint': {msg}");
    }

    #[test]
    fn outside_path_error_includes_paths() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let mut path = "/nonexistent_8v_test_path_outside_workspace".to_string();
        let result = resolve_path(&mut path, &root);

        assert!(result.is_err());
        let msg = result.unwrap_err();
        // The error must mention the requested path — so the user knows what
        // they asked for.
        assert!(
            msg.contains("nonexistent_8v_test_path_outside_workspace"),
            "error message should mention the requested path: {msg}"
        );
    }

    #[test]
    fn inside_path_resolves_ok() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        std::fs::write(dir.path().join("hello.rs"), "fn main() {}").unwrap();
        let mut path = "hello.rs".to_string();
        let result = resolve_path(&mut path, &root);
        assert!(
            result.is_ok(),
            "expected Ok for in-root path, got {result:?}"
        );
    }
}
