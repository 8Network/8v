// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! ANSI escape validation — assert diagnostics contain no color codes.

use o8v_core::{CheckOutcome, CheckReport, Location};

/// Assert that all diagnostics in the report contain no ANSI escape sequences.
///
/// Checks every field that may contain text: message, rule, location, snippet,
/// related span labels and locations, notes, and suggestion messages.
///
/// Panics if any ANSI escape sequence is found (e.g., `\x1b[31m` for red).
pub fn assert_sanitized(report: &CheckReport) {
    for result in report.results() {
        for entry in result.entries() {
            let diagnostics = match entry.outcome() {
                CheckOutcome::Failed { diagnostics, .. }
                | CheckOutcome::Passed { diagnostics, .. } => diagnostics,
                CheckOutcome::Error { .. } => continue,
                // Non-exhaustive: ignore future variants.
                #[allow(unreachable_patterns)]
                _ => continue,
            };
            for diagnostic in diagnostics {
                assert_no_ansi(&diagnostic.message, "message");
                if let Some(rule) = &diagnostic.rule {
                    assert_no_ansi(rule, "rule");
                }

                match &diagnostic.location {
                    Location::File(path) => {
                        assert_no_ansi(path, "location.file");
                    }
                    Location::Absolute(path) => {
                        assert_no_ansi(path, "location.absolute");
                    }
                    // Non-exhaustive: ignore future variants.
                    #[allow(unreachable_patterns)]
                    _ => {}
                }

                if let Some(snippet) = &diagnostic.snippet {
                    assert_no_ansi(snippet, "snippet");
                }

                for related in &diagnostic.related {
                    assert_no_ansi(&related.label, "related.label");
                    match &related.location {
                        Location::File(path) => {
                            assert_no_ansi(path, "related.location.file");
                        }
                        Location::Absolute(path) => {
                            assert_no_ansi(path, "related.location.absolute");
                        }
                        // Non-exhaustive: ignore future variants.
                        #[allow(unreachable_patterns)]
                        _ => {}
                    }
                }

                for note in &diagnostic.notes {
                    assert_no_ansi(note, "note");
                }

                for suggestion in &diagnostic.suggestions {
                    assert_no_ansi(&suggestion.message, "suggestion.message");
                }
            }
        }
    }
}

/// Check a string for ANSI escape sequences. Panics with context if found.
fn assert_no_ansi(text: &str, context: &str) {
    if text.contains('\x1b') {
        panic!(
            "diagnostic field '{}' contains ANSI escape sequence: {:?}",
            context, text
        );
    }
}
