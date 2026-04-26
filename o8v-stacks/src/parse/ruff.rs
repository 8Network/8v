//! Ruff JSON parser — covers `ruff check --output-format=json`.
//!
//! Ruff emits a single JSON array on stdout. Each element is a flat violation object.
//! Batch parse — if truncated at 1MB, falls back to `Unparsed`.
//!
//! Ruff uses 1-based row/column in its JSON (verified with ruff 0.4+).
//! `Span::new()` clamps 0→1 as a safety net if coordinates are unexpected.

use o8v_core::diagnostic::{
    Applicability, Diagnostic, Edit, ParseResult, ParseStatus, Severity, Span, Suggestion,
};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;

/// Parse ruff JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    // Parse as a JSON array of Value first, then deserialize each element
    // individually. One bad element doesn't kill the other 99 diagnostics.
    let array: Vec<serde_json::Value> = match serde_json::from_str(stdout) {
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
    let mut skipped = 0u32;
    let mut parsed_count = 0u32;
    for item in array {
        match serde_json::from_value::<RuffViolation>(item) {
            Ok(v) => {
                parsed_count += 1;
                diagnostics.push(convert(v, project_root, tool, stack));
            }
            Err(e) => {
                skipped += 1;
                tracing::debug!(error = %e, "skipping malformed ruff violation");
            }
        }
    }
    if skipped > 0 {
        tracing::warn!(
            skipped,
            total = diagnostics.len() + skipped as usize,
            "ruff: some violations could not be parsed"
        );
    }

    ParseResult {
        diagnostics,
        status: ParseStatus::Parsed,
        parsed_items: parsed_count,
    }
}

fn convert(
    v: RuffViolation,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Diagnostic {
    let location = super::normalize_path(&v.filename, project_root);

    let span = Some(Span::new(
        v.location.row,
        v.location.column,
        Some(v.end_location.row),
        Some(v.end_location.column),
    ));

    let suggestions = v
        .fix
        .map(|f| {
            let applicability = match f.applicability.as_str() {
                "safe" => Applicability::MachineApplicable,
                "unsafe" => Applicability::MaybeIncorrect,
                _ => Applicability::Unspecified,
            };

            let edits = f
                .edits
                .into_iter()
                .map(|e| Edit {
                    span: Span::new(
                        e.location.row,
                        e.location.column,
                        Some(e.end_location.row),
                        Some(e.end_location.column),
                    ),
                    new_text: e.content,
                })
                .collect();

            Suggestion {
                message: f.message,
                applicability,
                edits,
            }
        })
        .into_iter()
        .collect();

    Diagnostic {
        location,
        span,
        rule: Some(DisplayStr::from_untrusted(v.code)),
        severity: Severity::Error, // ruff only emits "error"
        raw_severity: Some(v.severity),
        message: DisplayStr::from_untrusted(v.message),
        related: vec![],
        notes: vec![],
        suggestions,
        snippet: None,
        tool: tool.to_string(),
        stack: stack.to_string(),
    }
}

// ─── Serde types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RuffViolation {
    filename: String,
    code: String,
    message: String,
    severity: String,
    location: RuffLocation,
    end_location: RuffLocation,
    fix: Option<RuffFix>,
}

#[derive(Deserialize)]
struct RuffLocation {
    row: u32,
    column: u32,
}

#[derive(Deserialize)]
struct RuffFix {
    message: String,
    applicability: String,
    edits: Vec<RuffEdit>,
}

#[derive(Deserialize)]
struct RuffEdit {
    content: String,
    location: RuffLocation,
    end_location: RuffLocation,
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
    fn empty_array() {
        let result = parse("[]", "", root(), "ruff", "python");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn invalid_json() {
        let result = parse("not json at all", "", root(), "ruff", "python");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn single_violation() {
        let stdout = r#"[{"filename":"src/app.py","code":"F401","message":"os imported but unused","severity":"error","location":{"row":1,"column":1},"end_location":{"row":1,"column":10},"fix":null}]"#;
        let result = parse(stdout, "", root(), "ruff", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.rule.as_deref(), Some("F401"));
        assert_eq!(d.message, "os imported but unused");
        assert_eq!(d.location, Location::File("src/app.py".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, Some(1));
        assert_eq!(span.end_column, Some(10));
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.tool, "ruff");
        assert_eq!(d.stack, "python");
    }

    #[test]
    fn violation_with_fix() {
        let stdout = r#"[{"filename":"src/app.py","code":"F401","message":"os imported but unused","severity":"error","location":{"row":1,"column":1},"end_location":{"row":1,"column":10},"fix":{"message":"Remove import","applicability":"safe","edits":[{"content":"","location":{"row":1,"column":1},"end_location":{"row":1,"column":10}}]}}]"#;
        let result = parse(stdout, "", root(), "ruff", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.suggestions.len(), 1);
        let s = &d.suggestions[0];
        assert_eq!(s.message, "Remove import");
        assert!(matches!(s.applicability, Applicability::MachineApplicable));
        assert_eq!(s.edits.len(), 1);
        assert_eq!(s.edits[0].new_text, "");
    }

    #[test]
    fn malformed_element() {
        // First element is valid, second is missing required fields
        let stdout = r#"[{"filename":"src/app.py","code":"F401","message":"unused","severity":"error","location":{"row":1,"column":1},"end_location":{"row":1,"column":10},"fix":null},{"bad":"data"}]"#;
        let result = parse(stdout, "", root(), "ruff", "python");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].rule.as_deref(), Some("F401"));
    }
}
