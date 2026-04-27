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
    #[arg(allow_hyphen_values = true)]
    pub content: Option<String>,

    /// Insert before line instead of replacing
    #[arg(long)]
    pub insert: bool,

    /// Delete lines instead of replacing
    #[arg(long)]
    pub delete: bool,

    /// Append to end of file. Appended content is written verbatim; add \n to the value if you want a trailing newline.
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

    /// Overwrite an existing file when writing whole-file content (no range, no --find)
    #[arg(long)]
    pub force: bool,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
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

// ─── Escape Sequence Handling ────────────────────────────────────────────────

/// Interpret standard escape sequences in write content arguments.
///
/// Recognised sequences: `\n` → newline, `\t` → tab, `\\` → backslash.
/// Any other `\X` is passed through unchanged (the backslash is preserved).
///
/// This applies to all content arguments: `--append`, `--insert`, and
/// positional content for line-replace and range-replace.
fn unescape_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
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
                        return Err("error: line numbers are 1-indexed, got 0".to_string());
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
                return Err("error: line numbers are 1-indexed, got 0".to_string());
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
            "error: cannot combine --insert, --delete, --append, --find, and --force\n\
             Usage: 8v write <path>:<line> --delete\n\
             Usage: 8v write <path>:<line> --insert \"content\"\n\
             Usage: 8v write <path> --append \"content\""
                .to_string(),
        );
    }

    // Find + replace mode
    if args.find.is_some() || args.replace.is_some() {
        if args.find.is_none() || args.replace.is_none() {
            return Err("error: --find and --replace must both be provided\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        if line_range.is_some() {
            return Err("error: line numbers cannot be used with --find mode\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        let find = unescape_content(&args.find.clone().unwrap());
        if find.is_empty() {
            return Err("error: --find pattern must not be empty\n\
                 Usage: 8v write <path> --find \"old\" --replace \"new\""
                .to_string());
        }
        let replace = unescape_content(&args.replace.clone().unwrap());
        return Ok(WriteOperation::FindReplace {
            find,
            replace,
            all: args.all,
        });
    }

    // Delete mode
    if args.delete {
        let (start, end) = line_range.ok_or(
            "error: delete requires a line number or range\n\
             Usage: 8v write <path>:<line> --delete\n\
             Usage: 8v write <path>:<start>-<end> --delete",
        )?;
        if args.content.is_some() {
            return Err("error: content argument not allowed with --delete\n\
                 Usage: 8v write <path>:<line> --delete"
                .to_string());
        }
        return Ok(WriteOperation::DeleteLines { start, end });
    }

    // Append mode
    if args.append {
        if line_range.is_some() {
            return Err("error: line numbers cannot be used with --append\n\
                 Usage: 8v write <path> --append \"content\""
                .to_string());
        }
        let raw = args.content.clone().ok_or(
            "error: content required for --append\n\
             Usage: 8v write <path> --append \"content\"\n\
             Escape sequences are interpreted: \\n = newline, \\t = tab, \\\\ = backslash",
        )?;
        let content = unescape_content(&raw);
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
            "error: insert requires a line number\n\
             Usage: 8v write <path>:<line> --insert \"content\"",
        )?;
        let raw = args.content.clone().ok_or(
            "error: content required for --insert\n\
             Usage: 8v write <path>:<line> --insert \"content\"\n\
             Escape sequences are interpreted: \\n = newline, \\t = tab, \\\\ = backslash",
        )?;
        let content = unescape_content(&raw);
        if content.is_empty() {
            return Err(
                "error: content cannot be empty for replace/insert — use --delete to remove lines, or provide non-empty content"
                    .to_string(),
            );
        }
        return Ok(WriteOperation::InsertBefore { line, content });
    }

    // Replace or create mode
    let raw = args.content.clone().ok_or(
        "error: content required\n\
         Usage: 8v write <path>:<line> \"content\"   (replace line)\n\
         Usage: 8v write <path>:<start>-<end> \"content\"   (replace range)\n\
         Usage: 8v write <path> \"content\"   (create file)\n\
         Escape sequences are interpreted: \\n = newline, \\t = tab, \\\\ = backslash",
    )?;

    match line_range {
        Some((start, end)) => {
            let content = unescape_content(&raw);
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
            content: raw,
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
    content
        .chars()
        .zip(content.chars().skip(1).chain(std::iter::once('\0')))
        .any(|(c, next)| c == '\r' && next != '\n')
}

/// Validate that the file's line endings are supported for line-based operations.
///
/// Returns Err for:
/// - Lone `\r` (classic Mac, no `\n` at all)
/// - Any lone `\r` in a `\n`-terminated file (mid-line `\r`)
/// - Mixed `\r\n` and lone `\n`
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
/// - Lone `\r` (classic Mac)
/// - `\r\n` (CRLF) — was silently allowed, now rejected
/// - Mixed `\r\n` and lone `\n`
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
    let stripped = content
        .strip_suffix("\r\n")
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
        .get::<crate::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    let op = validate_args(args)?;
    let (path_str, _) = parse_path_line(&args.path)?;

    let root = workspace.containment();
    let config = o8v_fs::FsConfig::default();
    let path = workspace.resolve(&path_str);

    let report_op: ReportOp = match &op {
        WriteOperation::ReplaceLine { line, content } => {
            let file = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!("8v: not found: {path_str}")
                } else {
                    format!("error: failed to read file: {e}")
                }
            })?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *line > lines.len() {
                return Err(format!(
                    "error: line {line} does not exist (file has {} lines)",
                    lines.len()
                ));
            }
            let old_lines = vec![lines[line - 1].to_string()];
            let new_content_str = content.clone();

            validate_content_line_endings(content)?;
            let content_lines = content_to_lines(content);
            let content_line_refs: Vec<&str> = content_lines.iter().map(String::as_str).collect();
            let mut new_lines: Vec<&str> =
                Vec::with_capacity(lines.len() - 1 + content_line_refs.len());
            new_lines.extend_from_slice(&lines[..*line - 1]);
            new_lines.extend_from_slice(&content_line_refs);
            new_lines.extend_from_slice(&lines[*line..]);
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root, new_content_bytes.as_bytes())
                .map_err(|e| format!("error: failed to write file: {e}"))?;

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
            let file = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!("8v: not found: {path_str}")
                } else {
                    format!("error: failed to read file: {e}")
                }
            })?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *start > lines.len() {
                return Err(format!(
                    "error: line {start} does not exist (file has {} lines)",
                    lines.len()
                ));
            }
            if *end > lines.len() {
                return Err(format!(
                    "error: line {end} does not exist (file has {} lines)",
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
            let mut new_lines: Vec<&str> =
                Vec::with_capacity(lines.len() - (end - start) + content_line_refs.len());
            new_lines.extend_from_slice(&lines[..*start - 1]);
            new_lines.extend_from_slice(&content_line_refs);
            new_lines.extend_from_slice(&lines[*end..]);
            let new_content_bytes = join_lines_with_ending(&new_lines, line_ending, trailing);
            o8v_fs::safe_write(&path, root, new_content_bytes.as_bytes())
                .map_err(|e| format!("error: failed to write file: {e}"))?;

            ReportOp::Replace {
                old_lines,
                new_content: new_content_str,
            }
        }
        WriteOperation::InsertBefore { line, content } => {
            let file = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!("8v: not found: {path_str}")
                } else {
                    format!("error: failed to read file: {e}")
                }
            })?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *line > lines.len() + 1 {
                return Err(format!(
                    "error: cannot insert at line {line} (file has {} lines)",
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
            o8v_fs::safe_write(&path, root, new_content_bytes.as_bytes())
                .map_err(|e| format!("error: failed to write file: {e}"))?;

            ReportOp::Insert {
                content: content.clone(),
            }
        }
        WriteOperation::DeleteLines { start, end } => {
            let file = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!("8v: not found: {path_str}")
                } else {
                    format!("error: failed to read file: {e}")
                }
            })?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);
            let trailing = has_trailing_newline(existing_content);
            let lines = split_lines(existing_content, line_ending, trailing);

            if *start > lines.len() || *end > lines.len() || start > end {
                return Err(format!(
                    "error: invalid line range {start}-{end} (file has {} lines)",
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
            o8v_fs::safe_write(&path, root, new_content_bytes.as_bytes())
                .map_err(|e| format!("error: failed to write file: {e}"))?;

            ReportOp::Delete { deleted_lines }
        }
        WriteOperation::CreateFile { content, force } => {
            if content.is_empty() {
                return Err("error: content cannot be empty when creating a file".to_string());
            }
            if !force {
                match o8v_fs::safe_exists(&path, root) {
                    Ok(true) => {
                        return Err(format!(
                            "error: file already exists: {path_str}\n  to replace entire file: add --force\n  to replace a range: use {path_str}:<start>-<end> \"<content>\"\n  to find/replace: use --find \"<old>\" --replace \"<new>\""
                        ));
                    }
                    Ok(false) => {}
                    Err(e) => {
                        return Err(format!("error: failed to check if file exists: {e}"));
                    }
                }
            }
            o8v_fs::safe_write(&path, root, content.as_bytes())
                .map_err(|e| format!("error: failed to create file: {e}"))?;
            let line_count = content.lines().count();
            ReportOp::Create { line_count }
        }
        WriteOperation::AppendToFile { content } => {
            // Surface NotFound early so the error message matches the existing
            // behavior ("8v: not found: <path>") without doing a full read.
            match o8v_fs::safe_exists(&path, root) {
                Ok(true) => {}
                Ok(false) => return Err(format!("8v: not found: {path_str}")),
                Err(o8v_fs::FsError::NotFound { .. }) => {
                    return Err(format!("8v: not found: {path_str}"));
                }
                Err(e) => return Err(format!("error: failed to check if file exists: {e}")),
            }
            validate_content_line_endings(content)?;
            o8v_fs::safe_append_with_separator(&path, root, content.as_bytes())
                .map_err(|e| format!("error: failed to append to file: {e}"))?;
            ReportOp::Append
        }
        WriteOperation::FindReplace { find, replace, all } => {
            validate_content_line_endings(find)?;
            validate_content_line_endings(replace)?;
            let file = o8v_fs::safe_read(&path, root, &config).map_err(|e| {
                if matches!(e, o8v_fs::FsError::NotFound { .. }) {
                    format!("8v: not found: {path_str}")
                } else {
                    format!("error: failed to read file: {e}")
                }
            })?;
            let existing_content = file.content();
            validate_line_endings(existing_content)?;
            let line_ending = detect_line_ending(existing_content);

            // Normalise find/replace to the file's line ending so that a user
            // who passes pure-LF patterns against a CRLF file gets correct
            // matches and CRLF-preserving output.
            let find_normalised;
            let replace_normalised;
            let (effective_find, effective_replace) = if line_ending == "\r\n" {
                find_normalised = find.replace('\n', "\r\n");
                replace_normalised = replace.replace('\n', "\r\n");
                (find_normalised.as_str(), replace_normalised.as_str())
            } else {
                (find.as_str(), replace.as_str())
            };

            let match_count = existing_content.matches(effective_find).count();

            if match_count == 0 {
                return Err(render_not_found_hint(find, existing_content, &path_str));
            }

            if !*all && match_count > 1 {
                return Err(format!(
                    "error: --find matched {match_count} occurrences but --all was not specified.\n\
                     To replace every occurrence add --all:\n\
                     \t8v write {path_str} --find \"...\" --replace \"...\" --all\n\
                     To replace a single occurrence, narrow the pattern so it matches exactly once."
                ));
            }

            let new_content = if *all {
                existing_content.replace(effective_find, effective_replace)
            } else {
                existing_content.replacen(effective_find, effective_replace, 1)
            };

            let count = if *all { match_count } else { 1 };
            o8v_fs::safe_write(&path, root, new_content.as_bytes())
                .map_err(|e| format!("error: failed to write file: {e}"))?;

            ReportOp::FindReplace { count }
        }
    };

    // Relativize display path against workspace root so rendered headers show
    // relative paths even when MCP resolution has made args.path absolute.
    // Falls back to the user-supplied path_str if the file is outside the workspace.
    // Canonicalize before strip_prefix to handle OS symlinks (e.g. macOS
    // /var → /private/var) that cause prefix mismatch when workspace.resolve()
    // returns the non-canonical form but ContainmentRoot stores the canonical form.
    // Use non-canonical `path` for safe_write above; canonical only for display.
    let path_canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.clone(),
    };
    let display_path_str = match path_canonical.strip_prefix(workspace.as_path()) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path_str.clone(),
    };

    Ok(WriteReport {
        path: display_path_str,
        operation: report_op,
    })
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::write_report::WriteReport;

pub struct WriteCommand {
    pub args: Args,
}

impl Command for WriteCommand {
    type Report = WriteReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        write_to_report(&self.args, ctx).map_err(CommandError::Execution)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

/// Render whitespace as visible glyphs so agents can see tab-vs-space and
/// trailing-space differences in error messages.
/// Space → `·`, tab → `→`, newline → `↵` (followed by a real newline so the
/// output still visually wraps).
fn visualize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            ' ' => out.push('·'),
            '\t' => out.push('→'),
            '\n' => {
                out.push('↵');
                out.push('\n');
            }
            c => out.push(c),
        }
    }
    out
}

/// Build the error message for a `--find` miss. If the find is single-line
/// and any line in the file matches `find.trim()` (i.e. whitespace-only
/// difference), render both with visible whitespace as a "closest match"
/// hint. Otherwise fall back to a generic message.
fn render_not_found_hint(find: &str, content: &str, path: &str) -> String {
    // Multi-line find is out of scope for the hint — too many candidate
    // strategies. Still emit a path-qualified fallback.
    if find.contains('\n') {
        return format!(
            "error: no matches found for {find:?} in {path}. \
             Read the file to find the exact text (whitespace and indentation must match), \
             then retry with the correct --find value."
        );
    }

    let find_trim = find.trim();
    let candidate = content
        .lines()
        .find(|line| !line.is_empty() && line.trim() == find_trim && *line != find);

    match candidate {
        Some(line) => format!(
            "error: no matches found for --find in {path}.\n\
             closest match differs in whitespace (· = space, → = tab, ↵ = newline):\n\
             requested:  {}\n\
             found:      {}\n\
             Fix the --find value to match the file exactly and retry.",
            visualize_whitespace(find),
            visualize_whitespace(line),
        ),
        None => format!(
            "error: no matches found for {find:?} in {path}. \
             Read the file to find the exact text (whitespace and indentation must match), \
             then retry with the correct --find value."
        ),
    }
}

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
            format: crate::commands::output_format::OutputFormat::default(),
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

    // ── Bug #12 (A2/D14): find/replace error must show closest-match diff hint ─
    //
    // When --find misses because of whitespace mismatch (tabs vs spaces,
    // trailing whitespace, etc.), the error today says only "no matches found".
    // Agents retry-loop because they can't see what differs. These tests pin
    // the new behavior: if a line in the file matches find.trim(), show the
    // candidate with visible whitespace markers so the difference is obvious.

    #[test]
    fn not_found_hint_shows_tab_vs_spaces_candidate() {
        let content = "    foo\n"; // four spaces + foo
        let find = "\tfoo"; // tab + foo
        let hint = render_not_found_hint(find, content, "file.txt");
        // Must reference the requested text.
        assert!(
            hint.contains("file.txt"),
            "hint should cite the path; got:\n{hint}"
        );
        // Must show a closest candidate (the spaces-prefixed foo line).
        assert!(
            hint.contains("closest match") || hint.contains("closest candidate"),
            "hint must announce a closest match; got:\n{hint}"
        );
        // Must make whitespace visible in BOTH the requested find and the
        // candidate so the reader sees tab-vs-space.
        // Representation: · for space, → for tab (documented in the message).
        assert!(
            hint.contains('→'),
            "hint must render tab as '→' so mismatch is visible; got:\n{hint}"
        );
        assert!(
            hint.contains('·'),
            "hint must render space as '·' so mismatch is visible; got:\n{hint}"
        );
    }

    #[test]
    fn not_found_hint_shows_trailing_space_candidate() {
        // File has two trailing spaces; --find has one. `replace`/`replacen`
        // would miss (not a substring), so the hint logic must kick in.
        let content = "let x = 1;  \n";
        let find = "let x = 1; ";
        let hint = render_not_found_hint(find, content, "file.rs");
        assert!(
            hint.contains("closest"),
            "expected closest hint in:\n{hint}"
        );
        assert!(hint.contains('·'), "expected '·' marker in:\n{hint}");
    }

    #[test]
    fn not_found_hint_falls_back_when_no_candidate() {
        // Totally unrelated find — no trim-equal line, no shared prefix.
        let content = "alpha\nbeta\ngamma\n";
        let find = "zzz_not_in_file_zzz";
        let hint = render_not_found_hint(find, content, "f.txt");
        // Must still produce a useful, non-panicking message.
        assert!(
            hint.contains("no matches found"),
            "fallback must still say no matches; got:\n{hint}"
        );
        // And must mention the path.
        assert!(hint.contains("f.txt"), "must cite path; got:\n{hint}");
    }

    // ── F6: escape sequences in content arguments ────────────────────────────
    //
    // These tests verify that \n, \t, and \\ in content arguments are
    // interpreted as their corresponding characters, not kept as literals.
    // They FAIL on pre-fix code (content stored verbatim) and pass after the fix.

    #[test]
    fn unescape_newline() {
        // \n must become an actual newline character
        let result = unescape_content("line1\\nline2");
        assert_eq!(result, "line1\nline2", "\\n must become a newline");
    }

    #[test]
    fn unescape_tab() {
        let result = unescape_content("col1\\tcol2");
        assert_eq!(result, "col1\tcol2", "\\t must become a tab");
    }

    #[test]
    fn unescape_backslash() {
        let result = unescape_content("a\\\\b");
        assert_eq!(result, "a\\b", "\\\\\\ must become a single backslash");
    }

    #[test]
    fn unescape_unknown_escape_preserved() {
        // \x is not a recognised sequence — backslash stays
        let result = unescape_content("a\\xb");
        assert_eq!(
            result, "a\\xb",
            "unknown escape must preserve the backslash"
        );
    }

    #[test]
    fn unescape_multiple_sequences() {
        let result = unescape_content("fn foo()\\n{\\n\\treturn;\\n}");
        assert_eq!(result, "fn foo()\n{\n\treturn;\n}");
    }

    #[test]
    fn append_operation_unescapes_newline() {
        // validate_args must unescape content for AppendToFile
        let args = Args {
            path: "f.txt".to_string(),
            content: Some("line1\\nline2".to_string()),
            insert: false,
            delete: false,
            append: true,
            find: None,
            replace: None,
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::AppendToFile { content } => {
                assert_eq!(
                    content, "line1\nline2",
                    "AppendToFile content must have real newline, not \\\\n literal"
                );
            }
            other => panic!("expected AppendToFile, got {other:?}"),
        }
    }

    #[test]
    fn insert_operation_unescapes_newline() {
        let args = Args {
            path: "f.txt:5".to_string(),
            content: Some("a\\nb".to_string()),
            insert: true,
            delete: false,
            append: false,
            find: None,
            replace: None,
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::InsertBefore { content, .. } => {
                assert_eq!(content, "a\nb", "InsertBefore content must unescape \\n");
            }
            other => panic!("expected InsertBefore, got {other:?}"),
        }
    }

    #[test]
    fn replace_line_operation_unescapes_newline() {
        let args = Args {
            path: "f.txt:3".to_string(),
            content: Some("x\\ny".to_string()),
            insert: false,
            delete: false,
            append: false,
            find: None,
            replace: None,
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::ReplaceLine { content, .. } => {
                assert_eq!(content, "x\ny", "ReplaceLine content must unescape \\n");
            }
            other => panic!("expected ReplaceLine, got {other:?}"),
        }
    }

    #[test]
    fn replace_range_operation_unescapes_newline() {
        let args = Args {
            path: "f.txt:2-4".to_string(),
            content: Some("a\\nb\\nc".to_string()),
            insert: false,
            delete: false,
            append: false,
            find: None,
            replace: None,
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::ReplaceRange { content, .. } => {
                assert_eq!(content, "a\nb\nc", "ReplaceRange content must unescape \\n");
            }
            other => panic!("expected ReplaceRange, got {other:?}"),
        }
    }

    #[test]
    fn find_replace_operation_unescapes_newline() {
        // --replace must unescape \n just like --append/--insert
        let args = Args {
            path: "f.txt".to_string(),
            content: None,
            insert: false,
            delete: false,
            append: false,
            find: Some("foo".to_string()),
            replace: Some("a\\nb".to_string()),
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::FindReplace { replace, .. } => {
                assert_eq!(
                    replace, "a\nb",
                    "FindReplace replace must have real newline, not \\\\n literal"
                );
            }
            other => panic!("expected FindReplace, got {other:?}"),
        }
    }

    // ── AF-4: --find must unescape like --replace and content args ──────────
    #[test]
    fn find_replace_operation_unescapes_find_newline() {
        let args = Args {
            path: "f.txt".to_string(),
            content: None,
            insert: false,
            delete: false,
            append: false,
            find: Some("a\\nb".to_string()),
            replace: Some("x".to_string()),
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::FindReplace { find, .. } => {
                assert_eq!(find, "a\nb", "--find must unescape \\n to a real newline");
            }
            other => panic!("expected FindReplace, got {other:?}"),
        }
    }

    #[test]
    fn find_replace_operation_unescapes_find_tab_and_backslash() {
        let args = Args {
            path: "f.txt".to_string(),
            content: None,
            insert: false,
            delete: false,
            append: false,
            find: Some("a\\tb\\\\c".to_string()),
            replace: Some("x".to_string()),
            all: false,
            force: false,
            format: crate::commands::output_format::OutputFormat::default(),
        };
        let op = validate_args(&args).expect("should succeed");
        match op {
            WriteOperation::FindReplace { find, .. } => {
                assert_eq!(find, "a\tb\\c", "--find must unescape \\t and \\\\");
            }
            other => panic!("expected FindReplace, got {other:?}"),
        }
    }
}
