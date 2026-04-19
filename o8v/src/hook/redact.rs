// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Redaction pipeline for bash commands captured by the hook layer.
//!
//! Applies three pattern-based redactions before a command string is stored
//! in any event. All patterns are compiled once at first call via `OnceLock`.

use regex::Regex;
use std::sync::OnceLock;

/// Compiled redaction patterns. Panics at first call if any pattern is invalid
/// (invalid patterns are programming errors, not runtime errors).
fn patterns() -> &'static [Regex; 3] {
    static PATTERNS: OnceLock<[Regex; 3]> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // API keys — e.g. OpenAI sk-... tokens.
            Regex::new(r"sk-[A-Za-z0-9]{20,}").expect("api key pattern is valid"),
            // JWTs — three base64url segments separated by dots.
            Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+")
                .expect("jwt pattern is valid"),
            // URL credentials — user:pass@ in any scheme.
            Regex::new(r"://[^:@/ ]+:[^@/ ]+@").expect("url credentials pattern is valid"),
        ]
    })
}

/// Redact secrets from a bash command string before storage or logging.
///
/// Applies three rules in order:
/// 1. API keys matching `sk-<20+ alphanumeric>` → `<secret>`
/// 2. JWTs (three base64url segments) → `<secret>`
/// 3. URL credentials (`://user:pass@`) → `://<secret>@`
///
/// The original string is not modified if no pattern matches.
pub fn redact_bash_command(s: &str) -> String {
    let [api_key, jwt, url_creds] = patterns();
    let s = api_key.replace_all(s, "<secret>");
    let s = jwt.replace_all(&s, "<secret>");
    let s = url_creds.replace_all(&s, "://<secret>@");
    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- API key ---

    #[test]
    fn redact_api_key_full_token() {
        let input =
            "curl -H 'Authorization: Bearer sk-abcdefghij1234567890' https://api.example.com";
        let out = redact_bash_command(input);
        assert!(
            !out.contains("sk-abcdefghij"),
            "key must be redacted: {out}"
        );
        assert!(out.contains("<secret>"), "placeholder must appear: {out}");
    }

    #[test]
    fn redact_api_key_env_assignment() {
        let input = "OPENAI_API_KEY=sk-ABCDEFGHIJKLMNOPQRST12345 python script.py";
        let out = redact_bash_command(input);
        assert!(!out.contains("sk-ABCD"), "key must be redacted: {out}");
        assert!(out.contains("<secret>"), "placeholder must appear: {out}");
    }

    #[test]
    fn redact_api_key_short_prefix_not_redacted() {
        // Fewer than 20 alphanumeric chars after sk- — must NOT be redacted.
        let input = "echo sk-short";
        let out = redact_bash_command(input);
        assert_eq!(out, input, "short token must not be redacted");
    }

    // --- JWT ---

    #[test]
    fn redact_jwt_typical_token() {
        let input = "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c'";
        let out = redact_bash_command(input);
        assert!(!out.contains("eyJhbGci"), "jwt must be redacted: {out}");
        assert!(out.contains("<secret>"), "placeholder must appear: {out}");
    }

    #[test]
    fn redact_jwt_env_assignment() {
        let input = "TOKEN=eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJleGFtcGxlIn0.abc123_xyz ./run.sh";
        let out = redact_bash_command(input);
        assert!(!out.contains("eyJhbGci"), "jwt must be redacted: {out}");
        assert!(out.contains("<secret>"), "placeholder must appear: {out}");
    }

    #[test]
    fn redact_jwt_incomplete_token_not_redacted() {
        // Only one segment — does not match three-segment JWT pattern.
        let input = "echo eyJhbGciOiJub25lIn0";
        let out = redact_bash_command(input);
        assert_eq!(out, input, "incomplete jwt must not be redacted");
    }

    // --- URL credentials ---

    #[test]
    fn redact_url_credentials_postgres_dsn() {
        let input = "psql postgresql://admin:s3cr3tP@ss@db.example.com/mydb";
        let out = redact_bash_command(input);
        assert!(!out.contains("s3cr3tP"), "password must be redacted: {out}");
        assert!(
            out.contains("://<secret>@"),
            "placeholder must appear: {out}"
        );
    }

    #[test]
    fn redact_url_credentials_http_basic_auth() {
        let input = "curl https://user:hunter2@example.com/api";
        let out = redact_bash_command(input);
        assert!(!out.contains("hunter2"), "password must be redacted: {out}");
        assert!(
            out.contains("://<secret>@"),
            "placeholder must appear: {out}"
        );
    }

    #[test]
    fn redact_url_credentials_no_auth_not_redacted() {
        // URL without credentials — must pass through unchanged.
        let input = "curl https://example.com/api";
        let out = redact_bash_command(input);
        assert_eq!(out, input, "url without credentials must not be redacted");
    }
}
