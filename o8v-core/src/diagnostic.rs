//! Universal diagnostic contract — one shape for every tool, every stack.
//!
//! See `docs/diagnostic-design.md` for the full design.

use crate::display_str::DisplayStr;
use serde::Serialize;

/// One error/warning from any tool, any stack.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Where this diagnostic originates.
    pub location: Location,

    /// Primary span. `None` if the tool provides no location.
    pub span: Option<Span>,

    /// Rule identifier — the tool's own name, preserved exactly.
    /// e.g. `"clippy::unused_variable"`, `"no-unused-vars"`, `"F401"`, `"TS2304"`.
    ///
    /// `DisplayStr` enforces sanitization at construction — any unsanitized `String`
    /// is a compile error. Use `DisplayStr::from_untrusted(raw_rule)` in parsers.
    pub rule: Option<DisplayStr>,

    /// Normalized severity.
    pub severity: Severity,

    /// Raw severity string as the tool reported it.
    /// `ESLint` uses `"1"`/`"2"`, go vet has none.
    pub raw_severity: Option<String>,

    /// The primary error message.
    ///
    /// `DisplayStr` enforces sanitization at construction — any unsanitized `String`
    /// is a compile error. Use `DisplayStr::from_untrusted(raw_msg)` in parsers.
    pub message: DisplayStr,

    /// Related spans — additional labeled locations.
    pub related: Vec<RelatedSpan>,

    /// Child notes — `"help:"`, `"note:"`, `"hint:"` messages.
    pub notes: Vec<String>,

    /// Suggested fixes. May have multiple per diagnostic.
    pub suggestions: Vec<Suggestion>,

    /// Source code snippet at the primary span.
    pub snippet: Option<String>,

    /// Which tool produced this. Injected by the check runner.
    pub tool: String,

    /// Which stack. Injected by the check runner.
    pub stack: String,
}

/// Where a diagnostic originates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
#[serde(tag = "type", content = "path")]
pub enum Location {
    /// Relative path within the project (normalized from tool's absolute path).
    File(String),
    /// Absolute path outside the project root.
    Absolute(String),
}

/// A location in source code. All coordinates are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    /// Line number (1-based, minimum 1).
    pub line: u32,
    /// Column number (1-based, minimum 1).
    pub column: u32,
    /// End line, for multi-line spans.
    pub end_line: Option<u32>,
    /// End column.
    pub end_column: Option<u32>,
}

impl Span {
    /// Create a span, clamping zero coordinates to 1 and normalizing
    /// backwards end positions (end before start → dropped).
    #[must_use]
    pub fn new(line: u32, column: u32, end_line: Option<u32>, end_column: Option<u32>) -> Self {
        let line = line.max(1);
        let column = column.max(1);

        // Drop end coordinates if they point before the start.
        let (end_line, end_column) = match (end_line, end_column) {
            (Some(el), Some(ec)) => {
                let el = el.max(1);
                let ec = ec.max(1);
                if el < line || (el == line && ec < column) {
                    (None, None)
                } else {
                    (Some(el), Some(ec))
                }
            }
            (Some(el), None) => {
                let el = el.max(1);
                if el < line {
                    (None, None)
                } else {
                    (Some(el), None)
                }
            }
            // end_column without end_line is meaningless — drop it.
            (None, _) => (None, None),
        };

        Self {
            line,
            column,
            end_line,
            end_column,
        }
    }
}

/// A secondary location referenced by a diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct RelatedSpan {
    /// Where.
    pub location: Location,
    /// Span within that location.
    pub span: Span,
    /// Label — e.g. `"defined here"`, `"first used here"`.
    pub label: String,
}

/// A suggested fix.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    /// Human-readable description.
    pub message: String,
    /// Whether this fix can be applied automatically.
    pub applicability: Applicability,
    /// The actual edits. Empty if the tool only provides a message.
    pub edits: Vec<Edit>,
}

/// Whether a fix can be applied automatically.
#[derive(Debug, Clone, Serialize)]
pub enum Applicability {
    /// Safe to apply without human review.
    MachineApplicable,
    /// Might change semantics — needs review.
    MaybeIncorrect,
    /// Contains placeholders that need filling.
    HasPlaceholders,
    /// Tool doesn't specify.
    Unspecified,
}

/// A single text replacement.
#[derive(Debug, Clone, Serialize)]
pub struct Edit {
    /// Where to apply.
    pub span: Span,
    /// The replacement text.
    pub new_text: String,
}

/// Sanitize a string for single-line use: strip ANSI + ALL control characters.
///
/// Used at data entry boundaries to sanitize tool output before it enters
/// the type system. Strips:
/// - ANSI escape sequences (CSI, OSC, two-byte)
/// - ALL control characters including newlines and carriage returns
/// - Only tabs (0x09) preserved
///
/// Diagnostic fields (message, rule, path, notes) are rendered inline.
/// A newline in a message would break the output format. Multi-line raw
/// tool output is handled separately by renderers that split by lines.
/// Sanitize a multi-line string: strip ANSI + control characters but preserve newlines.
///
/// Used for snippet fields which carry source code or diff content.
/// Newlines (`\n`) are structural — stripping them corrupts the content.
pub fn sanitize_multiline(s: &str) -> String {
    sanitize_impl(s, true)
}

pub fn sanitize(s: &str) -> String {
    sanitize_impl(s, false)
}

fn sanitize_impl(s: &str, preserve_newlines: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() || next == '@' || next == '~' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                Some(ch) if ch.is_ascii_alphabetic() || *ch == '=' || *ch == '>' => {
                    chars.next();
                }
                _ => {}
            }
        } else if c.is_control() && c != '\t' && !(preserve_newlines && c == '\n') {
            // Strip control characters. Tabs preserved always.
            // Newlines preserved only for multi-line fields (snippets).
        } else {
            out.push(c);
        }
    }
    out
}

impl Diagnostic {
    /// Sanitize all remaining string fields: strip ANSI escapes AND control characters.
    ///
    /// `message` and `rule` are `DisplayStr` — already sanitized at construction.
    /// This method covers the remaining fields: location paths, raw_severity,
    /// related spans, notes, suggestions, snippet, tool, stack.
    ///
    /// Called once at the data entry boundary (after parsing tool output).
    pub fn sanitize(&mut self) {
        // message and rule: handled by DisplayStr — no action needed.
        if let Some(ref mut raw_sev) = self.raw_severity {
            *raw_sev = sanitize(raw_sev);
        }
        match &mut self.location {
            Location::File(ref mut f) | Location::Absolute(ref mut f) => {
                *f = sanitize(f);
            }
        }
        for related in &mut self.related {
            related.label = sanitize(&related.label);
            match &mut related.location {
                Location::File(ref mut f) | Location::Absolute(ref mut f) => {
                    *f = sanitize(f);
                }
            }
        }
        for note in &mut self.notes {
            *note = sanitize(note);
        }
        for suggestion in &mut self.suggestions {
            suggestion.message = sanitize(&suggestion.message);
            for edit in &mut suggestion.edits {
                edit.new_text = sanitize(&edit.new_text);
            }
        }
        if let Some(ref mut snippet) = self.snippet {
            *snippet = sanitize_multiline(snippet);
        }
        self.tool = sanitize(&self.tool);
        self.stack = sanitize(&self.stack);
    }
}

/// Normalized severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
            Self::Info => f.write_str("info"),
            Self::Hint => f.write_str("hint"),
        }
    }
}

/// Whether diagnostics were successfully parsed from tool output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub enum ParseStatus {
    /// All tool output was parsed into diagnostics.
    Parsed,
    /// Tool output could not be parsed — use `raw_stdout` / `raw_stderr`.
    Unparsed,
    /// No parse was attempted — tool didn't run (spawn failure, timeout, signal).
    None,
}

impl std::fmt::Display for ParseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parsed => f.write_str("parsed"),
            Self::Unparsed => f.write_str("unparsed"),
            Self::None => f.write_str("none"),
        }
    }
}

/// Result of parsing tool output into diagnostics.
#[derive(Debug)]
pub struct ParseResult {
    /// Parsed diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether parsing succeeded.
    pub status: ParseStatus,
    /// Number of structured items successfully parsed from tool output.
    /// For JSON parsers: number of valid JSON objects/elements deserialized.
    /// For text parsers: number of diagnostic patterns matched.
    /// Used by enrich to distinguish "parser understood the format but found
    /// no violations" from "parser couldn't understand the format at all."
    /// When `parsed_items > 0 && diagnostics.is_empty()`, enrich trusts the
    /// parser — the tool genuinely found nothing.
    pub parsed_items: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_clamps_zero_to_one() {
        let s = Span::new(0, 0, None, None);
        assert_eq!(s.line, 1);
        assert_eq!(s.column, 1);
    }

    #[test]
    fn span_preserves_valid_values() {
        let s = Span::new(10, 5, Some(10), Some(20));
        assert_eq!(s.line, 10);
        assert_eq!(s.column, 5);
        assert_eq!(s.end_line, Some(10));
        assert_eq!(s.end_column, Some(20));
    }

    #[test]
    fn span_drops_backwards_end() {
        // end_line < start_line → dropped
        let s = Span::new(10, 5, Some(5), Some(10));
        assert_eq!(s.end_line, None);
        assert_eq!(s.end_column, None);
    }

    #[test]
    fn span_drops_backwards_column_same_line() {
        // same line, end_column < start_column → dropped
        let s = Span::new(10, 20, Some(10), Some(5));
        assert_eq!(s.end_line, None);
        assert_eq!(s.end_column, None);
    }

    #[test]
    fn span_clamps_end_zero_to_one() {
        let s = Span::new(1, 1, Some(0), Some(0));
        assert_eq!(s.end_line, Some(1));
        assert_eq!(s.end_column, Some(1));
    }

    #[test]
    fn span_end_after_start_is_valid() {
        let s = Span::new(5, 10, Some(10), Some(1));
        assert_eq!(s.end_line, Some(10));
        assert_eq!(s.end_column, Some(1));
    }

    // ─── Diagnostic::sanitize tests ────────────────────────────────────

    fn make_diagnostic(message: &str) -> Diagnostic {
        Diagnostic {
            location: Location::File("test.rs".to_string()),
            span: None,
            rule: Some(DisplayStr::from_untrusted("test-rule")),
            severity: Severity::Error,
            raw_severity: None,
            message: DisplayStr::from_untrusted(message),
            related: vec![],
            notes: vec!["note with \nnewline".to_string()],
            suggestions: vec![],
            snippet: Some("snippet\nwith\nnewlines".to_string()),
            tool: "test".to_string(),
            stack: "test".to_string(),
        }
    }

    #[test]
    fn sanitize_strips_newlines_from_message() {
        let mut d = make_diagnostic("line1\nline2\rline3");
        d.sanitize();
        assert_eq!(d.message, "line1line2line3");
    }

    #[test]
    fn sanitize_strips_ansi_from_message() {
        let mut d = make_diagnostic("\x1b[31mred\x1b[0m text");
        d.sanitize();
        assert_eq!(d.message, "red text");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        let mut d = make_diagnostic("a\x01b\x07c\x7fd");
        d.sanitize();
        assert_eq!(d.message, "abcd");
    }

    #[test]
    fn sanitize_preserves_tabs() {
        let mut d = make_diagnostic("a\tb");
        d.sanitize();
        assert_eq!(d.message, "a\tb");
    }

    #[test]
    fn sanitize_strips_from_all_fields() {
        let mut d = make_diagnostic("clean");
        d.rule = Some(DisplayStr::from_untrusted("rule\nwith\nnewlines"));
        d.location = Location::File("path\nwith\nnewline".to_string());
        d.sanitize();
        assert_eq!(d.rule.as_deref(), Some("rulewithnewlines"));
        assert!(matches!(&d.location, Location::File(f) if f == "pathwithnewline"));
        // Notes are single-line — newlines stripped
        assert_eq!(d.notes[0], "note with newline");
        // Snippets are multi-line — newlines preserved, only ANSI/control stripped
        assert_eq!(d.snippet.as_deref(), Some("snippet\nwith\nnewlines"));
    }

    #[test]
    fn sanitize_snippet_strips_ansi_preserves_newlines() {
        let mut d = make_diagnostic("clean");
        d.snippet = Some("\x1b[31mred\x1b[0m line\nsecond line\x01".to_string());
        d.sanitize();
        assert_eq!(d.snippet.as_deref(), Some("red line\nsecond line"));
    }

    // ─── Stress tests for sanitize() and sanitize_multiline() ────────────

    #[test]
    fn stress_all_ansi_csi_sequences() {
        // CSI sequences: ESC [ ... (a-zA-Z or @~)
        let input = "\x1b[0m\x1b[31m\x1b[1;32m\x1b[38;5;208m\x1b[48;2;255;0;0m";
        let result = sanitize(input);
        assert_eq!(result, "");
        // Ensure no panic, output has no control chars
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_all_ansi_osc_sequences() {
        // OSC sequences: ESC ] ... (BEL or ST)
        let input = "\x1b]0;title\x07\x1b]4;1;rgb:ff/00/00\x07";
        let result = sanitize(input);
        assert_eq!(result, "");
        assert!(!result.chars().any(|c| c.is_control() && c != '\t'));
    }

    #[test]
    fn stress_ansi_osc_with_st_terminator() {
        // OSC terminated by ST (ESC \) instead of BEL
        let input = "text\x1b]0;title\x1b\\more";
        let result = sanitize(input);
        assert_eq!(result, "textmore");
        assert!(!result.chars().any(|c| c.is_control() && c != '\t'));
    }

    #[test]
    fn stress_ansi_two_byte_sequences() {
        // Two-byte escape sequences: ESC (=, >, or alpha)
        let input = "\x1b=\x1b>\x1bM\x1bD";
        let result = sanitize(input);
        assert_eq!(result, "");
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_nested_ansi_like_sequences() {
        // Escape-like content inside escape (should not recursively process)
        let input = "\x1b[38;5;208mtext\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "text");
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_truncated_ansi_sequences() {
        // Incomplete CSI sequence — scanner consumes until it finds alphabetic/~/@ terminator
        // In "\x1b[31incomplete", the 'i' is alphabetic, so it terminates the CSI at that point
        let input = "before\x1b[31incomplete";
        let result = sanitize(input);
        assert_eq!(result, "beforencomplete");
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_truncated_ansi_osc_sequence() {
        // Incomplete OSC sequence
        let input = "start\x1b]0;no_terminator";
        let result = sanitize(input);
        // OSC loop consumes until BEL or ST, so "start" remains
        assert_eq!(result, "start");
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_all_control_characters() {
        // All bytes 0x00-0x1F except tab (0x09) and newline (0x0A in sanitize)
        let mut input = String::new();
        for byte in 0u8..=0x1F {
            input.push(byte as char);
        }
        let result = sanitize(&input);
        // Should preserve tab (0x09) only
        assert!(result.contains('\t'));
        // Should remove all others
        for byte in 0u8..=0x1F {
            if byte != 0x09 {
                assert!(!result.contains(byte as char));
            }
        }
    }

    #[test]
    fn stress_multiline_preserves_newlines_strips_others() {
        // All control chars 0x00-0x1F with multiline mode
        let mut input = String::new();
        for byte in 0u8..=0x1F {
            input.push(byte as char);
        }
        let result = sanitize_multiline(&input);
        // Should preserve tab (0x09) and newline (0x0A)
        assert!(result.contains('\t'));
        assert!(result.contains('\n'));
        // Should remove others
        for byte in 0u8..=0x1F {
            if byte != 0x09 && byte != 0x0A {
                assert!(!result.contains(byte as char));
            }
        }
    }

    #[test]
    fn stress_huge_input_1mb() {
        // 1 MB alternating ANSI + text
        let mut input = String::new();
        for _ in 0..10000 {
            input.push_str("\x1b[31m");
            input.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit. ");
            input.push_str("\x1b[0m");
        }
        let result = sanitize(&input);
        // Should not panic, output should be clean text
        assert!(!result.is_empty());
        assert!(!result.contains('\x1b'));
        assert!(!result.chars().any(|c| c.is_control() && c != '\t'));
    }

    #[test]
    fn stress_only_ansi_sequences() {
        // String composed entirely of ANSI sequences
        let input = "\x1b[31m\x1b[1;32m\x1b[0m\x1b[38;5;255m\x1b]0;title\x07";
        let result = sanitize(input);
        assert_eq!(result, "");
    }

    #[test]
    fn stress_unicode_emoji_with_ansi() {
        // Emoji and other Unicode mixed with ANSI
        let input = "\x1b[31m🔴 red\x1b[0m \x1b[34m🔵 blue\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "🔴 red 🔵 blue");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn stress_cjk_characters_with_ansi() {
        // CJK (Chinese, Japanese, Korean) with ANSI
        let input = "\x1b[32m你好\x1b[0m \x1b[33mこんにちは\x1b[0m 한글";
        let result = sanitize(input);
        assert_eq!(result, "你好 こんにちは 한글");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn stress_null_bytes_various_positions() {
        // Null bytes at start, middle, end
        let input = "\x00start\x00middle\x00end\x00";
        let result = sanitize(input);
        assert_eq!(result, "startmiddleend");
        assert!(!result.contains('\x00'));
        assert!(!result.chars().any(|c| c.is_control()));
    }

    #[test]
    fn stress_null_bytes_with_ansi() {
        // Null bytes mixed with ANSI
        let input = "\x1b[31m\x00red\x1b[0m\x00text";
        let result = sanitize(input);
        assert_eq!(result, "redtext");
    }

    #[test]
    fn stress_multiline_ansi_each_line() {
        // ANSI on each line, newlines should be preserved
        let input = "\x1b[31mline1\x1b[0m\n\x1b[32mline2\x1b[0m\n\x1b[33mline3\x1b[0m";
        let result = sanitize_multiline(input);
        assert_eq!(result, "line1\nline2\nline3");
        // Verify newlines still there
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");
    }

    #[test]
    fn stress_multiline_control_chars_each_line() {
        // Control chars on each line
        let input = "line1\x01\nline2\x07\nline3\x1f";
        let result = sanitize_multiline(input);
        assert_eq!(result, "line1\nline2\nline3");
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn stress_empty_string() {
        let result = sanitize("");
        assert_eq!(result, "");
    }

    #[test]
    fn stress_empty_string_multiline() {
        let result = sanitize_multiline("");
        assert_eq!(result, "");
    }

    #[test]
    fn stress_only_whitespace_with_tabs() {
        let input = "  \t  \t  ";
        let result = sanitize(input);
        assert_eq!(result, "  \t  \t  ");
    }

    #[test]
    fn stress_only_newlines_multiline() {
        let input = "\n\n\n";
        let result = sanitize_multiline(input);
        assert_eq!(result, "\n\n\n");
    }

    #[test]
    fn stress_only_newlines_singleline() {
        let input = "\n\n\n";
        let result = sanitize(input);
        assert_eq!(result, "");
    }

    #[test]
    fn stress_complex_ansi_256_color() {
        // 256-color palette sequences
        let input = "\x1b[38;5;196mRed\x1b[0m \x1b[38;5;46mGreen\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "Red Green");
    }

    #[test]
    fn stress_complex_ansi_truecolor() {
        // 24-bit RGB color sequences
        let input = "\x1b[38;2;255;0;0mRGB Red\x1b[0m \x1b[38;2;0;255;0mRGB Green\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "RGB Red RGB Green");
    }

    #[test]
    fn stress_ansi_with_parameters() {
        // ANSI with numeric parameters and semicolons
        let input = "\x1b[1;2;4;31;47mBold Underline on White\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "Bold Underline on White");
    }

    #[test]
    fn stress_interleaved_escapes_and_text() {
        // Many short escape-text-escape sequences
        let input = "\x1b[31mA\x1b[0m\x1b[32mB\x1b[0m\x1b[33mC\x1b[0m\x1b[34mD\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "ABCD");
    }

    #[test]
    fn stress_del_character() {
        // DEL (0x7F) is also a control character
        let input = "before\x7fafter";
        let result = sanitize(input);
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn stress_del_with_ansi() {
        let input = "\x1b[31m\x7ftext\x7f\x1b[0m";
        let result = sanitize(input);
        assert_eq!(result, "text");
    }

    #[test]
    fn stress_carriage_return_and_newline() {
        // \r\n sequences
        let input = "line1\r\nline2";
        let result = sanitize(input);
        assert_eq!(result, "line1line2");
    }

    #[test]
    fn stress_multiline_carriage_return_preserved() {
        // In multiline, \r should be stripped but \n preserved
        let input = "line1\r\nline2\rline3\nline4";
        let result = sanitize_multiline(input);
        assert_eq!(result, "line1\nline2line3\nline4");
    }

    #[test]
    fn stress_form_feed_and_vertical_tab() {
        // More exotic control chars
        let input = "line1\x0cline2\x0bline3";
        let result = sanitize(input);
        assert_eq!(result, "line1line2line3");
    }

    #[test]
    fn stress_bell_outside_osc() {
        // BEL character that's not part of OSC — BEL itself is a control char (0x07)
        let input = "text\x07alarm\x07more";
        let result = sanitize(input);
        assert_eq!(result, "textalarmmore");
        assert!(!result.contains('\x07'));
    }

    #[test]
    fn stress_very_long_single_escape() {
        // Very long CSI sequence (e.g., many parameters)
        let mut input = String::from("\x1b[");
        for i in 0..100 {
            if i > 0 {
                input.push(';');
            }
            input.push_str(&i.to_string());
        }
        input.push('m');
        input.push_str("text");
        let result = sanitize(&input);
        assert_eq!(result, "text");
    }

    #[test]
    fn stress_utf8_continuation_bytes() {
        // Multi-byte UTF-8 sequences (should be preserved, not treated as control)
        let input = "Hello 世界 🌍";
        let result = sanitize(input);
        assert_eq!(result, "Hello 世界 🌍");
    }
}
