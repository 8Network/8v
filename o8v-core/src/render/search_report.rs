// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Report type for `8v search` — content and file search results.

use super::output::Output;

// ─── Per-match context ────────────────────────────────────────────────────────

/// A single line match with optional text and context lines.
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    /// Match text. None in compact mode (no -C flag).
    pub content: Option<String>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

// ─── Per-file grouping ────────────────────────────────────────────────────────

/// All matches within one file.
pub struct FileMatches {
    pub path: String,
    pub matches: Vec<SearchMatch>,
}

// ─── Top-level report ─────────────────────────────────────────────────────────

/// Result of a search operation.
pub struct SearchReport {
    pub pattern: String,
    /// Per-file grouped matches. Empty when no matches found.
    pub files: Vec<FileMatches>,
    /// Total individual match count (after per-file limits).
    pub total_matches: usize,
    /// Total matches before per-file limits were applied.
    pub total_matches_before_limit: usize,
    /// Number of files that contained at least one match.
    pub total_files: usize,
    /// Total files examined (including those with no matches).
    pub files_searched: usize,
    /// Files skipped due to read errors or binary content.
    pub files_skipped: usize,
    /// True when file-limit or per-file-match-limit was hit.
    pub truncated: bool,
    /// True when --files mode was used (filename search).
    pub file_mode: bool,
    /// The context flag value from args (-C N). None = compact, Some(0) = text only, Some(N) = context.
    pub context: Option<usize>,
}

// ─── Rendering ────────────────────────────────────────────────────────────────

impl super::Renderable for SearchReport {
    fn render_plain(&self) -> Output {
        if self.files.is_empty() {
            return Output::new("no matches found".to_string());
        }

        let mut out = String::new();

        if self.file_mode {
            for file_match in &self.files {
                out.push_str(&file_match.path);
                out.push('\n');
            }
        } else {
            match self.context {
                None => {
                    // Compact mode: "file:line"
                    for fm in &self.files {
                        for m in &fm.matches {
                            out.push_str(&format!("{}:{}\n", fm.path, m.line));
                        }
                    }
                }
                Some(0) => {
                    // Text-only mode: "file:line: text"
                    for fm in &self.files {
                        for m in &fm.matches {
                            if let Some(ref text) = m.content {
                                out.push_str(&format!("{}:{}: {}\n", fm.path, m.line, text));
                            } else {
                                out.push_str(&format!("{}:{}\n", fm.path, m.line));
                            }
                        }
                    }
                }
                Some(_) => {
                    // Context mode: "file:line: text" + context before/after
                    for fm in &self.files {
                        for m in &fm.matches {
                            if let Some(ref text) = m.content {
                                out.push_str(&format!("{}:{}: {}\n", fm.path, m.line, text));
                            } else {
                                out.push_str(&format!("{}:{}\n", fm.path, m.line));
                            }
                            for ctx in &m.context_before {
                                out.push_str(&format!("  > {}\n", ctx));
                            }
                            for ctx in &m.context_after {
                                out.push_str(&format!("  < {}\n", ctx));
                            }
                        }
                    }
                }
            }
        }

        out.push('\n');

        if self.file_mode {
            out.push_str(&format!(
                "Found {} files matching {:?}",
                self.total_files, self.pattern
            ));
        } else {
            let truncated_per_file = self.total_matches_before_limit > self.total_matches;
            if self.truncated || truncated_per_file {
                out.push_str(&format!(
                    "Found {} of {} matches in {} files (searched {} files",
                    self.total_matches,
                    self.total_matches_before_limit,
                    self.total_files,
                    self.files_searched
                ));
            } else {
                out.push_str(&format!(
                    "Found {} matches in {} files (searched {} files",
                    self.total_matches, self.total_files, self.files_searched
                ));
            }
            if self.files_skipped > 0 {
                out.push_str(&format!(", {} skipped", self.files_skipped));
            }
            if self.truncated || truncated_per_file {
                out.push_str(", results truncated — use --limit and --max-per-file for more");
            }
            out.push(')');
        }

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        let json = if self.file_mode {
            let files: Vec<&str> = self.files.iter().map(|f| f.path.as_str()).collect();
            serde_json::json!({
                "files": files,
                "total": self.total_files,
                "files_searched": self.files_searched,
                "files_skipped": self.files_skipped,
                "truncated": self.truncated,
            })
        } else {
            let files: Vec<serde_json::Value> = self
                .files
                .iter()
                .map(|fm| {
                    let matches: Vec<serde_json::Value> = fm
                        .matches
                        .iter()
                        .map(|m| {
                            let mut obj = serde_json::json!({
                                "line": m.line,
                            });
                            if let Some(ref text) = m.content {
                                obj["text"] = serde_json::Value::String(text.clone());
                            }
                            if !m.context_before.is_empty() {
                                obj["context_before"] = serde_json::Value::Array(
                                    m.context_before
                                        .iter()
                                        .map(|s| serde_json::Value::String(s.clone()))
                                        .collect(),
                                );
                            }
                            if !m.context_after.is_empty() {
                                obj["context_after"] = serde_json::Value::Array(
                                    m.context_after
                                        .iter()
                                        .map(|s| serde_json::Value::String(s.clone()))
                                        .collect(),
                                );
                            }
                            obj
                        })
                        .collect();
                    serde_json::json!({
                        "path": fm.path,
                        "matches": matches,
                    })
                })
                .collect();

            serde_json::json!({
                "files": files,
                "total_matches": self.total_matches,
                "total_matches_before_limit": self.total_matches_before_limit,
                "total_files": self.total_files,
                "files_searched": self.files_searched,
                "files_skipped": self.files_skipped,
                "truncated": self.truncated,
            })
        };

        let s = match serde_json::to_string_pretty(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }

    fn render_human(&self) -> Output {
        self.render_plain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    fn sample() -> SearchReport {
        SearchReport {
            pattern: "TODO".to_string(),
            files: vec![
                FileMatches {
                    path: "src/main.rs".to_string(),
                    matches: vec![SearchMatch {
                        path: "src/main.rs".to_string(),
                        line: 42,
                        content: Some("// TODO: fix this".to_string()),
                        context_before: vec![],
                        context_after: vec![],
                    }],
                },
                FileMatches {
                    path: "src/lib.rs".to_string(),
                    matches: vec![SearchMatch {
                        path: "src/lib.rs".to_string(),
                        line: 10,
                        content: Some("// TODO: refactor".to_string()),
                        context_before: vec![],
                        context_after: vec![],
                    }],
                },
            ],
            total_matches: 2,
            total_matches_before_limit: 2,
            total_files: 2,
            files_searched: 50,
            files_skipped: 0,
            truncated: false,
            file_mode: false,
            context: Some(0),
        }
    }

    #[test]
    fn plain_format() {
        let out = sample().render_plain();
        assert!(out.as_str().contains("src/main.rs:42: // TODO: fix this"));
        assert!(out.as_str().contains("src/lib.rs:10: // TODO: refactor"));
    }

    #[test]
    fn plain_truncated() {
        let mut r = sample();
        r.truncated = true;
        let out = r.render_plain();
        assert!(out.as_str().contains("truncated"));
    }

    #[test]
    fn json_valid() {
        let out = sample().render_json();
        let v: serde_json::Value = serde_json::from_str(out.as_str()).unwrap();
        assert_eq!(v["total_matches"], 2);
        assert_eq!(v["files"].as_array().unwrap().len(), 2);
        assert_eq!(v["truncated"], false);
    }

    #[test]
    fn plain_empty() {
        let r = SearchReport {
            pattern: "xyz".to_string(),
            files: vec![],
            total_matches: 0,
            total_matches_before_limit: 0,
            total_files: 0,
            files_searched: 100,
            files_skipped: 0,
            truncated: false,
            file_mode: false,
            context: None,
        };
        let out = r.render_plain();
        assert_eq!(out.as_str(), "no matches found");
    }

    #[test]
    fn compact_mode_no_text() {
        let mut r = sample();
        r.context = None;
        // In compact mode, content is ignored, format is "file:line"
        let out = r.render_plain();
        assert!(out.as_str().contains("src/main.rs:42\n"));
        assert!(!out.as_str().contains("// TODO"));
    }

    #[test]
    fn file_mode_lists_paths() {
        let r = SearchReport {
            pattern: "main".to_string(),
            files: vec![FileMatches {
                path: "src/main.rs".to_string(),
                matches: vec![],
            }],
            total_matches: 0,
            total_matches_before_limit: 0,
            total_files: 1,
            files_searched: 5,
            files_skipped: 0,
            truncated: false,
            file_mode: true,
            context: None,
        };
        let out = r.render_plain();
        assert!(out.as_str().contains("src/main.rs\n"));
        assert!(out.as_str().contains("Found 1 files matching"));
    }
}
