// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

/// Sanitize a string for single-line terminal display.
///
/// Delegates to `crate::sanitize` — the single canonical implementation
/// that strips ANSI escape sequences and control characters (preserving tabs).
/// One function, one place, one behavior.
#[must_use]
pub fn sanitize_for_display(s: &str) -> String {
    crate::sanitize(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // sanitize_for_display delegates to crate::sanitize — test the
    // delegation and key behaviors. Comprehensive ANSI tests live in o8v-check.

    #[test]
    fn sanitize_strips_ansi() {
        assert_eq!(sanitize_for_display("\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_for_display("a\x01b\x02c"), "abc");
        assert_eq!(sanitize_for_display("hello\x07world"), "helloworld");
        assert_eq!(sanitize_for_display("a\x7fb"), "ab");
    }

    #[test]
    fn sanitize_preserves_tabs_strips_newlines() {
        assert_eq!(sanitize_for_display("a\tb"), "a\tb");
        assert_eq!(sanitize_for_display("a\nb"), "ab");
        assert_eq!(sanitize_for_display("a\rb"), "ab");
    }

    #[test]
    fn sanitize_clean_string_unchanged() {
        assert_eq!(sanitize_for_display("hello world"), "hello world");
    }
}
