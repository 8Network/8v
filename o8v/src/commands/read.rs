// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `read` command — symbol-first file reading.
//!
//! - `8v read path` — returns symbol map (functions, structs, types with line numbers)
//! - `8v read path:10-50` — returns specific line range
//! - `8v read path --full` — returns entire file content
//! - `8v read path --json` — structured JSON output
//!
//! Readable binaries (PDF, images) are auto-detected and returned as base64 +
//! MIME. Opaque binaries (archives, executables) return a structured error.

use std::path::Path;

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// File path(s), optionally with line range (path:start-end). Pass any combination in one
    /// call: distinct files, multiple ranges of the same file, or a mix.
    #[arg(required = true)]
    pub paths: Vec<String>,

    /// Show full file content instead of symbols
    #[arg(long, overrides_with = "full")]
    pub full: bool,

    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

/// Extensions where the symbol map is not meaningful and raw text is the
/// expected output: data, markup, and config files. When symbol extraction
/// returns empty for these, fall back to rendering the full content.
fn is_data_or_markup_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "svg"
            | "md"
            | "markdown"
            | "txt"
            | "text"
            | "toml"
            | "yaml"
            | "yml"
            | "json"
            | "json5"
            | "jsonc"
            | "xml"
            | "html"
            | "htm"
            | "csv"
            | "tsv"
            | "ini"
            | "conf"
            | "cfg"
            | "env"
            | "rst"
    )
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
    workspace: &crate::workspace::WorkspaceRoot,
) -> Result<o8v_core::render::read_report::ReadReport, String> {
    use base64::Engine;
    use o8v_core::mime::{detect_kind, mime_for_ext, FileKind};
    use o8v_core::render::read_report::{LineEntry, ReadReport, SymbolEntry};

    let (file_path, range) = parse_path_range(label);

    let abs_path = workspace.resolve(&file_path);
    let root = workspace.containment();

    // Relativize display path against workspace root so rendered headers show
    // relative paths even when MCP resolution has made args.paths absolute.
    // Canonicalize before strip_prefix to handle OS symlinks (e.g. macOS
    // /var → /private/var) that cause prefix mismatch when workspace.resolve()
    // returns the non-canonical form but ContainmentRoot stores the canonical form.
    // Falls back to the user-supplied label if the file is outside the workspace.
    let abs_path_canonical = match abs_path.canonicalize() {
        Ok(p) => p,
        Err(_) => abs_path.clone(),
    };
    let display_path = match abs_path_canonical.strip_prefix(workspace.as_path()) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => file_path.clone(),
    };

    let config = o8v_fs::FsConfig::default();

    // Classify by extension before attempting a text read — binary files
    // (PDF, images) take the bytes path, not the UTF-8 path.
    let extension = Path::new(&file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let kind = detect_kind(extension);

    if matches!(kind, FileKind::ReadableBinary) {
        let bytes =
            o8v_fs::safe_read_bytes(&abs_path, root, &config).map_err(|e| format!("8v: {e}"))?;
        let mime_type = mime_for_ext(extension)
            .unwrap_or("application/octet-stream")
            .to_string();
        let size_bytes = bytes.len() as u64;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Ok(ReadReport::BinaryContent {
            path: display_path,
            mime_type,
            size_bytes,
            base64: b64,
        });
    }

    if matches!(kind, FileKind::OpaqueBinary) {
        let size_bytes = match o8v_fs::safe_metadata(&abs_path, root) {
            Ok(m) => m.len(),
            Err(e) => return Err(format!("8v: {e}")),
        };
        let mime_type = mime_for_ext(extension).unwrap_or("application/octet-stream");
        return Err(format!(
            "8v: {file_path}: cannot read opaque binary file ({mime_type}, {size_bytes} bytes)"
        ));
    }

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
            path: display_path,
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
            path: display_path,
            total_lines,
            lines,
        }
    } else {
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
        // For text files with no extractable symbols (SVG, Markdown, plain
        // text, TOML, YAML, etc.), fall back to returning the full content.
        // The symbol map is empty and unhelpful; the raw text is what the
        // agent actually wants.
        if entries.is_empty() && is_data_or_markup_ext(extension) {
            let lines: Vec<LineEntry> = content
                .lines()
                .enumerate()
                .map(|(i, line)| LineEntry {
                    line: i + 1,
                    text: line.to_string(),
                })
                .collect();
            ReadReport::Full {
                path: display_path,
                total_lines,
                lines,
            }
        } else {
            ReadReport::Symbols {
                path: display_path,
                total_lines,
                symbols: entries,
            }
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
        .get::<crate::workspace::WorkspaceRoot>()
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
            // Relativize the label's path portion for display, preserving any :N-M suffix.
            let (path_part, range_suffix) = {
                let (p, r) = parse_path_range(label);
                let suffix = r.map(|(s, e)| format!(":{s}-{e}")).unwrap_or_default();
                (p, suffix)
            };
            let abs = workspace.resolve(&path_part);
            let abs_canonical = match abs.canonicalize() {
                Ok(p) => p,
                Err(_) => abs.clone(),
            };
            let display_label = match abs_canonical.strip_prefix(workspace.as_path()) {
                Ok(rel) => format!("{}{}", rel.to_string_lossy(), range_suffix),
                Err(_) => label.clone(),
            };
            let result = match read_one(label, args.full, workspace) {
                Ok(report) => MultiResult::Ok {
                    report: Box::new(report),
                },
                Err(message) => MultiResult::Err { message },
            };
            MultiEntry {
                label: display_label,
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
