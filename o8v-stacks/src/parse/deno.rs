//! Deno diagnostic parser — covers `deno check` stderr output.
//!
//! Deno writes diagnostics to stderr in a multi-line block format:
//! ```text
//! TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
//! const x: number = "hello";
//!       ^
//!     at file:///path/to/file.ts:1:7
//! ```
//!
//! Each block has: code + severity line, optional snippet, location line.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse deno check stderr output into diagnostics.
#[must_use]
pub fn parse(
    _stdout: &str,
    stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut any_dropped = false;
    let lines: Vec<&str> = stderr.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Match: TSxxxx [ERROR]: message  or  TSxxxx [WARNING]: message
        if let Some(header) = parse_header(line) {
            // Scan forward for "at file:///path:line:col"
            let mut location = None;
            let mut snippet_lines = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let next = lines[j].trim();
                if next.starts_with("at ") {
                    location = parse_at_line(next, project_root);
                    j += 1;
                    break;
                }
                if next.is_empty() || parse_header(next).is_some() {
                    break; // next block or empty line
                }
                snippet_lines.push(next);
                j += 1;
            }

            let (loc, span) = match location {
                Some((loc, line_num, col)) => (loc, Some(Span::new(line_num, col, None, None))),
                None => {
                    any_dropped = true;
                    i = j; // advance past scanned lines to avoid infinite loop
                    continue;
                }
            };

            let snippet = if snippet_lines.is_empty() {
                None
            } else {
                Some(snippet_lines.join("\n"))
            };

            diagnostics.push(Diagnostic {
                location: loc,
                span,
                rule: Some(DisplayStr::from_untrusted(header.code.to_string())),
                severity: header.severity,
                raw_severity: Some(header.raw_severity.to_string()),
                message: DisplayStr::from_untrusted(header.message.to_string()),
                related: vec![],
                notes: vec![],
                suggestions: vec![],
                snippet,
                tool: tool.to_string(),
                stack: stack.to_string(),
            });

            i = j;
            continue;
        }

        i += 1;
    }

    let parsed_items = diagnostics.len() as u32;
    let status = if any_dropped {
        ParseStatus::Unparsed
    } else {
        ParseStatus::Parsed
    };
    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

struct Header<'a> {
    code: &'a str,
    severity: Severity,
    raw_severity: &'a str,
    message: &'a str,
}

/// Parse `TS2322 [ERROR]: message` or `TS2322 [WARNING]: message`
fn parse_header(line: &str) -> Option<Header<'_>> {
    // Must start with TS + digits
    if !line.starts_with("TS") {
        return None;
    }

    let bracket_open = line.find('[')?;
    let bracket_close = line[bracket_open..].find(']')? + bracket_open;

    let code = line[..bracket_open].trim();
    // Verify code is TSxxxx
    if code.len() < 3 || !code[2..].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let sev_str = line[bracket_open + 1..bracket_close].trim();
    let (severity, raw_severity) = match sev_str {
        "ERROR" => (Severity::Error, "error"),
        "WARNING" | "WARN" => (Severity::Warning, "warning"),
        _ => return None,
    };

    // After "]: " is the message
    let rest = &line[bracket_close + 1..];
    let message = rest.strip_prefix(':').unwrap_or(rest).trim();

    Some(Header {
        code,
        severity,
        raw_severity,
        message,
    })
}

/// Parse `at file:///path/to/file.ts:line:col`
fn parse_at_line(
    line: &str,
    project_root: &std::path::Path,
) -> Option<(o8v_core::diagnostic::Location, u32, u32)> {
    let rest = line.strip_prefix("at ")?.trim();

    // Strip file:// protocol — file:///path → /path
    let path_str = rest.strip_prefix("file://").unwrap_or(rest);

    // Split on last two colons: path:line:col
    let last_colon = path_str.rfind(':')?;
    let col: u32 = match path_str[last_colon + 1..].parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    let before_col = &path_str[..last_colon];
    let second_colon = before_col.rfind(':')?;
    let line_num: u32 = match before_col[second_colon + 1..].parse() {
        Ok(n) => n,
        Err(_) => return None,
    };

    let file_path = &before_col[..second_colon];

    let location = super::normalize_path(file_path, project_root);
    Some((location, line_num, col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stderr: &str) -> ParseResult {
        parse("", stderr, Path::new(ROOT), "deno check", "deno")
    }

    #[test]
    fn empty_stderr() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn single_error() {
        let stderr = "\
TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
const x: number = \"hello\";
      ^
    at file:///project/src/main.ts:1:7
";
        let result = run(stderr);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);

        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("TS2322"));
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("not assignable"));
        assert_eq!(d.location, Location::File("src/main.ts".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 7);
        assert!(d.snippet.is_some());
        assert_eq!(d.tool, "deno check");
        assert_eq!(d.stack, "deno");
    }

    #[test]
    fn warning_lines_and_noise_skipped() {
        let stderr = "\
Warning \"exports\" field should be specified.
    at file:///project/deno.json
Check main.ts
TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
const x: number = \"hello\";
      ^
    at file:///project/main.ts:1:7

error: Type checking failed.
";
        let result = run(stderr);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("TS2322"));
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("main.ts".to_string())
        );
    }

    #[test]
    fn multiple_errors() {
        let stderr = "\
TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
const x: number = \"hello\";
      ^
    at file:///project/main.ts:1:7

TS2304 [ERROR]: Cannot find name 'foo'.
console.log(foo);
            ^^^
    at file:///project/main.ts:3:13
";
        let result = run(stderr);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("TS2322"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("TS2304"));
    }

    #[test]
    fn missing_location_does_not_infinite_loop() {
        // Header with no "at file://..." line — must skip, not loop forever.
        let stderr = "\
TS2322 [ERROR]: Type 'string' is not assignable to type 'number'.
const x: number = \"hello\";
      ^

TS2304 [ERROR]: Cannot find name 'foo'.
console.log(foo);
            ^^^
    at file:///project/main.ts:3:13
";
        let result = run(stderr);
        // First diagnostic has no location — skipped. Second has location — kept.
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("TS2304"));
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_deno_empty_input() {
        let result = run("");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_deno_huge_input() {
        let block = "TS2322 [ERROR]: Type error\nconst x = 1;\n      ^\n    at file:///project/main.ts:1:7\n\n";
        let huge = block.repeat(50_000);
        let result = run(&huge);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_deno_binary_garbage() {
        let garbage = "TS2322 [ERROR\x00\x01\x02]: message\n";
        let result = run(garbage);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_deno_whitespace_only() {
        let result = run("   \n\n\t\t\n   ");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_deno_truncated_block() {
        let truncated = "TS2322 [ERROR]: Type 'string' is not assignable\nconst x = 1;";
        let result = run(truncated);
        assert_eq!(result.status, ParseStatus::Unparsed);
        // No location, so diagnostic skipped
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_deno_unicode_in_paths() {
        let stderr = "TS2322 [ERROR]: 错误消息 🔥\nconst x: number = \"hello\";\n      ^\n    at file:///project/src/文件.ts:1:7\n";
        let result = run(stderr);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
        if let Some(d) = result.diagnostics.first() {
            assert!(d.message.contains("error") || !d.message.is_empty());
        }
    }

    #[test]
    fn stress_deno_extremely_long_line() {
        let long_msg = "x".repeat(1_000_000);
        let stderr = format!(
            "TS2322 [ERROR]: {}\ncode\n    ^\n    at file:///project/main.ts:1:7\n",
            long_msg
        );
        let result = run(&stderr);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn stress_deno_malformed_at_line() {
        // Blocks 1-3: location line is genuinely unparseable — diagnostic is dropped.
        let unparseable_blocks = vec![
            "TS2322 [ERROR]: error\n    at \n",
            "TS2322 [ERROR]: error\n    at file://\n",
            "TS2322 [ERROR]: error\n    at file:///path:invalid:col\n",
        ];
        for block in unparseable_blocks {
            let result = run(block);
            assert_eq!(result.status, ParseStatus::Unparsed);
        }

        // Block 4: 999999999 and 888888888 both fit in u32 — parse_at_line succeeds.
        // Path /path is not under /project → Location::Absolute. Diagnostic is pushed.
        let parseable_block = "TS2322 [ERROR]: error\n    at file:///path:999999999:888888888\n";
        let result = run(parseable_block);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn remote_url_location_drops_diagnostic_reports_unparsed() {
        // Header fully parsed (code=TS2345, severity=ERROR, message extracted),
        // but location line is a remote URL with no :line:col — parse_at_line returns None.
        // Bug: currently returns ParseStatus::Parsed with zero diagnostics (silent drop).
        // Fix: must return ParseStatus::Unparsed so caller can flag it.
        let stderr = concat!(
            "TS2345 [ERROR]: Argument of type 'string' is not assignable to parameter of type 'number'.\n",
            "const x: number = fn(\"hello\");\n",
            "                     ^^^^^^^\n",
            "    at https://deno.land/x/foo@1.0.0/mod.ts\n",
        );
        let result = run(stderr);
        // Header was extracted — this is a dropped diagnostic, not a no-op.
        // Must signal unparsed so caller knows something was lost.
        assert_eq!(result.status, ParseStatus::Unparsed);
        // The diagnostic cannot be emitted without a valid location — but status must be Unparsed
        assert_eq!(result.diagnostics.len(), 0);
    }
}
