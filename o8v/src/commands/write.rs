// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `write` command — line-based file writing.
//!
//! - `8v write path:12 "content"` — replace line 12
//! - `8v write path:12-15 "content"` — replace lines 12-15
//! - `8v write path:12 --insert "content"` — insert before line 12
//! - `8v write path:12-15 --delete` — delete lines 12-15
//! - `8v write path "content"` — create new file
//! - `8v write path --append "content"` — append to file
//! - `8v write path --find "old" --replace "new"` — find and replace
//! - `8v write path --json` — structured JSON output

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// File path, optionally with line number or range (path:N or path:N-M)
    pub path: String,

    /// Content to write (or old text for --find mode)
    pub content: Option<String>,

    /// Insert before line instead of replacing
    #[arg(long)]
    pub insert: bool,

    /// Delete lines instead of replacing
    #[arg(long)]
    pub delete: bool,

    /// Append to end of file
    #[arg(long)]
    pub append: bool,

    /// Find text (used with --replace)
    #[arg(long)]
    pub find: Option<String>,

    /// Replace text (used with --find)
    #[arg(long)]
    pub replace: Option<String>,

    /// Replace all occurrences in find mode
    #[arg(long)]
    pub all: bool,

    /// Force overwrite existing file (create mode only)
    #[arg(long)]
    pub force: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

impl Args {
    pub fn audience(&self) -> o8v_core::render::Audience {
        if self.json {
            o8v_core::render::Audience::Machine
        } else {
            o8v_core::render::Audience::Human
        }
    }
}

// ─── Operation Type ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum WriteOperation {
    ReplaceLine {
        line: usize,
        content: String,
    },
    ReplaceRange {
        start: usize,
        end: usize,
        content: String,
    },
    InsertBefore {
        line: usize,
        content: String,
    },
    DeleteLines {
        start: usize,
        end: usize,
    },
    CreateFile {
        content: String,
        force: bool,
    },
    AppendToFile {
        content: String,
    },
    FindReplace {
        find: String,
        replace: String,
        all: bool,
    },
}

// ─── Path Parsing ────────────────────────────────────────────────────────────

/// Parse `path:N-M` into (path, Some((N, M))) or `path:N` into (path, Some((N, N))) or (path, None).
///
/// Only splits on the last colon followed by digits to avoid splitting
/// Windows-style paths (C:\...) on the drive colon.
///
/// Returns Err when the line part parses as a number that is 0.
fn parse_path_line(input: &str) -> Result<(String, Option<(usize, usize)>), String> {
    if let Some(colon_pos) = input.rfind(':') {
        let line_part = &input[colon_pos + 1..];

        // Try parsing as range (N-M)
        if let Some(dash_pos) = line_part.find('-') {
            let start_str = &line_part[..dash_pos];
            let end_str = &line_part[dash_pos + 1..];
            match (start_str.parse::<usize>(), end_str.parse::<usize>()) {
                (Ok(start), Ok(end)) => {
                    if start == 0 || end == 0 {
                        return Err("Error: line numbers are 1-indexed, got 0".to_string());
                    }
                    if start <= end {
                        return Ok((input[..colon_pos].to_string(), Some((start, end))));
                    } else {
                        return Err(format!(
                            "error: invalid line range :{line_part} — start must be <= end"
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "error: invalid line range \":{line_part}\" — expected :N or :N-M where N,M are positive integers"
                    ));
                }
            }
        }

        // Try parsing as single line (N)
        if let Ok(line) = line_part.parse::<usize>() {
            if line == 0 {
                return Err("Error: line numbers are 1-indexed, got 0".to_string());
            }
            return Ok((input[..colon_pos].to_string(), Some((line, line))));
        }
    }
    Ok((input.to_string(), None))
}

// ─── Argument Validation ────────────────────────────────────────────────────

fn validate_args(args: &Args) -> Result<WriteOperation, String> {
    let (path_str, line_range) = parse_path_line(&args.path)?;
    let _ = path_str; // used later in write_to_file

    // Count mutually exclusive flags
    let mode_count = (args.insert as u8)
        + (args.delete as u8)
        + (args.append as u8)
        + (args.find.is_some() as u8)
        + (args.force as u8);

    if mode_count > 1 {
        return Err(
            "Error: cannot combine --insert, --delete, --append, --find, and --force\n\
             Usage: 8v write <path>:<line> --delete\n\
             Usage: 8v write <path>:<line> --insert \"content\"\n\
             Usage: 8v write <path> --append \"content\""
                .to_string(),
        );
    }

    // Find + replace mode
    if args.find.is_some() || args.replace.is_some() {
        if args.find.is_none() || args.replace.is_none() {
            return Err("Error: --find and --replace must both be provided\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        if line_range.is_some() {
            return Err("Error: line numbers cannot be used with --find mode\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        let find = args.find.clone().unwrap();
        if find.is_empty() {
            return Err("Error: --find pattern must not be empty\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        return Ok(WriteOperation::FindReplace {
            find,
            replace: args.replace.clone().unwrap(),
            all: args.all,
        });
    }

    // Delete mode
    if args.delete {
        let (start, end) = line_range.ok_or(
            "Error: delete requires a line number or range\n\
             Usage: 8v write <path>:<line> --delete\n\
             Usage: 8v write <path>:<start>-<end> --delete",
        )?;
        if args.content.is_some() {
            return Err("Error: content argument not allowed with --delete\n\
                 Usage: 8v write <path>:<line> --delete"
                .to_string());
        }
        return Ok(WriteOperation::DeleteLines { start, end });
    }

    // Append mode
    if args.append {
        if line_range.is_some() {
            return Err("Error: line numbers cannot be used with --append\n\
                 Usage: 8v write <path> --append \"content\""
                .to_string());
        }
        let content = args.content.clone().ok_or(
            "Error: content required for --append\n\
             Usage: 8v write <path> --append \"content\"",
        )?;
        if content.is_empty() {
            return Err(
                "error: content cannot be empty for --append — provide non-empty content or omit the command"
                    .to_string(),
            );
        }
        return Ok(WriteOperation::AppendToFile { content });
    }

    // Insert mode
    if args.insert {
        let (line, _) = line_range.ok_or(
            "Error: insert requires a line number\n\
             Usage: 8v write <path>:<line> --insert \"content\"",
        )?;
        let content = args.content.clone().ok_or(
            "Error: content required for --insert\n\
             Usage: 8v write <path>:<line> --insert \"content\"",
        )?;
        if content.is_empty() {
            return Err(
                "error: content cannot be empty for replace/insert — use --delete to remove lines, or provide non-empty content"
                    .to_string(),
            );
        }
        return Ok(WriteOperation::InsertBefore { line, content });
    }

    // Replace or create mode
    let content = args.content.clone().ok_or(
        "Error: content required\n\
         Usage: 8v write <path>:<line> \"content\"   (replace line)\n\
         Usage: 8v write <path>:<start>-<end> \"content\"   (replace range)\n\
         Usage: 8v write <path> \"content\"   (create file)",
    )?;

    match line_range {
        Some((start, end)) => {
            if content.is_empty() {
                return Err(
                    "error: content cannot be empty for replace/insert — use --delete to remove lines, or provide non-empty content"
                        .to_string(),
                );
            }
            if start == end {
                Ok(WriteOperation::ReplaceLine {
                    line: start,
                    content,
                })
            } else {
                Ok(WriteOperation::ReplaceRange {
                    start,
                    end,
                    content,
                })
            }
        }
        None => Ok(WriteOperation::CreateFile {
            content,
            force: args.force,
        }),
    }
}

// ─── Line Ending Detection ───────────────────────────────────────────────────

fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Returns true if `content` contains any lone `\r` (i.e., `\r` not immediately followed by `\n`).
fn has_lone_cr(content: &str) -> bool {
    content.chars().zip(content.chars().skip(1).chain(std::iter::once('\0')))
        .any(|(c, next)| c == '\r' && next != '\n')
}

/// Validate that the file's line endings are supported for line-based operations.
///
/// Returns Err for:
/// - Lone `\r` (classic Mac, no `\n` at all) — CE6
/// - Any lone `\r` in a `\n`-terminated file (mid-line `\r`) — HIGH-6
/// - Mixed `\r\n` and lone `\n` — CE19
fn validate_line_endings(content: &str) -> Result<(), String> {
    let has_crlf = content.contains("\r\n");
    let lone_cr = has_lone_cr(content);
    let has_lf = content.contains('\n');

    if lone_cr && !has_lf {
        return Err(
            "error: file uses classic Mac line endings (\\r only) — 8v does not support this format. Convert to \\n or \\r\\n first."
                .to_string(),
        );
    }
    if lone_cr && has_lf {
        return Err(
            "error: file contains carriage return (\\r) characters outside of \\r\\n sequences — normalize line endings first"
                .to_string(),
        );
    }
    if has_crlf && has_lf {
        // Check for standalone \n (not part of \r\n)
        // We know has_lf is true; check if any \n is not preceded by \r
        let has_standalone_lf = content
            .char_indices()
            .any(|(i, c)| c == '\n' && (i == 0 || content.as_bytes()[i - 1] != b'\r'));
        if has_standalone_lf {
            return Err(
                "error: file has mixed line endings (LF and CRLF) — 8v requires consistent line endings. Normalize the file first."
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Validate content provided by the user for line-based operations.
///
/// Rejects any content containing `\r`. The target file's line ending (LF or
/// CRLF) is handled by `detect_line_ending` + `join_lines_with_ending` — the
/// user never needs to put `\r` in content strings.
///
/// Returns Err for:
/// - Lone `\r` (classic Mac) — HIGH-3
/// - `\r\n` (CRLF) — was silently allowed, now rejected
/// - Mixed `\r\n` and lone `\n` — HIGH-2
fn validate_content_line_endings(content: &str) -> Result<(), String> {
    if content.contains('\r') {
        return Err(
            "error: content must use \\n line endings only — do not include \\r".to_string(),
        );
    }
    Ok(())
}

/// Split `content` into lines for insertion, respecting trailing-newline-as-terminator semantics.
///
/// - `"new"` → `["new"]`
/// - `"new\n"` → `["new"]` (trailing `\n` treated as terminator, not a blank line)
/// - `"\n"` → `[""]` (one blank line)
/// - `"\n\n"` → `["", ""]` (two blank lines)
/// - `"a\nb"` → `["a", "b"]`
/// - `"a\nb\n"` → `["a", "b"]`
/// - `"a\n\nb"` → `["a", "", "b"]` (blank middle preserved)
fn content_to_lines(content: &str) -> Vec<String> {
    let stripped = content.strip_suffix("\r\n")
        .or_else(|| content.strip_suffix('\n'))
        .unwrap_or(content);
    stripped.split('\n').map(String::from).collect()
}

fn has_trailing_newline(content: &str) -> bool {
    content.ends_with('\n')
}

/// Split content into lines, stripping line endings and the synthetic trailing
/// empty entry produced by split('\n') when content ends with '\n'.
fn split_lines<'a>(content: &'a str, line_ending: &str, trailing: bool) -> Vec<&'a str> {
    let lines: Vec<&'a str> = if line_ending == "\r\n" {
        content
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect()
    } else {
        content.lines().collect()
    };
    if trailing && line_ending == "\r\n" && lines.last() == Some(&"") {
        lines[..lines.len() - 1].to_vec()
    } else {
        lines
    }
}

fn join_lines_with_ending(lines: &[&str], ending: &str, trailing: bool) -> String {
    let mut result = lines.join(ending);
    if trailing && !lines.is_empty() {
        result.push_str(ending);
    }
    result
}

// ─── Main Run Function ──────────────────────────────────────────────────────

/// Execute the write operation and return a structured [`WriteReport`] with real operation data.
///
/// This is used by [`crate::write_command::WriteCommand`] to satisfy the [`Command`] trait
/// without duplicating the core write logic.
pub(crate) fn write_to_report(
    args: &Args,
    ctx: &o8v_core::command::CommandContext,
) -> Result<o8v_core::render::write_report::WriteReport, String> {
    use o8v_core::render::write_report::{WriteOperation as ReportOp, WriteReport};

    let workspace = ctx
        .extensions
        .get::<o8v::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    let op = validate_args(args)?;
    let (path_str, _) = parse_path_line(&args.path)?;

    let root = workspace.containment();
    let config = o8v_fs::FsConfig::default();
    let path = workspace.resolve(&path_str);

    let report_op: ReportOp = match &op {
        WriteOperation::ReplaceLine { line, content } => {
            let file = o8v_fs::safe_read(&path, root, &config)
                .map_err(|e| format!("Error: failed to read file: {e}"))?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *line > lines.len() {
                return Err(format!(
                    "Error: line {line} does not exist (file has {} lines)",
                    lines.len()
                ));
            }
            let old_lines = vec![lines[line - 1].to_string()];
            let new_content_str = content.clone();

            validate_content_line_endings(content)?;
            let content_lines = content_to_lines(content);
            let content_line_refs: Vec<&str> = content_lines.iter().map(String::as_str).collect();
            let mut new_lines: Vec<&str> = Vec::with_capacity(lines.len() - 1 + content_line_refs.len());
            new_lines.extend_from_slice(&lines[..*line - 1]);
            new_lines.extend_from_slice(&content_line_refs);
            new_lines.extend_from_slice(&lines[*line..]);
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root,new_content_bytes.as_bytes())
                .map_err(|e| format!("Error: failed to write file: {e}"))?;

            ReportOp::Replace {
                old_lines,
                new_content: new_content_str,
            }
        }
        WriteOperation::ReplaceRange {
            start,
            end,
            content,
        } => {
            let file = o8v_fs::safe_read(&path, root, &config)
                .map_err(|e| format!("Error: failed to read file: {e}"))?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *start > lines.len() {
                return Err(format!(
                    "Error: line {start} does not exist (file has {} lines)",
                    lines.len()
                ));
            }
            if *end > lines.len() {
                return Err(format!(
                    "Error: line {end} does not exist (file has {} lines)",
                    lines.len()
                ));
            }

            let old_lines: Vec<String> = lines[start - 1..=end - 1]
                .iter()
                .map(|l| l.to_string())
                .collect();
            let new_content_str = content.clone();

            validate_content_line_endings(content)?;
            let content_lines = content_to_lines(content);
            let content_line_refs: Vec<&str> = content_lines.iter().map(String::as_str).collect();
            let mut new_lines: Vec<&str> = Vec::with_capacity(lines.len() - (end - start) + content_line_refs.len());
            new_lines.extend_from_slice(&lines[..*start - 1]);
            new_lines.extend_from_slice(&content_line_refs);
            new_lines.extend_from_slice(&lines[*end..]);
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root,new_content_bytes.as_bytes())
                .map_err(|e| format!("Error: failed to write file: {e}"))?;

            ReportOp::Replace {
                old_lines,
                new_content: new_content_str,
            }
        }
        WriteOperation::InsertBefore { line, content } => {
            let file = o8v_fs::safe_read(&path, root, &config)
                .map_err(|e| format!("Error: failed to read file: {e}"))?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *line > lines.len() + 1 {
                return Err(format!(
                    "Error: cannot insert at line {line} (file has {} lines)",
                    lines.len()
                ));
            }
            validate_content_line_endings(content)?;
            let content_lines = content_to_lines(content);
            let content_line_refs: Vec<&str> = content_lines.iter().map(String::as_str).collect();
            let mut new_lines: Vec<&str> = lines.clone();
            for (i, cl) in content_line_refs.iter().enumerate() {
                new_lines.insert(line - 1 + i, cl);
            }
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root,new_content_bytes.as_bytes())
                .map_err(|e| format!("Error: failed to write file: {e}"))?;

            ReportOp::Insert {
                content: content.clone(),
            }
        }
        WriteOperation::DeleteLines { start, end } => {
            let file = o8v_fs::safe_read(&path, root, &config)
                .map_err(|e| format!("Error: failed to read file: {e}"))?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *start > lines.len() || *end > lines.len() || start > end {
                return Err(format!(
                    "Error: invalid line range {start}-{end} (file has {} lines)",
                    lines.len()
                ));
            }

            let deleted_lines: Vec<String> = lines[start - 1..=end - 1]
                .iter()
                .map(|l| l.to_string())
                .collect();

            let new_lines: Vec<&str> = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i < start - 1 || *i > end - 1)
                .map(|(_, line)| *line)
                .collect();
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root,new_content_bytes.as_bytes())
                .map_err(|e| format!("Error: failed to write file: {e}"))?;

            ReportOp::Delete { deleted_lines }
        }
        WriteOperation::CreateFile { content, force } => {
            if content.is_empty() {
                return Err(
                    "error: content cannot be empty when creating a file".to_string(),
                );
            }
            if !force {
                match o8v_fs::safe_exists(&path, root) {
                    Ok(true) => {
                        return Err(format!(
                            "Error: file already exists: {}\n  to replace entire file: add --force\n  to replace a range: use {path_str}:<start>-<end> \"<content>\"\n  to find/replace: use --find \"<old>\" --replace \"<new>\"", path.display()
                        ));
                    }
                    Ok(false) => {}
                    Err(e) => {
                        return Err(format!("Error: failed to check if file exists: {e}"));
                    }
                }
            }
            o8v_fs::safe_write(&path, root,content.as_bytes())
                .map_err(|e| format!("Error: failed to create file: {e}"))?;
            let line_count = content.lines().count();
            ReportOp::Create { line_count }
        }
        WriteOperation::AppendToFile { content } => {
            let existing = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!(
                        "error: file does not exist: {path_str}\n  to create a new file with content: 8v write {path_str} \"<content>\""
                    )
                } else {
                    format!("Error: failed to read file: {e}")
                }
            })?;
            let existing_content = existing.content();
            validate_line_endings(existing_content)?;
            validate_content_line_endings(content)?;
            let needs_separator = !existing_content.is_empty()
                && !existing_content.ends_with('\n');
            let appended = if needs_separator {
                let sep = detect_line_ending(existing_content);
                format!("{sep}{content}")
            } else {
                content.clone()
            };
            o8v_fs::safe_append(&path, root, appended.as_bytes())
                .map_err(|e| format!("Error: failed to append to file: {e}"))?;
            ReportOp::Append
        }
        WriteOperation::FindReplace { find, replace, all } => {
            validate_content_line_endings(find)?;
            validate_content_line_endings(replace)?;
            let file = o8v_fs::safe_read(&path, root, &config)
                .map_err(|e| format!("Error: failed to read file: {e}"))?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let new_content = if *all {
                existing_content.replace(find.as_str(), replace.as_str())
            } else {
                existing_content.replacen(find.as_str(), replace.as_str(), 1)
            };

            if new_content == existing_content {
                return Err(format!(
                    "Error: no matches found for {find:?} in {path_str}. \
                     Read the file to find the exact text (whitespace and indentation must match), \
                     then retry with the correct --find value."
                ));
            }

            let count = if *all {
                existing_content.matches(find.as_str()).count()
            } else {
                1
            };
            o8v_fs::safe_write(&path, root,new_content.as_bytes())
                .map_err(|e| format!("Error: failed to write file: {e}"))?;

            ReportOp::FindReplace { count }
        }
    };

    Ok(WriteReport {
        path: path_str,
        operation: report_op,
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bug 2: Empty --find pattern returns error ────────────────────────────

    #[test]
    fn test_empty_find_pattern_returns_error() {
        let args = Args {
            path: "file.txt".to_string(),
            content: None,
            insert: false,
            delete: false,
            append: false,
            find: Some("".to_string()),
            replace: Some("X".to_string()),
            all: false,
            force: false,
            json: false,
        };
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("--find pattern must not be empty"));
    }

    // ── Bug 5: Line 0 returns error ──────────────────────────────────────────

    #[test]
    fn test_line_zero_returns_error() {
        let result = parse_path_line("test.txt:0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("1-indexed"));
    }

    // ── parse_path_line correctness ──────────────────────────────────────────

    #[test]
    fn test_parse_path_line_single() {
        let (path, range) = parse_path_line("foo.txt:5").unwrap();
        assert_eq!(path, "foo.txt");
        assert_eq!(range, Some((5, 5)));
    }

    #[test]
    fn test_parse_path_line_range() {
        let (path, range) = parse_path_line("foo.txt:3-7").unwrap();
        assert_eq!(path, "foo.txt");
        assert_eq!(range, Some((3, 7)));
    }

    #[test]
    fn test_parse_path_line_no_line() {
        let (path, range) = parse_path_line("foo.txt").unwrap();
        assert_eq!(path, "foo.txt");
        assert_eq!(range, None);
    }
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::write_report::WriteReport;

pub struct WriteCommand {
    pub args: Args,
}

impl Command for WriteCommand {
    type Report = WriteReport;

    async fn execute(
        &self,
        ctx: &CommandContext,
    ) -> Result<Self::Report, CommandError> {
        write_to_report(&self.args, ctx).map_err(CommandError::Execution)
    }
}
