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

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use ignore::WalkBuilder;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::search_report::{
    FileMatches as ReportFileMatches, SearchMatch, SearchReport,
};
use o8v_fs::{ContainmentRoot, FsConfig};
use regex::{Regex, RegexBuilder};

// ─── Args ────────────────────────────────────────────────────────────────────

fn parse_nonzero_limit(s: &str) -> Result<usize, String> {
    let n: usize = s.parse().map_err(|_| format!("invalid limit: {s:?}"))?;
    if n == 0 {
        return Err("--limit must be 1 or greater".to_string());
    }
    Ok(n)
}

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
    #[arg(long, default_value = "20", value_parser = parse_nonzero_limit)]
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
    pub total_matches_before_limit: usize, // how many matches existed before per-file cap
    pub total_files: usize,                // files with matches shown in results (<= limit)
    pub files_searched: usize,
    pub files_skipped: usize,
    pub files_skipped_by_reason: BTreeMap<String, usize>,
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
        return Err("pattern cannot be empty".to_string());
    }
    RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()
        .map_err(|e| format!("invalid regex pattern: {e}"))
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
) -> Option<String> {
    let guarded = match o8v_fs::safe_read(path, containment, config) {
        Ok(f) => f,
        Err(e) => {
            let err_str = e.to_string();
            let reason = if err_str.contains("Permission denied")
                || err_str.contains("permission denied")
                || err_str.contains("os error 13")
            {
                "permission_denied"
            } else if err_str.contains("invalid utf")
                || err_str.contains("not valid UTF")
                || err_str.contains("stream did not contain valid UTF")
                || err_str.contains("InvalidUtf8")
                || err_str.contains("invalid utf-8")
                || err_str.contains("invalid utf8")
            {
                "binary"
            } else {
                "io_error"
            };
            tracing::debug!("cannot read {}: {e}", path.display());
            result.files_skipped += 1;
            return Some(reason.to_string());
        }
    };

    let content = guarded.content();

    // Reject files that contain NUL bytes — safe_read returns a String, so
    // invalid UTF-8 is already rejected. NUL bytes in otherwise-valid UTF-8
    // indicate binary content (e.g. embedded null-terminated strings).
    if content.contains('\0') {
        result.files_skipped += 1;
        return Some("binary".to_string());
    }

    let lines: Vec<&str> = content.lines().collect();
    let rel_path = crate::path_util::relative_to(root, path);

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

                let text = Some(truncate_line(line));

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

    None
}

/// Count matches in a file for the true-total denominator only.
///
/// Called for files that arrive after the limit has already been reached.
/// Does not add to `result.files` — only updates `total_matches_before_limit`
/// and `total_files` so the renderer can show the correct "of N" total.
fn count_file_matches(path: &Path, regex: &Regex, result: &mut SearchResult) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    if content.contains('\0') {
        return;
    }
    let mut file_had_match = false;
    for line in content.lines() {
        if regex.is_match(line) {
            file_had_match = true;
            result.total_matches_before_limit += 1;
        }
    }
    if file_had_match {
        result.total_files += 1;
    }
}

/// Returns `true` when stdin is a named pipe (FIFO), meaning the caller piped
/// data in. Returns `false` for TTYs, `/dev/null`, regular files, and any
/// other non-pipe fd — including how test harnesses spawn subprocesses.
fn stdin_is_pipe() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        match std::fs::metadata("/dev/stdin") {
            Ok(m) => m.file_type().is_fifo(),
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Run the full search and return a `SearchResult`.
pub fn do_search(args: &Args, ctx: &CommandContext) -> Result<SearchResult, String> {
    if args.path.is_none() && stdin_is_pipe() {
        eprintln!("note: stdin not consumed by 8v search; scanning cwd");
    }

    let regex = build_regex(args)?;

    let workspace = ctx
        .extensions
        .get::<crate::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    let root = match args.path.as_deref() {
        Some(p) => workspace.resolve(p),
        None => workspace.as_path().to_path_buf(),
    };

    // Canonicalize so we can compute relative paths later.
    let root = root.canonicalize().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("8v: not found: {}", root.display())
        } else {
            format!("cannot access path '{}': {e}", root.display())
        }
    })?;

    // When the search root is outside the workspace containment boundary (e.g. an
    // absolute path passed by the caller), we must anchor containment at the
    // search root itself. Otherwise `safe_read` rejects every file inside it
    // with a ContainmentViolation and the search silently returns no matches.
    let search_containment: ContainmentRoot;
    let containment = if root.starts_with(workspace.containment().as_path()) {
        workspace.containment()
    } else {
        search_containment = ContainmentRoot::new(&root)
            .map_err(|e| format!("invalid search path '{}': {e}", root.display()))?;
        &search_containment
    };
    let config = FsConfig::default();

    let mut result = SearchResult {
        files: Vec::new(),
        total_matches: 0,
        total_matches_before_limit: 0,
        total_files: 0,
        files_searched: 0,
        files_skipped: 0,
        files_skipped_by_reason: BTreeMap::new(),
        truncated: false,
    };

    let mut seen: HashSet<(String, String)> = HashSet::new();

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

        if !crate::path_util::matches_extension(path, args.extension.as_deref()) {
            continue;
        }

        result.files_searched += 1;

        // Once the limit is reached, only count matches for the true-total
        // denominator — do not add more files to result.files.
        if result.truncated {
            count_file_matches(path, &regex, &mut result);
            continue;
        }

        let skip_reason =
            search_file_contents(path, &root, containment, &config, &regex, args, &mut result);
        if let Some(reason) = skip_reason {
            let rel_path = crate::path_util::relative_to(&root, path);
            *result
                .files_skipped_by_reason
                .entry(reason.clone())
                .or_insert(0) += 1;
            // Emit stderr for real errors (not binary), deduped by (reason, path).
            if reason != "binary" && seen.insert((reason.clone(), rel_path.clone())) {
                let reason_display = match reason.as_str() {
                    "permission_denied" => "permission denied",
                    "not_utf8" => "not UTF-8",
                    _ => "I/O error",
                };
                eprintln!("error: search: {reason_display}: {rel_path}");
            }
        }

        // If this file pushed us over the limit, pop it from results so only
        // `args.limit` files are shown. Keep total_files as the true file count.
        if result.total_files > args.limit {
            result.truncated = true;
            if let Some(last) = result.files.pop() {
                result.total_matches -= last.matches.len();
            }
            result.total_files -= 1;
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
            files_skipped_by_reason: result.files_skipped_by_reason,
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

/// True when any file was skipped due to a read error (permission denied,
/// I/O failure, etc.). Binary content is filtered earlier and does NOT
/// increment `files_skipped`, so a non-zero count means a real failure the
/// agent should be able to detect via the exit code.
pub(crate) fn had_read_errors(report: &SearchReport) -> bool {
    report
        .files_skipped_by_reason
        .iter()
        .filter(|(k, _)| k.as_str() != "binary")
        .any(|(_, &v)| v > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::render::RenderConfig;

    fn empty_report_with_reason(reason: &str, count: usize) -> SearchReport {
        let mut by_reason = BTreeMap::new();
        if count > 0 {
            by_reason.insert(reason.to_string(), count);
        }
        SearchReport {
            pattern: String::new(),
            files: Vec::new(),
            total_matches: 0,
            total_matches_before_limit: 0,
            total_files: 0,
            files_searched: 0,
            files_skipped: count,
            files_skipped_by_reason: by_reason,
            truncated: false,
            file_mode: false,
            context: Some(2),
            render_config: RenderConfig {
                limit: Some(20),
                verbose: false,
                color: false,
                page: 1,
                errors_first: false,
            },
        }
    }

    #[test]
    fn had_read_errors_false_when_zero_skipped() {
        assert!(!had_read_errors(&empty_report_with_reason(
            "permission_denied",
            0
        )));
    }

    #[test]
    fn had_read_errors_true_when_permission_denied() {
        assert!(had_read_errors(&empty_report_with_reason(
            "permission_denied",
            1
        )));
        assert!(had_read_errors(&empty_report_with_reason("io_error", 42)));
    }

    #[test]
    fn had_read_errors_false_when_only_binary_skipped() {
        // Binary skips are expected noise — they must not flip the exit code.
        assert!(!had_read_errors(&empty_report_with_reason("binary", 1)));
        assert!(!had_read_errors(&empty_report_with_reason("binary", 999)));
    }
}
