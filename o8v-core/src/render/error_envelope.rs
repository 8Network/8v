// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Canonical JSON error envelope for all 8v commands under `--json`.
//!
//! Contract (§2.4 + §7 of docs/design/error-contract.md):
//! - Shape: `{"error":"<human-readable message>","code":"<machine key>"}`
//! - Optional fields: `"path":"<affected path>"`, `"line":<number>`
//! - Emitted to stdout (stderr empty), exit 1 on failure with `--json`.
//! - Approved codes: `not_found`, `permission_denied`, `outside_project`,
//!   `invalid_range`, `invalid_regex`, `content_empty`, `no_match`,
//!   `invocation_error`, `network`, `runtime` (fallback).

/// Returns the canonical JSON error envelope with the two required fields.
///
/// Output has a trailing newline, ready to write directly to stdout.
pub fn json_error_envelope(message: &str, code: &str) -> String {
    let mut s = serde_json::json!({
        "error": message,
        "code": code,
    })
    .to_string();
    s.push('\n');
    s
}

/// Returns the canonical JSON error envelope with an optional `"path"` field.
///
/// Output has a trailing newline.
pub fn json_error_envelope_with_path(message: &str, code: &str, path: &str) -> String {
    let mut s = serde_json::json!({
        "error": message,
        "code": code,
        "path": path,
    })
    .to_string();
    s.push('\n');
    s
}

/// Returns the canonical JSON error envelope with optional `"path"` and `"line"` fields.
///
/// Output has a trailing newline.
pub fn json_error_envelope_with_line(message: &str, code: &str, path: &str, line: usize) -> String {
    let mut s = serde_json::json!({
        "error": message,
        "code": code,
        "path": path,
        "line": line,
    })
    .to_string();
    s.push('\n');
    s
}

/// Classify an error message string into a machine-readable code.
///
/// Uses substring matching on the error message text. Falls back to `"runtime"`
/// for unrecognized errors (fallback for unrecognized error messages).
pub fn classify_error_code(msg: &str) -> &'static str {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("does not exist")
    {
        "not_found"
    } else if lower.contains("permission denied") || lower.contains("access denied") {
        "permission_denied"
    } else if lower.contains("outside") || lower.contains("outside_project") {
        "outside_project"
    } else if lower.contains("invalid range") || lower.contains("range") && lower.contains("out") {
        "invalid_range"
    } else if lower.contains("invalid regex")
        || lower.contains("regex")
        || lower.contains("regular expression")
    {
        "invalid_regex"
    } else if lower.contains("empty") && lower.contains("content") {
        "content_empty"
    } else if lower.contains("no match") || lower.contains("no_match") {
        "no_match"
    } else if lower.contains("network")
        || lower.contains("connection")
        || lower.contains("timeout")
        || lower.contains("unreachable")
    {
        "network"
    } else if lower.contains("no build step")
        || lower.contains("unsupported")
        || lower.contains("not supported")
    {
        "unsupported"
    } else {
        "runtime"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_has_error_and_code() {
        let out = json_error_envelope("file not found", "not_found");
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
        assert_eq!(v["error"].as_str(), Some("file not found"));
        assert_eq!(v["code"].as_str(), Some("not_found"));
    }

    #[test]
    fn envelope_ends_with_newline() {
        let out = json_error_envelope("msg", "runtime");
        assert!(out.ends_with('\n'), "must end with newline");
    }

    #[test]
    fn envelope_no_extra_fields() {
        let out = json_error_envelope("msg", "runtime");
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
        assert!(v.get("error_kind").is_none(), "no error_kind field");
        assert!(v.get("path").is_none(), "no path field without with_path");
        assert!(v.get("line").is_none(), "no line field without with_line");
    }

    #[test]
    fn envelope_with_path_includes_path() {
        let out = json_error_envelope_with_path("msg", "not_found", "/some/path.rs");
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
        assert_eq!(v["path"].as_str(), Some("/some/path.rs"));
    }

    #[test]
    fn envelope_with_line_includes_path_and_line() {
        let out = json_error_envelope_with_line("msg", "invalid_range", "/file.rs", 42);
        let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON");
        assert_eq!(v["path"].as_str(), Some("/file.rs"));
        assert_eq!(v["line"].as_u64(), Some(42));
    }

    #[test]
    fn classify_not_found() {
        assert_eq!(
            classify_error_code("no such file or directory"),
            "not_found"
        );
        assert_eq!(classify_error_code("file not found"), "not_found");
        assert_eq!(classify_error_code("does not exist"), "not_found");
    }

    #[test]
    fn classify_permission_denied() {
        assert_eq!(
            classify_error_code("permission denied"),
            "permission_denied"
        );
        assert_eq!(classify_error_code("Access denied"), "permission_denied");
    }

    #[test]
    fn classify_invalid_regex() {
        assert_eq!(
            classify_error_code("invalid regex: unclosed group"),
            "invalid_regex"
        );
        assert_eq!(
            classify_error_code("error in regular expression"),
            "invalid_regex"
        );
    }

    #[test]
    fn classify_network() {
        assert_eq!(classify_error_code("connection refused"), "network");
        assert_eq!(classify_error_code("network error"), "network");
        assert_eq!(classify_error_code("timeout"), "network");
    }

    #[test]
    fn classify_runtime_fallback() {
        assert_eq!(classify_error_code("some unknown error"), "runtime");
    }
}
