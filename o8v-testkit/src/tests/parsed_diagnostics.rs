// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

// ─── coverage: assert_parsed_diagnostics ────────────────────────

#[test]
fn assert_parsed_diagnostics_passes_on_valid_parse() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: Some(o8v_core::DisplayStr::from_untrusted("E0001")),
            severity: Severity::Error,
            raw_severity: Some("error".to_string()),
            message: o8v_core::DisplayStr::from_untrusted("test error"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "rustc".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Parsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "produced no diagnostics")]
fn assert_parsed_diagnostics_panics_on_empty() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![],
        status: ParseStatus::Parsed,
        parsed_items: 0,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "expected Parsed status")]
fn assert_parsed_diagnostics_panics_on_wrong_status() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: o8v_core::DisplayStr::from_untrusted("err"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "rustc".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Unparsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}

#[test]
#[should_panic(expected = "tool field mismatch")]
fn assert_parsed_diagnostics_panics_on_tool_mismatch() {
    use o8v_core::diagnostic::*;
    let result = ParseResult {
        diagnostics: vec![Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: None,
            severity: Severity::Error,
            raw_severity: None,
            message: o8v_core::DisplayStr::from_untrusted("err"),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "wrong".to_string(),
            stack: "rust".to_string(),
        }],
        status: ParseStatus::Parsed,
        parsed_items: 1,
    };
    assert_parsed_diagnostics(&result, "rustc", "rust");
}
