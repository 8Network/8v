// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_core::types::{SessionId, Warning, WarningSink};
use std::collections::HashSet;
use std::path::Path;

pub struct ArgvNormalizer {
    warned_sessions: HashSet<String>,
}

impl ArgvNormalizer {
    pub fn new() -> Self {
        Self {
            warned_sessions: HashSet::new(),
        }
    }

    fn normalize_token(
        &mut self,
        token: &str,
        canonical_project: Option<&Path>,
        session_id: &str,
        warnings: &mut WarningSink,
    ) -> String {
        if is_quoted_string(token) {
            return "<str>".to_string();
        }
        // Strip a trailing `:N` or `:N-M` range suffix (e.g. `path.rs:10-20`)
        // before path classification.  The suffix is NOT part of the filesystem
        // path and would cause canonicalize() to fail on a perfectly valid file.
        let (path_part, range_suffix) = split_range_suffix(token);
        if !looks_like_path(path_part) {
            return token.to_string();
        }
        let path = Path::new(path_part);
        if is_tempdir(path) {
            // Preserve range suffix in the shape: `<tmp>:10-20`
            return format!("<tmp>{range_suffix}");
        }
        if path.is_absolute() {
            // Only canonicalize if the path actually exists; otherwise fall
            // back to <abs> without emitting a warning.  The range suffix has
            // already been stripped (above), so a nonexistent-file path like
            // "/repo/src/main.rs" will silently become "<abs>" — no warning.
            // CanonicalizeFailed is reserved for paths that exist but cannot
            // be resolved (e.g. permission errors).
            let canonical_opt = if path.exists() {
                match std::fs::canonicalize(path) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        warnings.push(Warning::CanonicalizeFailed {
                            path: path_part.to_string(),
                            reason: e.to_string(),
                        });
                        None
                    }
                }
            } else {
                None
            };
            if let Some(canonical) = canonical_opt {
                if let Some(project) = canonical_project {
                    if canonical.starts_with(project) {
                        let rel = match canonical.strip_prefix(project) {
                            Ok(r) => r.to_path_buf(),
                            Err(_) => canonical.clone(),
                        };
                        return format!(
                            "{}{}",
                            normalize_separators(&rel.to_string_lossy()),
                            range_suffix
                        );
                    }
                }
            }
            return format!("<abs>{range_suffix}");
        }
        if canonical_project.is_none() {
            if !self.warned_sessions.contains(session_id) {
                self.warned_sessions.insert(session_id.to_string());
                warnings.push(Warning::NormalizerBasenameFallback {
                    session: SessionId::from_raw_unchecked(session_id.to_string()),
                    path: path_part.to_string(),
                });
            }
            let base = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_part.to_string());
            return format!("{base}{range_suffix}");
        }
        format!("{}{}", normalize_separators(path_part), range_suffix)
    }

    pub fn normalize_argv(
        &mut self,
        argv: &[String],
        project_path: Option<&str>,
        session_id: &str,
        warnings: &mut WarningSink,
    ) -> String {
        let canonical_project: Option<std::path::PathBuf> =
            project_path.map(|p| match std::fs::canonicalize(p) {
                Ok(c) => c,
                Err(_) => std::path::Path::new(p).to_path_buf(),
            });
        // Track whether the next token is a value for a content-carrying flag.
        // Such tokens must NOT be path-normalised; they are user-supplied
        // strings that may incidentally look like paths (e.g. `// comment`).
        let mut next_is_flag_value = false;
        let normalized: Vec<String> = argv
            .iter()
            .map(|tok| {
                if next_is_flag_value {
                    next_is_flag_value = false;
                    return "<str>".to_string();
                }
                if is_content_flag(tok) {
                    next_is_flag_value = true;
                    return tok.clone();
                }
                self.normalize_token(tok, canonical_project.as_deref(), session_id, warnings)
            })
            .collect();
        normalized.join(" ")
    }
}

impl Default for ArgvNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

fn is_quoted_string(token: &str) -> bool {
    (token.starts_with('"') && token.ends_with('"') && token.len() >= 2)
        || (token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2)
}

pub(crate) fn looks_like_path(token: &str) -> bool {
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains('/')
        || token.contains('\\')
}

fn is_tempdir(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.starts_with("/tmp")
        || s.starts_with("/var/folders")
        || s.starts_with("/private/tmp")
        || match std::env::var("TMPDIR") {
            Ok(td) => s.starts_with(td.trim_end_matches('/')),
            Err(_) => false,
        }
}

fn normalize_separators(s: &str) -> String {
    s.replace('\\', "/")
}

/// Strip a trailing range suffix of the form `:N` or `:N-M` from `token`.
/// Returns `(path_part, suffix)` where `suffix` is the stripped portion
/// (empty string if no suffix was found).
fn split_range_suffix(token: &str) -> (&str, &str) {
    if let Some(colon_pos) = token.rfind(':') {
        let after = &token[colon_pos + 1..];
        if is_range_suffix(after) {
            return (&token[..colon_pos], &token[colon_pos..]);
        }
    }
    (token, "")
}

fn is_range_suffix(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut iter = s.splitn(2, '-');
    let start = iter.next().unwrap_or("");
    if start.is_empty() || !start.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match iter.next() {
        None => true,                                                            // `:N`
        Some(end) => !end.is_empty() && end.chars().all(|c| c.is_ascii_digit()), // `:N-M`
    }
}

/// Returns true if `tok` is a flag whose next token is user-supplied content
/// (not a filesystem path) and must not be path-normalised.
fn is_content_flag(tok: &str) -> bool {
    matches!(tok, "--insert" | "--find" | "--replace" | "--append")
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::types::{Warning, WarningSink};

    #[test]
    fn argv_normalizer_quoted_string_replaced() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let shape = norm.normalize_argv(
            &[
                "write".to_string(),
                "src/main.rs".to_string(),
                "\"hello world\"".to_string(),
            ],
            None,
            "ses_test",
            &mut sink,
        );
        assert!(
            shape.contains("<str>"),
            "quoted string should become <str>; got: {shape}"
        );
    }

    #[test]
    fn argv_normalizer_tmp_path_replaced() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let shape = norm.normalize_argv(
            &["read".to_string(), "/tmp/fixture.rs".to_string()],
            None,
            "ses_test",
            &mut sink,
        );
        assert!(
            shape.contains("<tmp>"),
            "tmp path should become <tmp>; got: {shape}"
        );
    }

    #[test]
    fn argv_normalizer_warns_once_per_session_for_missing_project() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        // Two calls with different relative paths, same session, no project_path
        let _ = norm.normalize_argv(
            &["read".to_string(), "./src/main.rs".to_string()],
            None,
            "ses_warn",
            &mut sink,
        );
        let _ = norm.normalize_argv(
            &["read".to_string(), "./src/lib.rs".to_string()],
            None,
            "ses_warn",
            &mut sink,
        );
        // Should warn exactly once for that session
        let all_warnings = sink.into_inner();
        let session_warnings: Vec<_> = all_warnings
            .iter()
            .filter(|w| {
                matches!(
                    w,
                    Warning::NormalizerBasenameFallback { session, .. }
                        if session.as_str() == "ses_warn"
                )
            })
            .collect();
        assert_eq!(
            session_warnings.len(),
            1,
            "should warn once per session; got: {:?}",
            all_warnings
        );
    }

    // F2a: range spec (path:N-M) must not produce CanonicalizeFailed.
    // The `:N-M` suffix causes canonicalize to fail on a nonexistent path with
    // the range appended. Strip the suffix before resolving.
    #[test]
    fn argv_normalizer_range_spec_no_canonicalize_warning() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        // /nonexistent/a.rs does not exist but the range suffix must be stripped
        // before canonicalize is called, so no CanonicalizeFailed should appear.
        let _shape = norm.normalize_argv(
            &["read".to_string(), "/nonexistent/a.rs:155-195".to_string()],
            None,
            "ses_range",
            &mut sink,
        );
        let all_warnings = sink.into_inner();
        let canon_warns: Vec<_> = all_warnings
            .iter()
            .filter(|w| matches!(w, Warning::CanonicalizeFailed { .. }))
            .collect();
        assert!(
            canon_warns.is_empty(),
            "range spec should not produce CanonicalizeFailed; got: {:?}",
            canon_warns
        );
    }

    // F2b: flag-value tokens (content of --insert / --find / --replace /
    // --append) must NOT be path-normalised. A token like `// crate entry`
    // passes looks_like_path() because it contains `/`, but it is not a path.
    #[test]
    fn argv_normalizer_flag_value_not_path_normalised() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        // `// crate entry` comes after --insert; it must NOT be canonicalized.
        let shape = norm.normalize_argv(
            &[
                "write".to_string(),
                "src/main.rs:10".to_string(),
                "--insert".to_string(),
                "// crate entry".to_string(),
            ],
            None,
            "ses_flag",
            &mut sink,
        );
        let all_warnings = sink.into_inner();
        // Must produce no CanonicalizeFailed for the flag value.
        let canon_warns: Vec<_> = all_warnings
            .iter()
            .filter(|w| matches!(w, Warning::CanonicalizeFailed { .. }))
            .collect();
        assert!(
            canon_warns.is_empty(),
            "flag value should not produce CanonicalizeFailed; got: {:?}",
            canon_warns
        );
        // §6.1: flag value must be normalized to <str>, not appear verbatim.
        assert!(
            shape.contains("<str>"),
            "flag value should become <str>; got: {shape}"
        );
        assert!(
            !shape.contains("// crate entry"),
            "raw flag value must not appear in shape; got: {shape}"
        );
    }
}
