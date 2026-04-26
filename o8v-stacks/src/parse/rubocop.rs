//! Rubocop JSON parser — covers `rubocop --format json`.
//!
//! Rubocop emits a single JSON object on stdout containing:
//! - `files`: array of file objects with path and offenses
//! - `summary`: metadata about the check run
//!
//! Each file contains offenses with location, severity, message, and cop_name.
//! Severity maps: "fatal"/"error" → Error, "warning" → Warning, "convention"/"refactor" → Warning, "info" → Info.

use o8v_core::diagnostic::{Diagnostic, ParseResult, ParseStatus, Severity, Span};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse rubocop JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let output: RubocopOutput = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => {
            return ParseResult {
                diagnostics: vec![],
                status: ParseStatus::Unparsed,
                parsed_items: 0,
            }
        }
    };

    let mut diagnostics = Vec::new();
    let mut parsed_count = 0u32;

    for file in output.files {
        for offense in file.offenses {
            parsed_count += 1;
            diagnostics.push(convert(&file.path, offense, project_root, tool, stack));
        }
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: parsed_count,
    }
}

fn convert(
    path: &str,
    offense: RubocopOffense,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Diagnostic {
    let location = super::normalize_path(path, project_root);

    let severity = match offense.severity.as_str() {
        "fatal" | "error" => Severity::Error,
        "warning" => Severity::Warning,
        "convention" | "refactor" => Severity::Warning,
        "info" => Severity::Info,
        _ => Severity::Warning,
    };

    let span = Some(Span::new(
        offense.location.start_line,
        offense.location.start_column,
        offense.location.last_line,
        offense.location.last_column,
    ));

    Diagnostic {
        location,
        span,
        rule: Some(DisplayStr::from_untrusted(offense.cop_name)),
        severity,
        raw_severity: Some(offense.severity),
        message: DisplayStr::from_untrusted(offense.message),
        related: vec![],
        notes: vec![],
        suggestions: vec![],
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RubocopOutput {
    files: Vec<RubocopFile>,
}

#[derive(Deserialize)]
struct RubocopFile {
    path: String,
    offenses: Vec<RubocopOffense>,
}

#[derive(Deserialize)]
struct RubocopOffense {
    severity: String,
    message: String,
    cop_name: String,
    location: RubocopLocation,
}

#[derive(Deserialize)]
struct RubocopLocation {
    #[serde(rename = "start_line")]
    start_line: u32,
    #[serde(rename = "start_column")]
    start_column: u32,
    #[serde(rename = "last_line")]
    last_line: Option<u32>,
    #[serde(rename = "last_column")]
    last_column: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::diagnostic::*;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/project")
    }

    #[test]
    fn empty_files() {
        let stdout = r#"{"files":[],"summary":{"offense_count":0,"target_file_count":0,"inspected_file_count":0}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json", "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn single_offense() {
        let stdout = r#"{"files":[{"path":"app/main.rb","offenses":[{"severity":"warning","message":"Use 2 spaces for indentation.","cop_name":"Layout/IndentationWidth","location":{"start_line":1,"start_column":1,"last_line":1,"last_column":5,"length":5,"line":1,"column":1}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("Layout/IndentationWidth"));
        assert_eq!(d.message, "Use 2 spaces for indentation.");
        assert_eq!(d.location, Location::File("app/main.rb".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, Some(1));
        assert_eq!(span.end_column, Some(5));
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.tool, "rubocop");
        assert_eq!(d.stack, "ruby");
    }

    #[test]
    fn error_severity() {
        let stdout = r#"{"files":[{"path":"lib/app.rb","offenses":[{"severity":"error","message":"Undefined local variable.","cop_name":"NameError","location":{"start_line":5,"start_column":3,"last_line":5,"last_column":10}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.raw_severity, Some("error".to_string()));
    }

    #[test]
    fn fatal_severity() {
        let stdout = r#"{"files":[{"path":"bad.rb","offenses":[{"severity":"fatal","message":"Syntax error.","cop_name":"Syntax","location":{"start_line":1,"start_column":1}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn convention_severity() {
        let stdout = r#"{"files":[{"path":"style.rb","offenses":[{"severity":"convention","message":"Prefer single quotes.","cop_name":"Style/SingleQuotes","location":{"start_line":2,"start_column":5}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn info_severity() {
        let stdout = r#"{"files":[{"path":"doc.rb","offenses":[{"severity":"info","message":"Missing documentation.","cop_name":"Style/Documentation","location":{"start_line":1,"start_column":1}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn multiple_offenses_in_file() {
        let stdout = r#"{"files":[{"path":"app.rb","offenses":[{"severity":"warning","message":"First offense.","cop_name":"Rule1","location":{"start_line":1,"start_column":1}},{"severity":"error","message":"Second offense.","cop_name":"Rule2","location":{"start_line":5,"start_column":3}}]}],"summary":{"offense_count":2,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("Rule1"));
        assert_eq!(result.diagnostics[1].rule.as_deref(), Some("Rule2"));
    }

    #[test]
    fn multiple_files() {
        let stdout = r#"{"files":[{"path":"app.rb","offenses":[{"severity":"warning","message":"Issue in app.","cop_name":"Rule1","location":{"start_line":1,"start_column":1}}]},{"path":"lib.rb","offenses":[{"severity":"error","message":"Issue in lib.","cop_name":"Rule2","location":{"start_line":2,"start_column":3}}]}],"summary":{"offense_count":2,"target_file_count":2,"inspected_file_count":2}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("app.rb".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::File("lib.rb".to_string())
        );
    }

    #[test]
    fn offense_with_optional_end_line() {
        let stdout = r#"{"files":[{"path":"code.rb","offenses":[{"severity":"warning","message":"Error message.","cop_name":"Rule/Name","location":{"start_line":3,"start_column":5,"last_line":3,"last_column":10}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.end_line, Some(3));
        assert_eq!(span.end_column, Some(10));
    }

    #[test]
    fn offense_without_end_line() {
        let stdout = r#"{"files":[{"path":"code.rb","offenses":[{"severity":"warning","message":"Error message.","cop_name":"Rule/Name","location":{"start_line":3,"start_column":5}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(result.diagnostics.len(), 1);
        let span = result.diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.end_line, None);
        assert_eq!(span.end_column, None);
    }

    #[test]
    fn absolute_path_normalization() {
        let stdout = r#"{"files":[{"path":"/project/app.rb","offenses":[{"severity":"warning","message":"Message.","cop_name":"Rule","location":{"start_line":1,"start_column":1}}]}],"summary":{"offense_count":1,"target_file_count":1,"inspected_file_count":1}}"#;
        let result = parse(stdout, "", root(), "rubocop", "ruby");
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("app.rb".to_string())
        );
    }
}
