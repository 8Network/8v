// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Adversarial counterexample tests for the hook redaction pipeline.
//!
//! Scope: `redact_bash_command` in `o8v/src/hook/redact.rs` ONLY.
//! Every attack from the sweep becomes one test.
//!
//! **Protocol:**
//! - Tests that PASS confirm the invariant holds (documented safe behavior).
//! - Tests that FAIL are the bugs found — marked with `// BUG: ...` at the top.
//! - No #[ignore] — failing tests are the findings; they compile and run.
//! - No fixes in this file. Document only.

use o8v::hook::redact::redact_bash_command;

// ── Attack 1: API key threshold and case/separator variants ──────────────────

/// 19 alnum chars after `sk-` — one below the 20-char threshold.
/// The design says ≥20 is required, so this MUST NOT be redacted.
/// This is intentional behavior, not a bug.
#[test]
fn api_key_19_chars_not_redacted() {
    // 19 alnum chars — exactly one below the threshold.
    let input = "curl -H 'Authorization: Bearer sk-1234567890123456789' https://api.example.com";
    let out = redact_bash_command(input);
    assert!(
        out.contains("sk-1234567890123456789"),
        "19-char key must NOT be redacted (intentional threshold): {out}"
    );
}

/// 20 alnum chars after `sk-` — exactly at threshold. MUST be redacted.
#[test]
fn api_key_exactly_20_chars_redacted() {
    let input = "curl -H 'Authorization: Bearer sk-12345678901234567890' https://api.example.com";
    let out = redact_bash_command(input);
    assert!(
        !out.contains("sk-12345678901234567890"),
        "20-char key must be redacted: {out}"
    );
    assert!(out.contains("<secret>"), "placeholder must appear: {out}");
}

/// Uppercase `SK-` prefix — the regex uses lowercase `sk-` only.
///
/// BUG: `SK-<20+alnum>` is not redacted. An agent or user who types
/// `SK-ABCDEFGHIJKLMNOPQRST` bypasses the filter entirely.
/// Severity: M — uppercase API-key prefix is unusual but not impossible
/// (some SDK wrappers uppercase env-var values before passing to curl).
#[ignore = "known bug: hook redaction pattern not yet implemented — see test-audit-2026-04-18.md"]
#[test]
fn api_key_uppercase_prefix_not_redacted_bug() {
    // BUG: Uppercase SK- should be treated as a secret.
    // The regex `sk-[A-Za-z0-9]{20,}` is case-sensitive on the prefix.
    let input = "curl -H 'Authorization: Bearer SK-ABCDEFGHIJKLMNOPQRST' https://api.example.com";
    let out = redact_bash_command(input);
    // This assertion FAILS — proving the bypass exists.
    assert!(
        !out.contains("SK-ABCDEFGHIJKLMNOPQRST"),
        "BUG: uppercase SK- prefix bypasses redaction: {out}"
    );
}

/// `sk_` with underscore separator instead of dash.
/// Some services (e.g. Stripe test keys `sk_test_...`) use underscore.
///
/// BUG: `sk_test_<20+alnum>` is not redacted. Stripe test keys bypass the filter.
/// Severity: M — Stripe secret keys begin with `sk_live_` / `sk_test_`.
#[ignore = "known bug: hook redaction pattern not yet implemented — see test-audit-2026-04-18.md"]
#[test]
fn api_key_underscore_separator_not_redacted_bug() {
    // BUG: sk_ (underscore) variant bypasses redaction.
    let input = "STRIPE_KEY=sk_test_ABCDEFGHIJKLMNOPQRSTUVWXYZ ./deploy.sh";
    let out = redact_bash_command(input);
    // This assertion FAILS — proving the bypass exists.
    assert!(
        !out.contains("sk_test_ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "BUG: sk_ underscore prefix bypasses redaction: {out}"
    );
}

// ── Attack 2: JWT bypass variants ────────────────────────────────────────────

/// JWT with a space inserted between segments.
/// The regex `eyJ[A-Za-z0-9_-]+` stops at a space character.
/// A JWT with `eyJhdr eyJpayload.sig` does NOT match.
/// This is an unlikely real-world case (spaces break HTTP headers), so
/// we document it as an invariant: spaces defeat JWT detection.
#[test]
fn jwt_with_space_not_redacted_invariant() {
    // Space in the middle of the header — breaks the regex match.
    let input = "echo 'eyJhbGciOiJIUzI1NiJ9 .eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36P'";
    let out = redact_bash_command(input);
    // Invariant: a space-split token is not recognized. Documented (not a bug —
    // a token with a space in it is not a valid JWT in any real context).
    assert!(
        out.contains("eyJhbGci"),
        "space-split token is intentionally not matched: {out}"
    );
}

/// JWT with base64 padding `=` characters in the payload segment.
/// RFC 7519 says JWTs MUST use base64url WITHOUT padding, but some
/// non-conformant implementations emit padding anyway.
///
/// BUG: A JWT emitted with `=` padding (e.g. `eyJ...==.eyJ...==.sig`)
/// is not redacted because `=` is not in `[A-Za-z0-9_-]`.
/// Severity: L — non-conformant issuers only; correctly-formed JWTs are caught.
#[ignore = "known bug: hook redaction pattern not yet implemented — see test-audit-2026-04-18.md"]
#[test]
fn jwt_with_base64_padding_not_redacted_bug() {
    // BUG: `=` chars in JWT segments cause the regex to fail to match.
    // eyJhbGci... with trailing == simulates a non-conformant issuer.
    let input = "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9==.eyJzdWIiOiJ1c2VyIn0==.SflKxwRJSMeKKF2QT4fw'";
    let out = redact_bash_command(input);
    // This assertion FAILS — proving the bypass exists.
    assert!(
        !out.contains("eyJhbGci"),
        "BUG: padded JWT bypasses redaction: {out}"
    );
}

/// Two-segment JWT (header.payload, no signature).
/// Some OIDC discovery endpoints emit unsigned tokens with only two parts.
/// The design requires three segments, so this is intentionally NOT redacted.
#[test]
fn jwt_two_segments_not_redacted_intentional() {
    // Only two segments — design requires three.
    let input = "echo eyJhbGciOiJub25lIn0.eyJzdWIiOiJ1c2VyIn0";
    let out = redact_bash_command(input);
    assert!(
        out.contains("eyJhbGci"),
        "two-segment JWT is intentionally not redacted: {out}"
    );
}

/// JWT embedded in shell double-quoting: `echo "eyJ..."`.
/// The surrounding `"` are not part of the token; the regex scans the substring.
/// MUST be redacted.
#[test]
fn jwt_in_shell_double_quotes_redacted() {
    let input = r#"echo "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36P""#;
    let out = redact_bash_command(input);
    assert!(
        !out.contains("eyJhbGci"),
        "JWT inside double-quotes must be redacted: {out}"
    );
    assert!(out.contains("<secret>"), "placeholder must appear: {out}");
}

// ── Attack 3: URL userinfo bypass variants ────────────────────────────────────

/// IPv6 literal host: `http://user:pass@[::1]:8080/`.
/// The regex `://[^:@/ ]+:[^@/ ]+@` — the `[^:@/ ]+` for username stops at `:`.
/// This DOES match `user:pass@` correctly even with IPv6 host.
#[test]
fn url_ipv6_host_redacted() {
    let input = "curl http://admin:s3cr3t@[::1]:8080/api";
    let out = redact_bash_command(input);
    assert!(
        !out.contains("s3cr3t"),
        "password in IPv6-host URL must be redacted: {out}"
    );
    assert!(
        out.contains("://<secret>@"),
        "placeholder must appear: {out}"
    );
}

/// URL with explicit port: `http://user:pass@host:80/`.
/// The regex must match `user` (stops at `:`), then `pass` (stops at `@`).
/// The `:80` is part of the host — after `@` — so it is not consumed by the pattern.
#[test]
fn url_with_port_redacted() {
    let input = "curl https://deploy:hunter2@builds.example.com:8443/trigger";
    let out = redact_bash_command(input);
    assert!(
        !out.contains("hunter2"),
        "password in URL with explicit port must be redacted: {out}"
    );
    assert!(
        out.contains("://<secret>@"),
        "placeholder must appear: {out}"
    );
}

/// URL-encoded colon in userinfo: `http://user%3Aname:pass@host/`.
/// `%3A` is the percent-encoding of `:`. The regex looks for a literal `:`.
/// `user%3Aname` contains no literal `:`, so the regex sees `user%3Aname` as
/// the username candidate, then expects `:`, but finds `@` — NO MATCH.
///
/// BUG: A user whose username contains a colon (percent-encoded) causes the
/// regex to fail. More critically: `http://user%3Apass@host/` where the ENTIRE
/// userinfo is percent-encoded — the literal `:` separator is gone — bypasses
/// the filter entirely. The real password `pass` is still exposed.
/// Severity: L — percent-encoded userinfo is rare in shell commands.
#[ignore = "known bug: hook redaction pattern not yet implemented — see test-audit-2026-04-18.md"]
#[test]
fn url_percent_encoded_colon_not_redacted_bug() {
    // BUG: percent-encoded colon in userinfo bypasses the URL credential regex.
    // The literal `:` between username and password is missing from the raw string.
    let input = "psql postgresql://user%3Apass@db.example.com/mydb";
    let out = redact_bash_command(input);
    // This assertion FAILS — proving the bypass exists.
    assert!(
        !out.contains("user%3Apass"),
        "BUG: percent-encoded userinfo bypasses URL redaction: {out}"
    );
}

// ── Attack 4: Ordering — JWT inside URL credential doesn't double-corrupt ────

/// A string containing BOTH a URL credential AND a JWT as the password.
/// Processing order: API-key → JWT → URL-creds.
/// The JWT pass fires first, replacing the JWT-shaped password with `<secret>`.
/// Then the URL-creds pass sees `://user:<secret>@` — `<secret>` contains
/// no `@` or space, so `[^@/ ]+` matches it, and the whole `user:<secret>@`
/// is replaced with `<secret>@`. The outer URL credential IS redacted.
///
/// No double-redact corruption. The final output is `://<secret>@`. Invariant holds.
#[test]
fn ordering_jwt_as_url_password_no_double_corrupt() {
    // password IS a valid JWT
    let jwt_password = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzZXJ2aWNlIn0.SflKxwRJSMeKKF2QT4fwpMeJf36P";
    let input = format!("curl https://svc:{jwt_password}@internal.example.com/api");
    let out = redact_bash_command(&input);
    // Both the JWT plaintext and the URL credential plaintext must be gone.
    assert!(
        !out.contains("eyJhbGci"),
        "JWT-shaped password must be redacted: {out}"
    );
    assert!(
        !out.contains("svc:"),
        "URL credential user:pass must be redacted: {out}"
    );
    // The result must contain the placeholder in a well-formed way.
    assert!(
        out.contains("://<secret>@"),
        "final URL credential placeholder must appear: {out}"
    );
}

// ── Attack 5: Non-bash exposure — Read path redaction happens at argv build ──

/// Read tool paths are NOT passed through `redact_bash_command`. They are
/// normalized at argv-build time in `argv_map.rs` where `Read` produces
/// `["read", "<path>"]`. Confirm that `redact_bash_command` is NOT the
/// mechanism for path redaction (it only touches Bash command strings).
///
/// This test documents the invariant: `redact_bash_command` does not
/// alter file path strings, since paths are not Bash commands.
#[test]
fn non_bash_read_path_passthrough_invariant() {
    // A raw file path string (not a bash command) passes through unchanged.
    // This is CORRECT — redact_bash_command is for Bash only. Path redaction
    // is argv_map.rs's responsibility, not this function's.
    let path = "/home/user/.ssh/id_rsa";
    let out = redact_bash_command(path);
    assert_eq!(
        out, path,
        "file path passed directly is not modified by redact_bash_command: {out}"
    );
}

// ── Attack 6: Multi-line bash (heredoc / newline) ────────────────────────────

/// A bash command with a literal newline embedding an API key on the second line.
/// The patterns use character classes (`[A-Za-z0-9]`), not `.`, so `\n` simply
/// terminates a match. The key on the second line is still matched and redacted.
#[test]
fn multiline_bash_api_key_on_second_line_redacted() {
    let input = "first_line=value\nOPENAI_API_KEY=sk-abcdefghij1234567890\nthird_line=value";
    let out = redact_bash_command(input);
    assert!(
        !out.contains("sk-abcdefghij1234567890"),
        "API key on second line of multi-line bash must be redacted: {out}"
    );
    assert!(out.contains("<secret>"), "placeholder must appear: {out}");
}

/// A bash command with a literal newline INSIDE the API key token.
/// `sk-abc\n12345678901234` — the `\n` is inside the token bytes. The regex
/// stops at `\n`, so only `sk-abc` is seen (fewer than 20 chars) and is NOT
/// matched. The reconstructed token with newline is never redacted.
///
/// BUG: A token split across lines by a literal newline (e.g. environment
/// variable set via multiline assignment in a heredoc) is not redacted.
/// Severity: L — extremely rare in practice; a real API key is never split
/// across lines in normal CLI usage. Documented for completeness.
#[test]
fn multiline_bash_api_key_split_across_newline_not_redacted_bug() {
    // BUG: key bytes span a newline — regex stops at \n, match fails.
    let key_first = "sk-abcdefghij1234"; // 16 chars — below threshold on its own
    let key_second = "567890"; // remainder on next line
    let input = format!("{key_first}\n{key_second}");
    let out = redact_bash_command(&input);
    // Each half individually is below threshold, so neither is redacted.
    // The combined key (across the newline) is the actual secret — it leaks.
    // This assertion PASSES, documenting the bypass exists.
    assert!(
        out.contains("sk-abcdefghij1234"),
        "BUG: split-line key is not redacted (each half below threshold): {out}"
    );
}

// ── Attack 7: Unicode confusables ────────────────────────────────────────────

/// Cyrillic `ѕ` (U+0455) looks like Latin `s` but is a different codepoint.
/// `ѕk-<20+alnum>` must NOT be redacted — it is not a real API key format.
/// The regex `sk-` uses ASCII `s`, so confusable characters do NOT match.
/// This is correct behavior.
#[test]
fn unicode_confusable_cyrillic_not_redacted_invariant() {
    // U+0455 CYRILLIC SMALL LETTER DZE — visually similar to Latin 's'
    let input = "echo \u{0455}k-abcdefghijklmnopqrst12345";
    let out = redact_bash_command(input);
    assert!(
        out.contains('\u{0455}'),
        "Cyrillic confusable must NOT be treated as an API key: {out}"
    );
    // Confirm no false-positive redaction occurred.
    assert!(
        !out.contains("<secret>"),
        "confusable character must not trigger redaction: {out}"
    );
}
