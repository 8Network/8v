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
        if !looks_like_path(token) {
            return token.to_string();
        }
        let path = Path::new(token);
        if is_tempdir(path) {
            return "<tmp>".to_string();
        }
        if path.is_absolute() {
            let canonical = match std::fs::canonicalize(path) {
                Ok(c) => c,
                Err(e) => {
                    warnings.push(Warning::CanonicalizeFailed {
                        path: token.to_string(),
                        reason: e.to_string(),
                    });
                    path.to_path_buf()
                }
            };
            if let Some(project) = canonical_project {
                if canonical.starts_with(project) {
                    let rel = match canonical.strip_prefix(project) {
                        Ok(r) => r.to_path_buf(),
                        Err(_) => canonical.clone(),
                    };
                    return normalize_separators(&rel.to_string_lossy());
                }
            }
            return "<abs>".to_string();
        }
        if canonical_project.is_none() {
            if !self.warned_sessions.contains(session_id) {
                self.warned_sessions.insert(session_id.to_string());
                warnings.push(Warning::NormalizerBasenameFallback {
                    session: SessionId::from_raw_unchecked(session_id.to_string()),
                    path: token.to_string(),
                });
            }
            return path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| token.to_string());
        }
        normalize_separators(token)
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
        let normalized: Vec<String> = argv
            .iter()
            .map(|tok| {
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
}
