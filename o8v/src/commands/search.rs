// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `search` command — token-efficient content search for AI agents.
//!
//! - `8v search <pattern> [path]` — search file contents (default: current directory)
//! - `8v search <pattern> --files` — search file names instead of contents
//! - `8v search <pattern> -i` — case-insensitive
//! - `8v search <pattern> -e rs` — filter by extension
//! - `8v search <pattern> -C 0` — show match text only (no context)
//! - `8v search <pattern> -C 2` — match text + 2 context lines before/after (default: no context)
//! - `8v search <pattern> --limit 20` — max files with matches (default: 20)
//! - `8v search <pattern> --json` — JSON output (nested by file)

use ignore::WalkBuilder;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::search_report::{
    FileMatches as ReportFileMatches, SearchMatch, SearchReport,
};
use o8v_fs::{ContainmentRoot, FsConfig};
use regex::{Regex, RegexBuilder};
use std::path::Path;

// ─── Args ────────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Regex pattern to search for
    pub pattern: String,

    /// Directory or file to search in (default: current directory)
    pub path: Option<String>,

    /// Case-insensitive search
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Filter by file extension (e.g. rs, py, ts)
    #[arg(short = 'e', long = "ext")]
    pub extension: Option<String>,

    /// Number of context lines before and after each match. If not provided, compact mode (no text).
    /// -C 0 = text only, -C N = text + N context lines
    #[arg(short = 'C', long = "context")]
    pub context: Option<usize>,

    /// Maximum number of files with matches to return (default: 20)
    #[arg(long, default_value = "20")]
    pub limit: usize,

    /// Maximum matches per file (default: 10)
    #[arg(long, default_value = "10")]
    pub max_per_file: usize,

    /// Search file names instead of contents
    #[arg(long)]
    pub files: bool,

    /// Page number for paginated output (default: 1)
    #[arg(long, default_value = "1")]
    pub page: usize,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

// ─── Data structures ─────────────────────────────────────────────────────────

pub struct Match {
    pub line: usize,
    pub text: Option<String>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

pub struct FileMatches {
    pub path: String,
    pub matches: Vec<Match>,
}

pub struct SearchResult {
    pub files: Vec<FileMatches>,
    pub total_matches: usize,
    pub total_matches_before_limit: usize, // how many matches existed before limits applied
    pub total_files: usize,
    pub files_searched: usize,
    pub files_skipped: usize,
    pub truncated: bool,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

const MAX_LINE_LEN: usize = 200;

/// Truncate a line to at most MAX_LINE_LEN chars, respecting UTF-8 boundaries.
fn truncate_line(s: &str) -> String {
    if s.chars().count() <= MAX_LINE_LEN {
        s.to_owned()
    } else {
        let mut end = 0;
        for (char_count, c) in s.chars().enumerate() {
            if char_count >= MAX_LINE_LEN {
                break;
            }
            end += c.len_utf8();
        }
        format!("{}…", &s[..end])
    }
}

// ─── Core implementation ─────────────────────────────────────────────────────

/// Build and compile the regex from args.
fn build_regex(args: &Args) -> Result<Regex, String> {
    if args.pattern.is_empty() {
        return Err("error: pattern cannot be empty".to_string());
    }
    RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()
        .map_err(|e| format!("error: invalid regex pattern: {e}"))
}

/// Search file contents for regex matches.
///
/// Uses `o8v_fs::safe_read` for the full guard pipeline: canonicalize,
/// containment check, symlink rejection, size limits, BOM stripping.
/// Binary files (invalid UTF-8, NUL bytes) are skipped with a warning.
fn search_file_contents(
    path: &Path,
    root: &Path,
    containment: &ContainmentRoot,
    config: &FsConfig,
    regex: &Regex,
    args: &Args,
    result: &mut SearchResult,
) {
    let guarded = match o8v_fs::safe_read(path, containment, config) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!("cannot read {}: {e}", path.display());
            result.files_skipped += 1;
            return;
        }
    };

    let content = guarded.content();

    // Reject files that contain NUL bytes — safe_read returns a String, so
    // invalid UTF-8 is already rejected. NUL bytes in otherwise-valid UTF-8
    // indicate binary content (e.g. embedded null-terminated strings).
    if content.contains('\0') {
        return;
    }

    let lines: Vec<&str> = content.lines().collect();
    let rel_path = crate::util::relative_to(root, path);

    let mut file_matches = Vec::new();
    let mut file_had_match = false;

    for (line_idx, &line) in lines.iter().enumerate() {
        if regex.is_match(line) {
            file_had_match = true;
            result.total_matches_before_limit += 1;

            if file_matches.len() < args.max_per_file {
                result.total_matches += 1;

                // Build context based on mode determined by args.context:
                // None = compact mode (no text, no context)
                // Some(0) = text only (no context)
                // Some(N) = text + context
                let context_lines = args.context.unwrap_or(0);
                let context_before = if args.context.is_some() {
                    let before_start = line_idx.saturating_sub(context_lines);
                    lines[before_start..line_idx]
                        .iter()
                        .map(|l| truncate_line(l))
                        .collect()
                } else {
                    Vec::new()
                };

                let context_after = if args.context.is_some() {
                    let after_end = (line_idx + 1 + context_lines).min(lines.len());
                    lines[(line_idx + 1)..after_end]
                        .iter()
                        .map(|l| truncate_line(l))
                        .collect()
                } else {
                    Vec::new()
                };

                let text = if args.context.is_some() {
                    Some(truncate_line(line))
                } else {
                    None
                };

                file_matches.push(Match {
                    line: line_idx + 1,
                    text,
                    context_before,
                    context_after,
                });
            }
        }
    }

    if file_had_match {
        result.total_files += 1;
        result.files.push(FileMatches {
            path: rel_path,
            matches: file_matches,
        });
    }
}

/// Search file names for pattern matches.
fn search_file_names(path: &Path, root: &Path, regex: &Regex, result: &mut SearchResult) {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return,
    };

    if regex.is_match(file_name) {
        let rel_path = crate::util::relative_to(root, path);
        result.files.push(FileMatches {
            path: rel_path,
            matches: vec![Match {
                line: 0,
                text: None,
                context_before: Vec::new(),
                context_after: Vec::new(),
            }],
        });
        result.total_matches += 1;
        result.total_files += 1;
    }
}

/// Run the full search and return a `SearchResult`.
pub fn do_search(args: &Args, ctx: &CommandContext) -> Result<SearchResult, String> {
    let regex = build_regex(args)?;

    let workspace = ctx
        .extensions
        .get::<o8v::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    let root = match args.path.as_deref() {
        Some(p) => workspace.resolve(p),
        None => workspace.as_path().to_path_buf(),
    };

    // Canonicalize so we can compute relative paths later.
    let root = root
        .canonicalize()
        .map_err(|e| format!("error: cannot access path '{}': {e}", root.display()))?;

    let containment = workspace.containment();
    let config = FsConfig::default();

    let mut result = SearchResult {
        files: Vec::new(),
        total_matches: 0,
        total_matches_before_limit: 0,
        total_files: 0,
        files_searched: 0,
        files_skipped: 0,
        truncated: false,
    };

    let walker = WalkBuilder::new(&root)
        .standard_filters(true) // respects .gitignore, hidden, etc.
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("cannot walk directory entry: {e}");
                continue;
            }
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if !crate::util::matches_extension(path, args.extension.as_deref()) {
            continue;
        }

        result.files_searched += 1;

        // Check file limit before processing
        if result.total_files >= args.limit {
            result.truncated = true;
            break;
        }

        if args.files {
            search_file_names(path, &root, &regex, &mut result);
        } else {
            search_file_contents(path, &root, containment, &config, &regex, args, &mut result);
        }
    }

    Ok(result)
}

// ── Command trait impl ──────────────────────────────────────────────────

pub struct SearchCommand {
    pub args: Args,
}

impl Command for SearchCommand {
    type Report = SearchReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        let result = do_search(&self.args, ctx).map_err(CommandError::Execution)?;

        let files: Vec<ReportFileMatches> = result
            .files
            .into_iter()
            .map(|fm| ReportFileMatches {
                path: fm.path.clone(),
                matches: fm
                    .matches
                    .into_iter()
                    .map(|m| SearchMatch {
                        path: fm.path.clone(),
                        line: m.line,
                        content: m.text,
                        context_before: m.context_before,
                        context_after: m.context_after,
                    })
                    .collect(),
            })
            .collect();

        Ok(SearchReport {
            pattern: self.args.pattern.clone(),
            files,
            total_matches: result.total_matches,
            total_matches_before_limit: result.total_matches_before_limit,
            total_files: result.total_files,
            files_searched: result.files_searched,
            files_skipped: result.files_skipped,
            truncated: result.truncated,
            file_mode: self.args.files,
            context: self.args.context,
            render_config: o8v_core::render::RenderConfig {
                limit: Some(self.args.limit),
                verbose: false,
                color: !self.args.format.no_color && std::env::var_os("NO_COLOR").is_none(),
                page: self.args.page,
                errors_first: false,
            },
        })
    }
}
