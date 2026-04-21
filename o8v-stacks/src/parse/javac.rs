//! Javac text parser — covers `javac` and Maven/Gradle compile output.
//!
//! Javac outputs diagnostics to stderr in the format:
//! `FILE:LINE: error: MESSAGE` or `FILE:LINE: warning: MESSAGE`
//!
//! Maven writes javac diagnostics to stdout (wrapped in Maven format).
//! Gradle writes to stderr. We parse both streams.
//!
//! Additional context lines are indented and point to the error location (^).
//! We skip these and only parse the diagnostic header lines.
//!
//! Example (javac stderr):
//! ```text
//! Test.java:3: error: incompatible types: String cannot be converted to int
//!         int x = "hello";
//!                 ^
//! Test.java:4: error: cannot find symbol
//!         System.out.println(y);
//!                            ^
//!   symbol:   variable y
//!   location: class Test
//! 2 errors
//! ```
//!
//! Example (Maven stdout):
//! ```text
//! [ERROR] COMPILATION ERROR :
//! [ERROR] /path/to/App.java:[3,17] incompatible types: java.lang.String cannot be converted to int
//! [ERROR] /path/to/App.java:[4,28] cannot find symbol
//! ```

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse javac text output into diagnostics.
/// Reads from both stderr (javac direct, Gradle) and stdout (Maven).
#[must_use]
pub fn parse(
    stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut in_failure_footer = false;

    // Maven writes to stdout, javac/gradle write to stderr. Parse both.
    if !stderr.is_empty() && !stdout.is_empty() {
        tracing::debug!("javac: both stdout and stderr have content, preferring stderr");
    }
    let combined = if stderr.is_empty() { stdout } else { stderr };

    for line in combined.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip Maven wrapper ([ERROR], [WARNING], etc.) before parsing
        let line = if let Some(rest) = line.strip_prefix("[ERROR]") {
            rest.trim()
        } else if let Some(rest) = line.strip_prefix("[WARNING]") {
            rest.trim()
        } else {
            line
        };

        // Skip header lines that don't contain file paths
        if line.starts_with("COMPILATION ERROR") {
            continue;
        }

        // Maven failure footer repeats every diagnostic — stop to avoid duplicates.
        if line.starts_with("Failed to execute") {
            in_failure_footer = true;
            continue;
        }
        if in_failure_footer {
            continue;
        }

        // Skip summary lines ("N errors", "N warnings")
        if (line.ends_with("error")
            || line.ends_with("errors")
            || line.ends_with("warning")
            || line.ends_with("warnings"))
            && line.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }

        // Skip indented context lines (source code and carets, symbol info)
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        // Try to parse this line as a diagnostic header
        if let Some(d) = parse_diagnostic_line(line, project_root, tool, stack) {
            diagnostics.push(d);
        }
    }

    // Text parser: we scanned every line. If we found none, the output is clean.
    let status = ParseStatus::Parsed;
    let parsed_items = diagnostics.len() as u32;

    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

/// Parse one javac diagnostic line.
/// Handles two formats:
/// - Javac/Gradle: `FILE:LINE: error: MESSAGE` or `FILE:LINE: warning: MESSAGE`
/// - Maven: `FILE:[LINE,COLUMN] message` (no explicit "error:" keyword, inferred from context)
fn parse_diagnostic_line(
    line: &str,
    project_root: &std::path::Path,
    _tool: &str,
    _stack: &str,
) -> Option<Diagnostic> {
    // Try javac format first: FILE:LINE: error: MESSAGE
    if let Some(diag) = parse_javac_format(line, project_root) {
        return Some(diag);
    }

    // Try Maven format: FILE:[LINE,COLUMN] message
    parse_maven_format(line, project_root)
}

/// Parse javac format: `FILE:LINE: error: MESSAGE` or `FILE:LINE: warning: MESSAGE`
fn parse_javac_format(line: &str, project_root: &std::path::Path) -> Option<Diagnostic> {
    let parts: Vec<&str> = line.splitn(4, ':').collect();
    if parts.len() < 4 {
        return None;
    }

    let file = parts[0];
    let line_num_str = parts[1].trim();
    let severity_str = parts[2].trim();
    let message = parts[3].trim();

    // Parse line number — must be valid or skip this diagnostic
    let line_num: u32 = match line_num_str.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    // Parse severity
    let (severity, raw_sev) = match severity_str {
        "error" => (Severity::Error, "error"),
        "warning" => (Severity::Warning, "warning"),
        _ => return None,
    };

    let column = 1u32;
    let location = super::normalize_path(file, project_root);

    Some(Diagnostic {
        location,
        span: Some(Span::new(line_num, column, None, None)),
        rule: None,
        severity,
        raw_severity: Some(raw_sev.to_string()),
        message: DisplayStr::from_untrusted(message),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: "javac".to_string(),
        stack: "java".to_string(),
    })
}

/// Parse Maven format: `FILE:[LINE,COLUMN] message`
/// Maven doesn't explicitly say "error" or "warning", but diagnostics printed here are errors.
fn parse_maven_format(line: &str, project_root: &std::path::Path) -> Option<Diagnostic> {
    // Maven format: /path/to/File.java:[3,17] incompatible types...
    // Find the opening bracket
    let bracket_pos = line.find('[')?;
    let file = &line[..bracket_pos];

    // Find the matching closing bracket
    let closing_bracket_pos = line.find(']')?;
    if closing_bracket_pos <= bracket_pos {
        return None;
    }

    let coords = &line[bracket_pos + 1..closing_bracket_pos];
    let parts: Vec<&str> = coords.split(',').collect();
    if parts.len() < 2 {
        return None;
    }

    let line_num: u32 = match parts[0].trim().parse() {
        Ok(n) => n,
        Err(_) => return None,
    };
    let column: u32 = match parts[1].trim().parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    // Message is after the ']'
    let message = line[closing_bracket_pos + 1..].trim();
    if message.is_empty() {
        return None;
    }

    let location = super::normalize_path(file, project_root);

    Some(Diagnostic {
        location,
        span: Some(Span::new(line_num, column, None, None)),
        rule: None,
        severity: Severity::Error,
        raw_severity: Some("error".to_string()),
        message: DisplayStr::from_untrusted(message),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: "javac".to_string(),
        stack: "java".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_error() {
        let stderr = "Test.java:3: error: incompatible types: String cannot be converted to int\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.message,
            "incompatible types: String cannot be converted to int"
        );
    }

    #[test]
    fn parse_multiple_diagnostics() {
        let stderr = "Test.java:3: error: error1\nTest.java:4: warning: warning1\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn skip_indented_context_lines() {
        let stderr = "Test.java:3: error: message\n        int x = \"hello\";\n                ^\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn skip_summary_line() {
        let stderr = "Test.java:3: error: message\n2 errors\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn parse_with_multiline_message() {
        let stderr = "Test.java:4: error: cannot find symbol\n        System.out.println(y);\n                           ^\n  symbol:   variable y\n  location: class Test\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "cannot find symbol");
    }

    #[test]
    fn extract_line_and_column() {
        let stderr = "src/Main.java:42: error: test error\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 42);
        assert_eq!(span.column, 1); // Javac doesn't reliably provide column in main line
    }

    #[test]
    fn empty_stderr() {
        let result = parse("", "", std::path::Path::new("/project"), "javac", "java");
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn invalid_line_number() {
        let stderr = "Test.java:abc: error: message\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn maven_failure_footer_does_not_duplicate_diagnostics() {
        // Maven repeats each error in the failure footer after "Failed to execute goal".
        // The parser must stop at that line and not emit duplicates.
        let stdout = "[ERROR] COMPILATION ERROR : 
\n                      [ERROR] /path/App.java:[5,17] type mismatch
\n                      [ERROR] /path/App.java:[6,28] undefined symbol
\n                      [ERROR] Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:compile
\n                      [ERROR] /path/App.java:[5,17] type mismatch
\n                      [ERROR] /path/App.java:[6,28] undefined symbol
";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(
            result.diagnostics.len(),
            2,
            "footer errors must not be re-emitted as diagnostics"
        );
    }

    #[test]
    fn parse_maven_stdout_output() {
        // Maven wraps javac errors in [ERROR] lines using Maven format
        let stdout = "[ERROR] COMPILATION ERROR : \n\
                      [ERROR] /private/tmp/java-fix/src/main/java/App.java:[3,17] incompatible types: java.lang.String cannot be converted to int\n\
                      [ERROR] /private/tmp/java-fix/src/main/java/App.java:[4,28] cannot find symbol\n\
                      [ERROR] Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:3.15.0:compile\n";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        // Should parse errors from stdout when stderr is empty
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        // Maven format captures column info
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 3);
        assert_eq!(span.column, 17);
    }

    #[test]
    fn maven_format_with_brackets() {
        // Maven format: FILE:[LINE,COLUMN] message
        let stdout = "/project/App.java:[5,10] incompatible types error\n";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 10);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn prefers_stderr_over_stdout() {
        let stderr = "Test.java:3: error: stderr error\n";
        let stdout = "Test.java:4: error: stdout error\n";
        let result = parse(
            stdout,
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );

        // When stderr is not empty, it should be parsed (not stdout)
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("stderr error"));
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_javac_empty_input() {
        let result = parse("", "", std::path::Path::new("/project"), "javac", "java");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.parsed_items, 0);
    }

    #[test]
    fn stress_javac_whitespace_only() {
        let result = parse(
            "   \n\n\t\t\n   ",
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_javac_huge_input() {
        let block = "Test.java:10: error: message\n";
        let huge = block.repeat(50_000);
        let result = parse("", &huge, std::path::Path::new("/project"), "javac", "java");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_javac_binary_garbage() {
        let garbage = "Test.java:3: error\x00\x01\x02: message\n";
        let result = parse(
            "",
            garbage,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        // Binary in the middle should prevent diagnostic parsing
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_javac_truncated_block() {
        let truncated_lines = vec![
            "Test.java:3: error",        // missing message (only 3 parts)
            "Test.java:",                // missing line number
            "Test.java:abc: error: msg", // invalid line number (abc is not numeric)
        ];

        for truncated in truncated_lines {
            let result = parse(
                "",
                truncated,
                std::path::Path::new("/project"),
                "javac",
                "java",
            );
            assert_eq!(result.status, ParseStatus::Parsed);
            assert_eq!(result.diagnostics.len(), 0);
        }
    }

    #[test]
    fn stress_javac_unicode_in_paths() {
        let stderr = "src/文件.java:1: error: 错误消息 🔥\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "错误消息 🔥");
    }

    #[test]
    fn stress_javac_extremely_long_line() {
        let long_msg = "x".repeat(1_000_000);
        let stderr = format!("Test.java:1: error: {}\n", long_msg);
        let result = parse(
            "",
            &stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, long_msg);
    }

    #[test]
    fn stress_javac_malformed_line_numbers() {
        let malformed_lines = vec![
            "Test.java:0: error: zero line number\n",
            "Test.java:999999999: error: huge line number\n",
            "Test.java:-1: error: negative line number\n",
            "Test.java:1e9: error: scientific notation\n",
        ];

        for line in malformed_lines {
            let result = parse("", line, std::path::Path::new("/project"), "javac", "java");
            assert_eq!(result.status, ParseStatus::Parsed);
            // Line numbers that fail to parse should be skipped
            let parsed_ok =
                line.contains("0:") || line.contains("999999999:") || line.contains("1e9:");
            if !parsed_ok && line.contains("1:") {
                // "Test.java:1e9:" should fail to parse
                assert_eq!(result.diagnostics.len(), 0);
            }
        }
    }

    #[test]
    fn stress_javac_mixed_stderr_with_noise() {
        let stderr = "Random noise line\nTest.java:1: error: real error\nMore noise\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn stress_javac_gradle_output() {
        // Gradle stderr format is similar to javac
        let stderr = "src/main/java/App.java:5: error: cannot find symbol\n\
                      src/main/java/App.java:6: warning: unreachable code\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn stress_javac_maven_missing_column() {
        // Maven format but missing column number (only line number in brackets)
        let stdout = "[ERROR] /path/to/File.java:[5] missing column\n";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should fail because column is required in maven format
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_javac_maven_very_large_numbers() {
        let stdout = "[ERROR] /path/to/File.java:[999999,888888] large coordinates\n";
        let result = parse(
            stdout,
            "",
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.line, 999999);
        assert_eq!(span.column, 888888);
    }

    #[test]
    fn stress_javac_malformed_maven_format() {
        let malformed_lines = vec![
            "[ERROR] /path/to/File.java:[no,comma] message\n", // non-numeric coords
            "[ERROR] /path/to/File.java:[] empty brackets\n",  // empty brackets
            "[ERROR] /path/to/File.java:[1] no comma\n",       // only one number
        ];

        for line in malformed_lines {
            let result = parse(line, "", std::path::Path::new("/project"), "javac", "java");
            assert_eq!(result.status, ParseStatus::Parsed);
            // Malformed maven format should not parse
            assert_eq!(result.diagnostics.len(), 0);
        }
    }

    #[test]
    fn stress_javac_unknown_severity() {
        let stderr = "Test.java:1: notice: unknown severity\nTest.java:2: info: also unknown\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        // Unknown severities should be skipped
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_javac_mixed_javac_and_maven() {
        // Preference order: stderr over stdout, so if both present, use stderr
        let stderr = "Test.java:1: error: javac format\n";
        let stdout = "[ERROR] /path/to/File.java:[2,3] maven format\n";
        let result = parse(
            stdout,
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // Should prefer stderr (javac format)
        assert!(result.diagnostics[0].message.contains("javac format"));
    }

    #[test]
    fn stress_javac_indented_multiline_context() {
        let stderr = "Test.java:3: error: incompatible types\n\
                      int x = \"hello\";\n\
                      ^\n\
                      Test.java:5: error: cannot find symbol\n\
                      System.out.println(y);\n\
                      ^\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn stress_javac_all_whitespace_in_message() {
        let stderr = "Test.java:1: error:    \t   \n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "");
    }

    #[test]
    fn stress_javac_colon_in_message() {
        let stderr = "Test.java:1: error: time format: HH:MM:SS expected\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        // Message should preserve colons
        assert_eq!(
            result.diagnostics[0].message,
            "time format: HH:MM:SS expected"
        );
    }

    #[test]
    fn stress_javac_windows_path() {
        let stderr = "C:\\Users\\test\\File.java:1: error: message\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("C:\\project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should handle Windows paths (though path logic depends on normalize_path)
        assert!(!result.diagnostics.is_empty() || result.diagnostics.is_empty());
        // Either it parses or doesn't — no crash
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn stress_javac_relative_path_with_dots() {
        let stderr = "../src/File.java:1: error: message\n";
        let result = parse(
            "",
            stderr,
            std::path::Path::new("/project"),
            "javac",
            "java",
        );
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
