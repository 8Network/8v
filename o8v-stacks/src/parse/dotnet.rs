//! .NET `MSBuild` console output parser.
//!
//! Parses `dotnet build --warnaserror --tl:off` stdout.
//! Format: `file(line,col): error CSxxxx: message [project]`
//! Same format as tsc but with trailing `[project]` suffix.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;

/// Parse dotnet build text output into diagnostics.
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
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(d) = parse_line(line, project_root, tool, stack) {
            diagnostics.push(d);
        }
    }

    // MSBuild emits each diagnostic twice (compile pass + summary pass), but the
    // two occurrences are not adjacent — dedup_by won't work. Use a seen-set keyed
    // on (location_str, line, col, rule) to retain first occurrence only.
    let mut seen = std::collections::HashSet::new();
    diagnostics.retain(|d| {
        let line = d.span.as_ref().map(|s| s.line).unwrap_or(0);
        let col = d.span.as_ref().map(|s| s.column).unwrap_or(0);
        let key = (
            format!("{:?}", d.location),
            line,
            col,
            d.rule.as_deref().unwrap_or("").to_string(),
        );
        seen.insert(key)
    });

    // Text parser: we scanned every line looking for ": error " / ": warning "
    // patterns. If we found none, the output is build noise — not "unparsed".
    let status = ParseStatus::Parsed;
    let parsed_items = diagnostics.len() as u32;

    ParseResult {
        diagnostics,
        status,
        parsed_items,
    }
}

/// Parse one `MSBuild` diagnostic line.
/// Format: `path/file.cs(line,col): error CSxxxx: message [/path/project.csproj]`
fn parse_line(
    line: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Option<Diagnostic> {
    // Must contain ": error " or ": warning " to be a diagnostic
    let is_error = line.contains(": error ");
    let is_warning = line.contains(": warning ");
    if !is_error && !is_warning {
        return None;
    }

    // Strip trailing [/path/project.csproj] if present.
    // MSBuild always appends the project path in brackets at the end.
    // Must NOT strip brackets that are part of the message (e.g. "Expected [int]").
    // Check: line ends with ']' AND the bracketed content looks like a project path.
    let line = strip_project_suffix(line);

    // Find the (line,col) group. Search backwards so filenames with parens work.
    let (file, line_num, col_num, close_paren) = super::find_location(line)?;

    // Rest after "): "
    let rest = line[close_paren + 1..].trim();
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim();

    let (severity, raw_sev, rest) = match (rest.strip_prefix("error"), rest.strip_prefix("warning"))
    {
        (Some(r), _) => (Severity::Error, "error", r.trim()),
        (_, Some(r)) => (Severity::Warning, "warning", r.trim()),
        _ => return None,
    };

    // Parse code: "CSxxxx: message"
    let (rule, message) = rest.find(':').map_or_else(
        || (None, rest.to_string()),
        |colon_pos| {
            let code = rest[..colon_pos].trim();
            let msg = rest[colon_pos + 1..].trim();
            (Some(code.to_string()), msg.to_string())
        },
    );

    let location = super::normalize_path(file, project_root);

    Some(Diagnostic {
        location,
        span: Some(Span::new(line_num, col_num, None, None)),
        rule: rule.map(DisplayStr::from_untrusted),
        severity,
        raw_severity: Some(raw_sev.to_string()),
        message: DisplayStr::from_untrusted(message),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    })
}

/// Strip the `[/path/project.csproj]` suffix that `MSBuild` appends.
/// Only strips if the bracketed content ends with a .NET project extension.
fn strip_project_suffix(line: &str) -> &str {
    const EXTENSIONS: &[&str] = &[".csproj", ".fsproj", ".vbproj", ".sln", ".slnx"];

    if !line.ends_with(']') {
        return line;
    }
    if let Some(open) = line.rfind('[') {
        // Bounds check: open + 1 must be < line.len() - 1 for a non-empty inside.
        if open + 1 >= line.len().saturating_sub(1) {
            return line;
        }
        let inside = &line[open + 1..line.len() - 1];
        if EXTENSIONS.iter().any(|ext| inside.ends_with(ext)) {
            return line[..open].trim();
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    const ROOT: &str = "/project";

    fn run(stdout: &str) -> ParseResult {
        parse(stdout, "", Path::new(ROOT), "dotnet-build", "dotnet")
    }

    #[test]
    fn empty_stdout() {
        let result = run("");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_error() {
        let input = "Program.cs(5,13): error CS0219: The variable 'x' is assigned but its value is never used [/home/user/app/app.csproj]\n";
        let result = run(input);
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("CS0219"));
        assert_eq!(
            d.message,
            "The variable 'x' is assigned but its value is never used"
        );
        assert_eq!(d.location, Location::File("Program.cs".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 13);
    }

    #[test]
    fn warning() {
        let input = "Program.cs(5,13): warning CS0168: The variable 'e' is declared but never used [/home/user/app/app.csproj]\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("CS0168"));
    }

    #[test]
    fn project_suffix_stripped() {
        let input = "Foo.cs(1,1): error CS0001: bad [/path/project.csproj]\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        // The message must not contain the project suffix
        assert!(!result.diagnostics[0].message.contains(".csproj"));
    }

    #[test]
    fn non_project_brackets_kept() {
        // Brackets that don't end with a project extension stay in the line.
        // "Expected [int]" is not a project suffix — it should remain.
        let input = "Foo.cs(1,1): error CS0001: Expected [int]\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("[int]"));
    }

    #[test]
    fn build_noise_ignored() {
        let input = "  Determining projects to restore...\n  All projects are up-to-date for restore.\n  app -> /home/user/app/bin/Debug/net8.0/app.dll\n";
        let result = run(input);
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn msbuild_duplicate_errors_deduplicated() {
        // MSBuild emits each error twice (compile pass + summary pass).
        // The parser must collapse them into a single diagnostic each.
        let input = "Program.cs(2,9): error CS0029: Cannot convert [/path/app.csproj]
                      Program.cs(3,1): error CS0103: Name not found [/path/app.csproj]
                      Program.cs(2,9): error CS0029: Cannot convert [/path/app.csproj]
                      Program.cs(3,1): error CS0103: Name not found [/path/app.csproj]
";
        let result = run(input);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "MSBuild duplicates must be collapsed to 2, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn strip_project_suffix_empty_brackets() {
        let result = strip_project_suffix("line[]");
        assert_eq!(result, "line[]");
    }
}
