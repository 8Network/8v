//! Cargo JSON parser — covers `cargo check` and `cargo clippy`.
//!
//! Cargo `--message-format=json` emits NDJSON (one JSON object per line).
//! Filter for `reason: "compiler-message"` to extract diagnostics.
//! Other events (`build-script-executed`, `build-finished`) are ignored.

use o8v_core::diagnostic::{
    Applicability, Diagnostic, Edit, Location, ParseResult, ParseStatus, RelatedSpan, Severity,
    Span, Suggestion,
};
use o8v_core::display_str::DisplayStr;
use serde::Deserialize;
use std::collections::HashSet;

/// Parse cargo/clippy JSON output into diagnostics.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> ParseResult {
    let mut diagnostics = Vec::new();
    let mut parsed_any = false;
    let mut parsed_count = 0u32;
    // Fingerprint: (message, severity_str, rule, file_path, line, column)
    let mut seen: HashSet<(String, String, Option<String>, String, u32, u32)> = HashSet::new();
    let mut dedup_count = 0u32;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Each line is a JSON object. Parse it.
        let Ok(event) = serde_json::from_str::<CargoEvent>(line) else {
            tracing::debug!(line, "skipping non-JSON line in cargo output");
            continue;
        };
        parsed_any = true;
        parsed_count += 1;

        // Only process compiler messages.
        if event.reason != "compiler-message" {
            continue;
        }

        let Some(msg) = event.message else {
            continue;
        };

        let diagnostic = convert_message(&msg, project_root, tool, stack);

        // Deduplicate: cargo compiles each crate twice (lib + test pass),
        // producing identical diagnostics for both passes.
        let file_path = match &diagnostic.location {
            Location::File(p) | Location::Absolute(p) => p.clone(),
            _ => String::new(),
        };
        let (line_start, col_start) = diagnostic
            .span
            .as_ref()
            .map_or((0u32, 0u32), |s| (s.line, s.column));
        let fingerprint = (
            diagnostic.message.to_string(),
            diagnostic.raw_severity.clone().unwrap_or_default(),
            diagnostic.rule.as_ref().map(|r| r.to_string()),
            file_path,
            line_start,
            col_start,
        );

        if seen.contains(&fingerprint) {
            tracing::debug!(
                message = %diagnostic.message,
                "skipping duplicate diagnostic"
            );
            dedup_count += 1;
            continue;
        }
        seen.insert(fingerprint);

        diagnostics.push(diagnostic);
    }

    if dedup_count > 0 {
        tracing::warn!(
            dedup_count,
            "deduplicated duplicate cargo diagnostics (likely lib+test double-compilation)"
        );
    }

    // Filter out diagnostics from external code (e.g., nightly stdlib paths).
    // These have Location::Absolute(...) with non-empty paths (outside project root).
    // Spanless messages have Location::Absolute("") and are real compiler output — keep them.
    let mut filtered_count = 0u32;
    diagnostics.retain(|d| {
        if let Location::Absolute(path) = &d.location {
            if !path.is_empty() {
                // Non-empty absolute path = external code, filter it out.
                filtered_count += 1;
                return false;
            }
        }
        true
    });

    if filtered_count > 0 {
        tracing::debug!(
            filtered_count,
            "filtered out diagnostics from external code (nightly stdlib or toolchain paths)"
        );
    }

    let parse_status = if !diagnostics.is_empty() {
        ParseStatus::Parsed
    } else if parsed_any {
        // We parsed JSON events but found no diagnostics — tool passed.
        ParseStatus::Parsed
    } else if stdout.trim().is_empty() {
        ParseStatus::Parsed // empty stdout = no output
    } else {
        ParseStatus::Unparsed // couldn't parse anything
    };

    ParseResult {
        diagnostics,
        status: parse_status,
        parsed_items: parsed_count,
    }
}

/// Convert a Cargo compiler message into our Diagnostic.
#[allow(clippy::too_many_lines)]
fn convert_message(
    msg: &CargoMessage,
    project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Diagnostic {
    let primary_span = msg.spans.iter().find(|s| s.is_primary);

    // Spanless messages (linking errors, build script failures, macro diagnostics)
    // are real compiler output — don't drop them.
    let (location, span, snippet) = primary_span.map_or_else(
        || (Location::Absolute(String::new()), None, None),
        |ps| {
            (
                super::normalize_path(&ps.file_name, project_root),
                Some(Span::new(
                    ps.line_start,
                    ps.column_start,
                    Some(ps.line_end),
                    Some(ps.column_end),
                )),
                ps.text.first().map(|t| t.text.clone()),
            )
        },
    );

    let severity = match msg.level.as_str() {
        "warning" => Severity::Warning,
        "note" => Severity::Info,
        "help" => Severity::Hint,
        _ => Severity::Error,
    };

    let rule = msg
        .code
        .as_ref()
        .map(|c| DisplayStr::from_untrusted(c.code.clone()));

    // Related spans — non-primary spans with labels.
    let related = msg
        .spans
        .iter()
        .filter(|s| !s.is_primary)
        .filter_map(|s| {
            let label = s.label.clone()?;
            Some(RelatedSpan {
                location: super::normalize_path(&s.file_name, project_root),
                span: Span::new(
                    s.line_start,
                    s.column_start,
                    Some(s.line_end),
                    Some(s.column_end),
                ),
                label,
            })
        })
        .collect();

    // Notes — from children with level "note" or "help" (without suggestions).
    // Walk recursively — Cargo nests children for explanation chains.
    let mut notes = Vec::new();
    collect_notes(&msg.children, &mut notes);

    // Suggestions — from children that have spans with suggested_replacement.
    let suggestions = msg
        .children
        .iter()
        .filter(|c| !c.spans.is_empty())
        .map(|c| {
            let edits = c
                .spans
                .iter()
                .filter_map(|s| {
                    let replacement = s.suggested_replacement.clone()?;
                    let applicability = match s.suggestion_applicability.as_deref() {
                        Some("MachineApplicable") => Applicability::MachineApplicable,
                        Some("MaybeIncorrect") => Applicability::MaybeIncorrect,
                        Some("HasPlaceholders") => Applicability::HasPlaceholders,
                        _ => Applicability::Unspecified,
                    };
                    Some((
                        Edit {
                            span: Span::new(
                                s.line_start,
                                s.column_start,
                                Some(s.line_end),
                                Some(s.column_end),
                            ),
                            new_text: replacement,
                        },
                        applicability,
                    ))
                })
                .collect::<Vec<_>>();

            let applicability = edits
                .first()
                .map_or(Applicability::Unspecified, |(_, a)| a.clone());

            Suggestion {
                message: c.message.clone(),
                applicability,
                edits: edits.into_iter().map(|(e, _)| e).collect(),
            }
        })
        .collect();

    Diagnostic {
        location,
        span,
        rule,
        severity,
        raw_severity: Some(msg.level.clone()),
        message: DisplayStr::from_untrusted(msg.message.clone()),
        related,
        notes,
        suggestions,
        snippet,
        tool: tool.to_string(),
        stack: stack.to_string(),
    }
}

/// Recursively collect notes from children (and their children).
/// Children with spans are suggestions (handled separately).
fn collect_notes(children: &[CargoChild], notes: &mut Vec<String>) {
    for c in children {
        if c.spans.is_empty() {
            notes.push(format!("{}: {}", c.level, c.message));
        }
        collect_notes(&c.children, notes);
    }
}

// ─── Serde types for Cargo JSON ──────────────────────────────────────────

#[derive(Deserialize)]
struct CargoEvent {
    reason: String,
    message: Option<CargoMessage>,
}

#[derive(Deserialize)]
struct CargoMessage {
    message: String,
    level: String,
    code: Option<CargoCode>,
    #[serde(default)]
    spans: Vec<CargoSpan>,
    #[serde(default)]
    children: Vec<CargoChild>,
    #[allow(dead_code)]
    rendered: Option<String>,
}

#[derive(Deserialize)]
struct CargoCode {
    code: String,
}

#[derive(Deserialize)]
struct CargoSpan {
    file_name: String,
    line_start: u32,
    line_end: u32,
    column_start: u32,
    column_end: u32,
    is_primary: bool,
    label: Option<String>,
    suggested_replacement: Option<String>,
    suggestion_applicability: Option<String>,
    text: Vec<CargoSpanText>,
}

#[derive(Deserialize)]
struct CargoSpanText {
    text: String,
}

#[derive(Deserialize)]
struct CargoChild {
    message: String,
    level: String,
    spans: Vec<CargoSpan>,
    #[allow(dead_code)]
    children: Vec<Self>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/project")
    }

    #[test]
    fn empty_stdout() {
        let result = parse("", "", root(), "cargo", "rust");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn non_json_lines() {
        let stdout = "Compiling foo v0.1.0\nFinished dev profile\n";
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Unparsed);
    }

    #[test]
    fn build_finished_only() {
        let stdout = r#"{"reason":"build-finished","success":true}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.diagnostics.len(), 0);
        assert_eq!(result.status, ParseStatus::Parsed);
    }

    #[test]
    fn single_error() {
        let stdout = r#"{"reason":"compiler-message","message":{"message":"cannot find value `x`","code":{"code":"E0425"},"level":"error","spans":[{"file_name":"src/main.rs","line_start":5,"line_end":5,"column_start":10,"column_end":11,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let y = x;"}]}],"children":[],"rendered":"error[E0425]: ..."}}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.rule.as_deref(), Some("E0425"));
        assert_eq!(d.message, "cannot find value `x`");
        assert_eq!(d.location, Location::File("src/main.rs".to_string()));
        let span = d.span.as_ref().unwrap();
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 10);
        assert_eq!(span.end_line, Some(5));
        assert_eq!(span.end_column, Some(11));
    }

    #[test]
    fn warning_with_code() {
        let stdout = r##"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/main.rs","line_start":2,"line_end":2,"column_start":9,"column_end":10,"is_primary":true,"label":"help: if this is intentional, prefix it with an underscore","suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 1;"}]}],"children":[{"message":"#[warn(unused_variables)] on by default","level":"note","spans":[],"children":[]}],"rendered":"warning..."}}"##;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.rule.as_deref(), Some("unused_variables"));
    }

    #[test]
    fn suggestion_with_replacement() {
        let stdout = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/main.rs","line_start":2,"line_end":2,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 1;"}]}],"children":[{"message":"if this is intentional, prefix it with an underscore: `_x`","level":"help","spans":[{"file_name":"src/main.rs","line_start":2,"line_end":2,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":"_x","suggestion_applicability":"MachineApplicable","text":[{"text":"    let x = 1;"}]}],"children":[]}],"rendered":"warning..."}}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.suggestions.len(), 1);
        let s = &d.suggestions[0];
        assert!(matches!(s.applicability, Applicability::MachineApplicable));
        assert_eq!(s.edits.len(), 1);
        assert_eq!(s.edits[0].new_text, "_x");
    }

    #[test]
    fn spanless_message() {
        let stdout = r#"{"reason":"compiler-message","message":{"message":"linking with `cc` failed","code":null,"level":"error","spans":[],"children":[],"rendered":"error: linking with `cc` failed"}}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.location, Location::Absolute(String::new()));
        assert!(d.span.is_none());
        assert_eq!(d.message, "linking with `cc` failed");
    }

    #[test]
    fn nested_children_notes() {
        let stdout = r#"{"reason":"compiler-message","message":{"message":"mismatched types","code":{"code":"E0308"},"level":"error","spans":[{"file_name":"src/main.rs","line_start":3,"line_end":3,"column_start":5,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    x"}]}],"children":[{"message":"expected `u32`, found `&str`","level":"note","spans":[],"children":[{"message":"see https://doc.rust-lang.org/error-index.html#E0308","level":"help","spans":[],"children":[]}]}],"rendered":"error..."}}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.notes[0], "note: expected `u32`, found `&str`");
        assert_eq!(
            d.notes[1],
            "help: see https://doc.rust-lang.org/error-index.html#E0308"
        );
    }

    #[test]
    fn related_spans() {
        let stdout = r#"{"reason":"compiler-message","message":{"message":"use of moved value: `x`","code":{"code":"E0382"},"level":"error","spans":[{"file_name":"src/main.rs","line_start":5,"line_end":5,"column_start":10,"column_end":11,"is_primary":true,"label":"value used here after move","suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    dbg!(x);"}]},{"file_name":"src/main.rs","line_start":4,"line_end":4,"column_start":10,"column_end":11,"is_primary":false,"label":"value moved here","suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    drop(x);"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.related.len(), 1);
        assert_eq!(d.related[0].label, "value moved here");
        assert_eq!(
            d.related[0].location,
            Location::File("src/main.rs".to_string())
        );
        assert_eq!(d.related[0].span.line, 4);
    }

    #[test]
    fn deduplicates_identical_diagnostics() {
        // Simulates cargo compiling lib + test pass: same diagnostic emitted twice.
        let event = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":3,"line_end":3,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 1;"}]}],"children":[],"rendered":"warning..."}}"#;
        // Feed the same event three times (lib pass, test pass, maybe once more).
        let stdout = format!("{event}\n{event}\n{event}\n");
        let result = parse(&stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Only one unique diagnostic should survive deduplication.
        assert_eq!(result.diagnostics.len(), 1, "duplicates must be dropped");
        // parsed_items counts all three JSON lines parsed.
        assert_eq!(result.parsed_items, 3);
    }

    #[test]
    fn deduplicates_keeps_distinct_diagnostics() {
        // Two diagnostics that differ only in line number must both survive.
        let event_a = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":3,"line_end":3,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 1;"}]}],"children":[],"rendered":"warning..."}}"#;
        let event_b = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":7,"line_end":7,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 2;"}]}],"children":[],"rendered":"warning..."}}"#;
        let stdout = format!("{event_a}\n{event_b}\n{event_a}\n{event_b}\n");
        let result = parse(&stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "distinct diagnostics must all survive"
        );
    }

    #[test]
    fn filters_out_external_absolute_paths() {
        // Nightly stdlib diagnostic with an absolute path (e.g., ~/.rustup/toolchains/.../library/core/...).
        // normalize_path returns Location::Absolute(path) for paths outside project root.
        let external_lib = r#"{"reason":"compiler-message","message":{"message":"unstable feature: never_type","code":{"code":"E0658"},"level":"warning","spans":[{"file_name":"/Users/user/.rustup/toolchains/nightly-x86_64-apple-darwin/lib/rustlib/src/rust/library/core/src/option.rs","line_start":1234,"line_end":1234,"column_start":5,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    fn foo() -> !"}]}],"children":[],"rendered":"warning..."}}"#;
        let result = parse(external_lib, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should be filtered out because it has a non-empty Location::Absolute(...).
        assert_eq!(
            result.diagnostics.len(),
            0,
            "diagnostics from external code (stdlib, toolchain) must be filtered"
        );
    }

    #[test]
    fn keeps_spanless_and_relative_filters_external() {
        // Mix: spanless (empty absolute), project-relative file, and external lib.
        let normal = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","code":{"code":"unused_variables"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":3,"line_end":3,"column_start":9,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    let x = 1;"}]}],"children":[],"rendered":"warning..."}}"#;
        let spanless = r#"{"reason":"compiler-message","message":{"message":"linking with `cc` failed","code":null,"level":"error","spans":[],"children":[],"rendered":"error: linking with `cc` failed"}}"#;
        let external_lib = r#"{"reason":"compiler-message","message":{"message":"unstable feature","code":{"code":"E0658"},"level":"warning","spans":[{"file_name":"/Users/user/.rustup/toolchains/nightly-x86_64-apple-darwin/lib/rustlib/src/rust/library/core/src/option.rs","line_start":100,"line_end":100,"column_start":5,"column_end":10,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"    fn foo()"}]}],"children":[],"rendered":"warning..."}}"#;
        let stdout = format!("{normal}\n{spanless}\n{external_lib}\n");
        let result = parse(&stdout, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Normal (relative file) and spanless (empty absolute) should survive.
        // External lib should be filtered out.
        assert_eq!(
            result.diagnostics.len(),
            2,
            "keep spanless and relative, filter external"
        );
        assert_eq!(
            result.diagnostics[0].location,
            Location::File("src/lib.rs".to_string())
        );
        assert_eq!(
            result.diagnostics[1].location,
            Location::Absolute(String::new()),
            "spanless messages (empty absolute) should be kept"
        );
    }

    // ─── Stress tests ───────────────────────────────────────────────────────

    #[test]
    fn stress_cargo_empty_input() {
        let result = parse("", "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items < 100);
    }

    #[test]
    fn stress_cargo_huge_input() {
        let event = r#"{"reason":"compiler-message","message":{"message":"test message","code":{"code":"E0000"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"warning..."}}"#;
        let huge = (0..100_000).map(|_| event).collect::<Vec<_>>().join("\n");
        let result = parse(&huge, "", root(), "cargo", "rust");
        // Should not panic, regardless of parse status
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
        assert!(result.parsed_items < u32::MAX);
    }

    #[test]
    fn stress_cargo_binary_garbage() {
        let garbage = "{\x00\x01\x02 invalid json }\x00";
        let result = parse(garbage, "", root(), "cargo", "rust");
        // Should not panic, handle gracefully
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
        assert!(result.parsed_items < 100);
    }

    #[test]
    fn stress_cargo_whitespace_only() {
        let result = parse("   \n\n\t\t\n   ", "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn stress_cargo_truncated_json() {
        let truncated = r#"{"reason":"compiler-message","message":{"message":"incomplete"#;
        let result = parse(truncated, "", root(), "cargo", "rust");
        // Should not panic on incomplete JSON
        assert!(matches!(
            result.status,
            ParseStatus::Parsed | ParseStatus::Unparsed
        ));
    }

    #[test]
    fn stress_cargo_unicode_in_paths() {
        let event = r#"{"reason":"compiler-message","message":{"message":"错误测试 🔥","code":{"code":"E0001"},"level":"error","spans":[{"file_name":"src/文件.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":"RTL: תוקן עברית","suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should parse CJK, emoji, RTL text without crashing
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_cargo_extremely_long_line() {
        let long_msg = "x".repeat(1_000_000);
        let event = format!(
            r#"{{"reason":"compiler-message","message":{{"message":"{}","code":{{"code":"E0000"}},"level":"error","spans":[{{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{{"text":"code"}}]}}],"children":[],"rendered":"error..."}}}}
"#,
            long_msg
        );
        let result = parse(&event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should not crash on 1MB+ line
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn stress_cargo_deeply_nested_children() {
        // Build deeply nested children structure
        let mut nested =
            r#"{"level":"info","message":"level 9","spans":[],"children":[]}"#.to_string();
        for i in (1..10).rev() {
            nested = format!(
                r#"{{"level":"info","message":"level {}","spans":[],"children":[{}]}}"#,
                i, nested
            );
        }
        let event = format!(
            r#"{{"reason":"compiler-message","message":{{"message":"root","code":{{"code":"E0000"}},"level":"error","spans":[{{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{{"text":"code"}}]}}],"children":[{}],"rendered":"error..."}}}}"#,
            nested
        );
        let result = parse(&event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        // Should handle deep nesting
        assert!(!result.diagnostics.is_empty());
    }

    // ─── Security Tests ────────────────────────────────────────────────────

    #[test]
    fn security_cargo_dos_many_events() {
        // 100,000 cargo events (NDJSON lines) should be parsed efficiently
        let event = r#"{"reason":"compiler-message","message":{"message":"test","code":{"code":"E0001"},"level":"warning","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"warning..."}}"#;
        let huge = (0..100_000).map(|_| event).collect::<Vec<_>>().join("\n");
        let result = parse(&huge, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(result.parsed_items > 0);
    }

    #[test]
    fn security_cargo_injection_quote_in_message() {
        // Quote character in message should not break NDJSON parsing
        let event = r#"{"reason":"compiler-message","message":{"message":"error: \"quoted\" message","code":{"code":"E0001"},"level":"error","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics[0].message.contains("quoted"));
    }

    #[test]
    fn security_cargo_injection_control_chars_in_message() {
        // Control characters in message (escaped in JSON)
        let event = r#"{"reason":"compiler-message","message":{"message":"error\nwith\nnewlines","code":{"code":"E0001"},"level":"error","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
        // DisplayStr::from_untrusted strips newlines at parse time — no downstream injection risk.
        assert!(!result.diagnostics[0].message.contains('\n'));
        assert!(result.diagnostics[0].message.contains("error"));
    }

    #[test]
    fn security_cargo_injection_html_in_message() {
        // HTML-like tags in message should not be interpreted
        let event = r#"{"reason":"compiler-message","message":{"message":"error: <script>alert('xss')</script>","code":{"code":"E0001"},"level":"error","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
        // Script tag should be preserved as literal text, not interpreted
        assert!(result.diagnostics[0].message.contains("<script>"));
    }

    #[test]
    fn security_cargo_injection_backslash_in_message() {
        // Backslashes in message
        let event = r#"{"reason":"compiler-message","message":{"message":"path: C:\\Users\\test","code":{"code":"E0001"},"level":"error","spans":[{"file_name":"src/lib.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"label":null,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"text":"code"}]}],"children":[],"rendered":"error..."}}"#;
        let result = parse(event, "", root(), "cargo", "rust");
        assert_eq!(result.status, ParseStatus::Parsed);
        assert!(!result.diagnostics.is_empty());
        // Backslash should be in message
        assert!(result.diagnostics[0].message.contains("C:"));
    }
}
