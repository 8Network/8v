//! Test helpers for constructing `Diagnostic` values.
//!
//! `Diagnostic` has many required fields. Tests that construct one directly
//! must fill every field — and break whenever a new field is added. This
//! module centralises that construction so only `DiagnosticBuilder::build()`
//! needs updating when the struct evolves.
//!
//! ## Usage
//!
//! ```rust
//! use o8v_core::DiagnosticBuilder;
//!
//! let d = DiagnosticBuilder::new("src/main.rs", "unused function `foo`")
//!     .rule("dead-code")
//!     .at_line(10)
//!     .build();
//! ```

/// Builder for test `Diagnostic` values.
///
/// Only two fields are mandatory at construction: the source file and the
/// message. Everything else has a safe default. Call `build()` to produce
/// the `Diagnostic`.
pub struct DiagnosticBuilder {
    file: String,
    rule: Option<String>,
    message: String,
    line: Option<u32>,
    severity: crate::Severity,
}

impl DiagnosticBuilder {
    /// Start building a diagnostic at `file` with the given `message`.
    pub fn new(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            rule: None,
            message: message.into(),
            line: None,
            severity: crate::Severity::Error,
        }
    }

    /// Set the rule identifier (e.g. `"dead-code"`, `"clippy::unused"`).
    pub fn rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }

    /// Set the primary line number (1-based).
    pub fn at_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the severity. Defaults to `Error`.
    pub fn severity(mut self, severity: crate::Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Consume the builder and produce a `Diagnostic`.
    ///
    /// All optional fields not set via the builder methods are set to their
    /// zero/empty values. When `Diagnostic` gains new fields, update this
    /// method — callers do not need to change.
    pub fn build(self) -> crate::Diagnostic {
        crate::Diagnostic {
            location: crate::Location::File(self.file),
            span: self.line.map(|l| crate::Span::new(l, 1, None, None)),
            rule: self.rule.map(crate::DisplayStr::from_untrusted),
            severity: self.severity,
            raw_severity: None,
            message: crate::DisplayStr::from_untrusted(self.message),
            related: vec![],
            notes: vec![],
            suggestions: vec![],
            snippet: None,
            tool: "test-tool".to_string(),
            stack: "test-stack".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_minimal() {
        let d = DiagnosticBuilder::new("src/lib.rs", "some error").build();
        assert!(matches!(d.location, crate::Location::File(ref p) if p == "src/lib.rs"));
        assert_eq!(d.message.as_str(), "some error");
        assert!(d.rule.is_none());
        assert!(d.span.is_none());
        assert_eq!(d.severity, crate::Severity::Error);
    }

    #[test]
    fn test_builder_with_rule_and_line() {
        let d = DiagnosticBuilder::new("src/main.rs", "unused var")
            .rule("dead-code")
            .at_line(42)
            .build();
        assert_eq!(d.rule.as_ref().map(|r| r.as_str()), Some("dead-code"));
        assert_eq!(d.span.as_ref().map(|s| s.line), Some(42));
    }

    #[test]
    fn test_builder_severity_override() {
        let d = DiagnosticBuilder::new("src/main.rs", "info msg")
            .severity(crate::Severity::Warning)
            .build();
        assert_eq!(d.severity, crate::Severity::Warning);
    }

    #[test]
    fn test_builder_sanitizes_message() {
        // from_untrusted strips ANSI — the builder must not bypass sanitization.
        let d = DiagnosticBuilder::new("src/main.rs", "msg \x1b[31mred\x1b[0m").build();
        assert!(
            !d.message.as_str().contains('\x1b'),
            "ANSI must be stripped"
        );
    }
}
