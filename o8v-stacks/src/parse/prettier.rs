//! Prettier formatter parser — covers `prettier --list-different .`.
//!
//! Prettier with `--list-different` outputs one filename per line to stdout.
//! Exit 0 = all files formatted correctly, exit 1 = files differ.
//!
//! Output format:
//! ```text
//! src/index.js
//! src/utils.js
//! ```
//!
//! Each line (non-empty) becomes one diagnostic: "formatting differs from expected".

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity};
use o8v_core::display_str::DisplayStr;

/// Parse prettier `--list-different` output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let location = super::normalize_path(trimmed, project_root);
        diagnostics.push(Diagnostic {
            location,
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_trusted("formatting differs from expected"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: tool.to_string(),
            stack: stack.to_string(),
        });
    }

    // Text parser: we scanned every line. Zero diagnostics means the output
    // was clean (no formatting differences), not that parsing failed.
    let status = ParseStatus::Parsed;
    let parsed_items = diagnostics.len() as u32;

    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::Location;
    use std::path::Path;

    #[test]
    fn empty_output_returns_parsed_empty() {
        let result = parse("", "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn single_file() {
        let input = "src/index.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/index.js".to_string())
        );
        assert_eq!(
            result.diagnostics[0].message,
            "formatting differs from expected"
        );
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[0].tool, "prettier");
        assert_eq!(result.diagnostics[0].stack, "javascript");
    }

    #[test]
    fn multiple_files() {
        let input = "src/index.js\nsrc/utils.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "typescript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/index.js".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/utils.js".to_string())
        );
    }

    #[test]
    fn whitespace_lines_skipped() {
        let input = "src/index.js\n   \nsrc/utils.js\n\n  \t  \n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/index.js".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/utils.js".to_string())
        );
    }

    #[test]
    fn absolute_path_under_root_stripped() {
        let input = "/project/src/index.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/index.js".to_string())
        );
    }

    #[test]
    fn absolute_path_outside_root() {
        let input = "/other/place/index.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::Absolute("/other/place/index.js".to_string())
        );
    }

    #[test]
    fn relative_path_preserved() {
        let input = "src/lib.ts\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "typescript");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/lib.ts".to_string())
        );
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_prettier_huge_input() {
        let file = "src/file.js\n";
        let huge = file.repeat(100_000);
        let result = parse(&huge, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_prettier_binary_garbage() {
        let garbage = "src/file.js\n\x00\x01\x02src/other.js\n";
        let result = parse(garbage, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Binary shouldn't prevent parsing the lines
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_prettier_whitespace_only() {
        let result = parse(
            "   \n\n\t\t\n   ",
            "",
            Path::new("/project"),
            "prettier",
            "javascript",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_prettier_unicode_in_paths() {
        let input = "src/文件.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/文件.js".to_string())
        );
    }

    #[test]
    fn stress_prettier_extremely_long_filename() {
        let long_name = "a".repeat(10_000);
        let input = format!("src/{}.js\n", long_name);
        let result = parse(&input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_prettier_many_files() {
        let mut input = String::new();
        for i in 0..10_000 {
            input.push_str(&format!("src/file{}.js\n", i));
        }
        let result = parse(&input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 10_000);
    }

    // ─── Coverage matrix tests ──────────────────────────────────────────────

    #[test]
    fn coverage_crash_malformed_input() {
        // Test 4: crash — malformed/corrupt input that might break parsing
        let garbage = "src/file1.js\n\x00\x01\x02src/file2.js\n\x7F";
        let result = parse(garbage, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Parser should handle binary gracefully by reading line-by-line
        // Some valid paths should still be extracted
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn coverage_malformed_truncated_input() {
        // Test 5: malformed — partial/truncated input
        let truncated = "src/file1.js\nsrc/file2"; // No newline at end, incomplete filename
        let result = parse(
            truncated,
            "",
            Path::new("/project"),
            "prettier",
            "javascript",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        // Both lines (including incomplete one) should be processed
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/file1.js".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/file2".to_string())
        );
    }

    #[test]
    fn coverage_real_prettier_output() {
        // Test 7: real — matches real prettier --list-different output
        // Actual prettier output with multiple files
        let real_output = "app.js\nsrc/index.tsx\ncomponents/Button.vue\ntests/unit.spec.js\n";
        let result = parse(
            real_output,
            "",
            Path::new("/project"),
            "prettier",
            "javascript",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 4);
        assert_eq!(result.parsed_items, 4);

        // Verify all filenames are present
        let locations: Vec<String> = result
            .diagnostics
            .iter()
            .map(|d| match &d.location {
                Location::File(f) | Location::Absolute(f) => f.clone(),
                _ => String::new(),
            })
            .collect();

        assert_eq!(locations[0], "app.js");
        assert_eq!(locations[1], "src/index.tsx");
        assert_eq!(locations[2], "components/Button.vue");
        assert_eq!(locations[3], "tests/unit.spec.js");

        // All should be Error severity with formatting message
        for diag in &result.diagnostics {
            assert_eq!(diag.severity, Severity::Error);
            assert_eq!(diag.message, "formatting differs from expected");
            assert_eq!(diag.tool, "prettier");
            assert_eq!(diag.stack, "javascript");
        }
    }

    #[test]
    fn coverage_paths_with_spaces() {
        // Edge case: paths containing spaces
        let input = "src/my file.js\nsrc/another file.ts\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/my file.js".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("src/another file.ts".to_string())
        );
    }

    #[test]
    fn coverage_deeply_nested_paths() {
        // Edge case: deeply nested directory structures
        let deep = "very/deep/nested/folder/structure/file.js\n";
        let result = parse(deep, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("very/deep/nested/folder/structure/file.js".to_string())
        );
    }

    #[test]
    fn coverage_mixed_file_extensions() {
        // Edge case: various file extensions prettier supports
        let input = "src/app.js\nsrc/styles.css\nsrc/template.html\nsrc/data.json\nsrc/script.ts\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 5);
        assert_eq!(result.parsed_items, 5);
    }

    #[test]
    fn coverage_single_blank_line() {
        // Edge case: single blank line in output
        let input = "src/file.js\n\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn coverage_mixed_blank_and_content() {
        // Edge case: mix of blank lines, whitespace, and content
        let input = "src/a.js\n\n\nsrc/b.js\n  \nsrc/c.js\n\t\nsrc/d.js\n";
        let result = parse(input, "", Path::new("/project"), "prettier", "javascript");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 4);

        let files: Vec<String> = result
            .diagnostics
            .iter()
            .map(|d| match &d.location {
                Location::File(f) => f.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(files, vec!["src/a.js", "src/b.js", "src/c.js", "src/d.js"]);
    }
}
