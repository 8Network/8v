// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `DisplayStr` — a string that has been sanitized for terminal display.
//!
//! Follows the `ContainmentRoot` pattern: the invariant is enforced at
//! construction, not at use. Once a string is inside `DisplayStr`, it has
//! been stripped of ANSI escape sequences and control characters. The type
//! signature carries the proof.
//!
//! ## Boundaries
//!
//! `DisplayStr` is the type for `Diagnostic.message` and `Diagnostic.rule`.
//! At those fields, any unsanitized `String` is a compile error. Parsers
//! wrap tool output: `DisplayStr::from_untrusted(raw)`. Internal/synthetic
//! strings use `DisplayStr::from_trusted(s)`.
//!
//! ## Limitations
//!
//! `DisplayStr` protects the **structured rendering pipeline** (human terminal,
//! plain text, JSON renderers). It does NOT protect:
//!
//! - `tracing::` log output — loggers display to terminals; `tracing` fields
//!   take `&str`. Sanitize explicitly at log call sites if needed.
//! - MCP response strings — ANSI is valid JSON; the client is responsible.
//! - Ad-hoc `eprintln!` / `write!(stderr)` calls outside the renderer path.
//!
//! The invariant is enforced at the API level, not the language level. A
//! developer can bypass via `from_trusted(raw)` — this is explicit and
//! grepable, unlike a forgotten `sanitize()` call.

use serde::{Deserialize, Serialize};

/// A string guaranteed to be free of ANSI escape sequences and control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DisplayStr(String);

impl DisplayStr {
    /// Construct from untrusted input — strips ANSI, control characters, newlines.
    ///
    /// Use for all strings from external sources: tool output, file names,
    /// error messages from subprocesses.
    #[must_use]
    pub fn from_untrusted(s: impl Into<String>) -> Self {
        Self(crate::diagnostic::sanitize(&s.into()))
    }

    /// Construct without sanitization — bypass for strings you own and know are clean.
    ///
    /// Use ONLY for:
    /// - Static string literals
    /// - Strings produced by internal formatting (numeric values, enum Display)
    /// - Strings that were already sanitized before reaching this call
    ///
    /// This is a deliberate bypass — callers must only use it for strings
    /// they constructed or already sanitized. Grep for `from_trusted` to audit.
    #[must_use]
    pub fn from_trusted(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Return the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::ops::Deref for DisplayStr {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DisplayStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DisplayStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// Convenience comparisons so `assert_eq!(d.message, "expected")` works.
impl PartialEq<str> for DisplayStr {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}
impl PartialEq<DisplayStr> for str {
    fn eq(&self, other: &DisplayStr) -> bool {
        self == other.0
    }
}
impl PartialEq<&str> for DisplayStr {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
impl PartialEq<DisplayStr> for &str {
    fn eq(&self, other: &DisplayStr) -> bool {
        *self == other.0
    }
}
impl PartialEq<String> for DisplayStr {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}
impl PartialEq<DisplayStr> for String {
    fn eq(&self, other: &DisplayStr) -> bool {
        self == &other.0
    }
}

// Deserialize through sanitization — data read from disk/network is untrusted.
impl<'de> Deserialize<'de> for DisplayStr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_untrusted(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_untrusted_strips_ansi() {
        let s = DisplayStr::from_untrusted("\x1b[31mred\x1b[0m");
        assert_eq!(s.as_str(), "red");
    }

    #[test]
    fn from_untrusted_strips_newlines() {
        let s = DisplayStr::from_untrusted("line1\nline2");
        assert_eq!(s.as_str(), "line1line2");
    }

    #[test]
    fn from_untrusted_clean_unchanged() {
        let s = DisplayStr::from_untrusted("hello world");
        assert_eq!(s, "hello world");
    }

    #[test]
    fn from_trusted_bypasses_sanitization() {
        let raw = "raw\x1b[31m";
        let s = DisplayStr::from_trusted(raw);
        assert_eq!(s.as_str(), raw);
    }

    #[test]
    fn deref_to_str() {
        let s = DisplayStr::from_untrusted("hello");
        let r: &str = &s;
        assert_eq!(r, "hello");
    }

    #[test]
    fn display_impl() {
        let s = DisplayStr::from_untrusted("hello");
        assert_eq!(format!("{s}"), "hello");
    }

    #[test]
    fn partial_eq_str() {
        let s = DisplayStr::from_untrusted("hello");
        assert_eq!(s, "hello");
        assert_eq!("hello", s);
    }

    #[test]
    fn partial_eq_string() {
        let s = DisplayStr::from_untrusted("hello");
        assert_eq!(s, String::from("hello"));
    }

    #[test]
    fn deserialize_sanitizes() {
        let json = r#""hello\u001b[31mworld""#;
        let s: DisplayStr = serde_json::from_str(json).unwrap();
        assert_eq!(s, "helloworld");
    }

    #[test]
    fn serialize_as_plain_string() {
        let s = DisplayStr::from_untrusted("hello");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#""hello""#);
    }
}
