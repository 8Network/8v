//! Rustfmt diff parser — covers `cargo fmt --all --check -- --color=never`.
//!
//! Rustfmt emits a custom diff format (NOT unified diff):
//! ```text
//! Diff in /path/to/file.rs:N:
//!  context
//! -removed
//! +added
//!  context
//! ```
//!
//! Each `Diff in` block becomes one diagnostic at the given file and line.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse rustfmt diff output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();

    // Accumulate state for current diff block.
    let mut current_file: Option<String> = None;
    let mut current_line: u32 = 0;
    let mut current_diff: Vec<String> = Vec::new();

    for line in stdout.lines() {
        if let Some((file, line_num)) = parse_diff_header(line) {
            // Flush previous block.
            if let Some(ref file_path) = current_file {
                if !current_diff.is_empty() {
                    diagnostics.push(make_diagnostic(
                        file_path,
                        current_line,
                        &current_diff,
                        project_root,
                        tool,
                        stack,
                    ));
                }
            }
            current_file = Some(file);
            current_line = line_num;
            current_diff.clear();
        } else if current_file.is_some() {
            // Part of current diff block — collect all lines (context, +, -).
            current_diff.push(line.to_string());
        }
    }

    // Flush last block.
    if let Some(ref file_path) = current_file {
        if !current_diff.is_empty() {
            diagnostics.push(make_diagnostic(
                file_path,
                current_line,
                &current_diff,
                project_root,
                tool,
                stack,
            ));
        }
    }

    // Text parser: we scanned every line for "Diff in" headers. If we found
    // none, the output is clean or noise — not "unparsed".
    let status = ParseStatus::Parsed;
    let parsed_items = diagnostics.len() as u32;

    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

/// Parse a `Diff in PATH:N:` header line.
/// Returns `(file_path, line_number)` if this is a valid header.
///
/// Real rustfmt output format: `Diff in /path/to/file.rs:1:`
/// The path and line number are separated by the last two colons.
fn parse_diff_header(line: &str) -> Option<(String, u32)> {
    let rest = line.strip_prefix("Diff in ")?;

    // Strip trailing ':'
    let rest = rest.strip_suffix(':')?;

    // Find the last ':' — separates path from line number.
    // Use rfind to handle paths with colons (Windows drives, etc.).
    let colon = rest.rfind(':')?;
    let file = &rest[..colon];
    let num_str = &rest[colon + 1..];

    if file.is_empty() {
        return None;
    }

    let line_num: u32 = match num_str.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    Some((file.to_string(), line_num))
}

fn make_diagnostic(
    file: &str,
    line_num: u32,
    diff_lines: &[String],
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Diagnostic {
    let location = super::normalize_path(file, project_root);
    let snippet = diff_lines.join("\n");

    Diagnostic {
        location,
        span: Some(Span::new(line_num, 1, None, None)),
        rule: None,
        severity: Severity::Error,
        raw_severity: None,
        message: DisplayStr::from_trusted("formatting differs from expected"),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: Some(snippet),
        tool: tool.to_string(),
        stack: stack.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::Location;
    use std::path::Path;

    #[test]
    fn parses_single_diff_block() {
        let input = "\
Diff in /project/src/main.rs:1:
 fn main() {
-    let x=1;
+    let x = 1;
 }
";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/main.rs".to_string())
        );
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 1);
        assert_eq!(result.diagnostics[0].tool, "cargo fmt");
        assert!(result.diagnostics[0]
            .snippet
            .as_ref()
            .unwrap()
            .contains("-    let x=1;"));
        assert!(result.diagnostics[0]
            .snippet
            .as_ref()
            .unwrap()
            .contains("+    let x = 1;"));
    }

    #[test]
    fn parses_multiple_diff_blocks() {
        let input = "\
Diff in /project/src/a.rs:5:
-bad
+good
Diff in /project/src/b.rs:10:
-old
+new
";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/a.rs".to_string())
        );
        assert_eq!(result.diagnostics[0].span.as_ref().unwrap().line, 5);
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/b.rs".to_string())
        );
        assert_eq!(result.diagnostics[1].span.as_ref().unwrap().line, 10);
    }

    #[test]
    fn empty_output_returns_parsed_empty() {
        let result = parse("", "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn non_diff_output_returns_parsed_clean() {
        let input = "some random output that is not a diff";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        // Text parser scanned lines, found no "Diff in" headers → clean → Parsed.
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn relative_path_preserved() {
        let input = "Diff in src/lib.rs:3:\n-bad\n+good\n";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/lib.rs".to_string())
        );
    }

    #[test]
    fn absolute_path_outside_root() {
        let input = "Diff in /other/place/lib.rs:1:\n-x\n+y\n";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(
            result.diagnostics[0].location,
            Location::Absolute("/other/place/lib.rs".to_string())
        );
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_rustfmt_empty_input() {
        let result = parse("", "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_rustfmt_huge_input() {
        let block = "Diff in /project/src/file.rs:10:\n-old\n+new\n";
        let huge = block.repeat(100_000);
        let result = parse(&huge, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_rustfmt_binary_garbage() {
        let garbage = "Diff in /project/src/file.rs:1\x00\x01\x02:\n-x\n+y\n";
        let result = parse(garbage, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Binary in the middle should prevent header parsing
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_rustfmt_whitespace_only() {
        let result = parse(
            "   \n\n\t\t\n   ",
            "",
            Path::new("/project"),
            "cargo fmt",
            "rust",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_rustfmt_truncated_header() {
        let truncated = "Diff in /project/src/file.rs:1\n";
        let result = parse(truncated, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Missing trailing ':', so header doesn't parse
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_rustfmt_unicode_in_paths() {
        let input = "Diff in /project/src/文件.rs:10:\n-old\n+new\n";
        let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_rustfmt_extremely_long_diff_line() {
        let long_content = "x".repeat(1_000_000);
        let input = format!(
            "Diff in /project/src/file.rs:1:\n-{}\n+{}\n",
            long_content, long_content
        );
        let result = parse(&input, "", Path::new("/project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_rustfmt_malformed_line_number() {
        let malformed = vec![
            "Diff in /project/src/file.rs:0:\n-x\n+y\n",
            "Diff in /project/src/file.rs:invalid:\n-x\n+y\n",
            "Diff in /project/src/file.rs:999999999:\n-x\n+y\n",
        ];
        for input in malformed {
            let result = parse(input, "", Path::new("/project"), "cargo fmt", "rust");
            assert_eq!(result.status, ParseStatus::Parsed);
            // Malformed line numbers should be handled
        }
    }

    #[test]
    fn stress_rustfmt_multiple_colons_in_path() {
        // Windows paths with drive letters: C:\path\file.rs:1:
        let input = "Diff in C:\\path\\file.rs:1:\n-x\n+y\n";
        let result = parse(input, "", Path::new("C:\\project"), "cargo fmt", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // rfind should find the last colon correctly
    }
}
