// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `read` command — symbol-first file reading.
//!
//! - `8v read path` — returns symbol map (functions, structs, types with line numbers)
//! - `8v read path:10-50` — returns specific line range
//! - `8v read path --full` — returns entire file content
//! - `8v read path --json` — structured JSON output

use std::path::Path;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// File path(s), optionally with line range (path:start-end). Multiple paths allowed.
    #[arg(required = true)]
    pub paths: Vec<String>,

    /// Show full file content instead of symbols
    #[arg(long)]
    pub full: bool,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

// ─── Path Parsing ────────────────────────────────────────────────────────────

/// Parse `path:N-M` into (path, Some((N, M))) or (path, None).
///
/// Only splits on the last colon followed by `digits-digits` — avoids
/// splitting Windows-style paths (C:\...) on the drive colon.
fn parse_path_range(input: &str) -> (String, Option<(usize, usize)>) {
    if let Some(colon_pos) = input.rfind(':') {
        let range_part = &input[colon_pos + 1..];
        if let Some(dash_pos) = range_part.find('-') {
            if let (Ok(start), Ok(end)) = (
                range_part[..dash_pos].parse::<usize>(),
                range_part[dash_pos + 1..].parse::<usize>(),
            ) {
                return (input[..colon_pos].to_string(), Some((start, end)));
            }
        }
    }
    (input.to_string(), None)
}

// ─── Typed Report ────────────────────────────────────────────────────────────

/// Read a single file path (with optional range suffix) and return a typed `ReadReport`.
///
/// Returns `Err(String)` if the file cannot be read or the range is invalid.
fn read_one(
    label: &str,
    full: bool,
    workspace: &o8v::workspace::WorkspaceRoot,
) -> Result<o8v_core::render::read_report::ReadReport, String> {
    use o8v_core::render::read_report::{LineEntry, ReadReport, SymbolEntry};

    let (file_path, range) = parse_path_range(label);

    let abs_path = workspace.resolve(&file_path);
    let root = workspace.containment();

    let config = o8v_fs::FsConfig::default();
    let file = match o8v_fs::safe_read(&abs_path, root, &config) {
        Ok(f) => f,
        Err(o8v_fs::FsError::Io { cause, .. })
            if cause.kind() == std::io::ErrorKind::InvalidData =>
        {
            return Err(format!(
                "8v: {file_path}: file contains invalid UTF-8 (binary file?)"
            ));
        }
        Err(e) => return Err(format!("8v: {e}")),
    };

    let content = file.content();

    if content.contains('\0') {
        return Err(format!(
            "8v: {file_path}: file contains invalid UTF-8 (binary file?)"
        ));
    }

    let total_lines = content.lines().count();

    if let Some((start, end)) = range {
        if start > end {
            return Err(format!(
                "8v: invalid range {}:{}-{} — start must be less than or equal to end",
                file_path, start, end
            ));
        }
        if start > total_lines {
            return Err(format!(
                "8v: range {}:{}-{} is beyond end of file ({} lines)",
                file_path, start, end, total_lines
            ));
        }
    }

    let report = if let Some((start, end)) = range {
        let clamped_start = start.max(1);
        let clamped_end = end.min(total_lines);
        let lines: Vec<LineEntry> = content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let line_num = i + 1;
                if line_num >= clamped_start && line_num <= clamped_end {
                    Some(LineEntry {
                        line: line_num,
                        text: line.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        ReadReport::Range {
            path: file_path,
            start: clamped_start,
            end: clamped_end,
            total_lines,
            lines,
        }
    } else if full {
        let lines: Vec<LineEntry> = content
            .lines()
            .enumerate()
            .map(|(i, line)| LineEntry {
                line: i + 1,
                text: line.to_string(),
            })
            .collect();
        ReadReport::Full {
            path: file_path,
            total_lines,
            lines,
        }
    } else {
        let extension = Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let symbols = o8v_core::symbols::extract_symbols(content, extension);
        let entries: Vec<SymbolEntry> = symbols
            .into_iter()
            .map(|s| SymbolEntry {
                name: s.name,
                kind: s.kind,
                line: s.line,
                signature: s.signature,
            })
            .collect();
        ReadReport::Symbols {
            path: file_path,
            total_lines,
            symbols: entries,
        }
    };

    Ok(report)
}

/// Read one or more files and return a typed `ReadReport`.
///
/// Single path → returns the specific variant (Symbols, Range, Full) — backward compatible.
/// Multiple paths → returns `ReadReport::Multi` with per-file results; errors are inline.
pub fn read_to_report(
    args: &Args,
    ctx: &o8v_core::command::CommandContext,
) -> Result<o8v_core::render::read_report::ReadReport, String> {
    use o8v_core::render::read_report::{MultiEntry, MultiResult, ReadReport};

    let workspace = ctx
        .extensions
        .get::<o8v::workspace::WorkspaceRoot>()
        .ok_or_else(|| "8v: no workspace — run 8v init first".to_string())?;

    if args.paths.len() == 1 {
        // Single path — backward-compatible: return the sub-report directly (no Multi wrapper).
        return read_one(&args.paths[0], args.full, workspace);
    }

    // Multiple paths — collect into Multi, errors are inline.
    let entries: Vec<MultiEntry> = args
        .paths
        .iter()
        .map(|label| {
            let result = match read_one(label, args.full, workspace) {
                Ok(report) => MultiResult::Ok {
                    report: Box::new(report),
                },
                Err(message) => MultiResult::Err { message },
            };
            MultiEntry {
                label: label.clone(),
                result,
            }
        })
        .collect();

    Ok(ReadReport::Multi { entries })
}

// ── Command trait impl ──────────────────────────────────────────────────

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::read_report::ReadReport;

pub struct ReadCommand {
    pub args: Args,
}

impl Command for ReadCommand {
    type Report = ReadReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        read_to_report(&self.args, ctx).map_err(CommandError::Execution)
    }
}
