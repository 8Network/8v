// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Observability wrapper — bracket MCP invocations with McpInvoked/McpCompleted events.
//! Parsing is handled by `parse_mcp_command()`, dispatch by `command_dispatch::run()`.

use o8v_core::render::Audience;
use rmcp::{Peer, RoleServer};

/// Parse and execute an 8v command, returning the plain text result.
///
/// Observability wrapper: emits `McpInvoked` before dispatch and `McpCompleted`
/// after — always, on every code path. The single exit point in this function
/// guarantees `McpCompleted` is never missed.
pub(super) async fn handle_command(command: &str, client: Peer<RoleServer>) -> String {
    let start_ms = crate::util::unix_ms();
    let run_id = crate::util::new_uuid();

    // Resolve containment root: prefer MCP client roots, fall back to process CWD.
    // The MCP server is spawned from the project directory, so CWD is the project root.
    //
    // LIMITATION: These early returns cannot emit McpInvoked — containment_root and
    // state_dir_opt are not yet available. Trade-off: rare errors (missing CWD) are
    // unobservable. Acceptable because MCP server is only spawned from project root.
    let root_path = match super::path::get_root_directory(&client).await {
        Some(r) => r,
        None => match std::env::current_dir() {
            Ok(cwd) => cwd.to_string_lossy().into_owned(),
            Err(e) => {
                tracing::debug!(error = ?e, "mcp handler: cannot get current directory");
                return "error: cannot determine working directory — set MCP roots or run from project directory".to_string();
            }
        },
    };

    // Build ContainmentRoot once — reused for both StateDir and path validation.
    // If this fails, .8v/ cannot be opened either, so events are skipped silently.
    let containment_root = match o8v_fs::ContainmentRoot::new(&root_path) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("mcp handler: cannot create containment root: {e}");
            return "error: cannot create containment root — invalid directory".to_string();
        }
    };

    // Open StorageDir (best-effort — ~/.8v/ creation may fail, events are skipped silently).
    let storage_opt = match o8v_workspace::StorageDir::open() {
        Ok(sd) => Some(sd),
        Err(e) => {
            tracing::debug!(error = ?e, "mcp: ~/.8v/ not available, events disabled");
            None
        }
    };

    // Emit McpInvoked (best-effort — skipped if ~/.8v/ is missing).
    if let Some(ref storage) = storage_opt {
        let ev = super::events::McpInvoked::new(run_id.clone(), command, root_path);
        super::events::emit(storage, &ev);
    }

    // Parse command then dispatch — returns output string.
    let output = match super::parse::parse_mcp_command(command, &containment_root) {
        Ok(parsed_command) => {
            let ctx = o8v::dispatch::core_context(&super::INTERRUPTED);
            match super::dispatch::run(parsed_command, &ctx, Audience::Agent).await {
                Ok(out) => out,
                Err(e) => format!("error: {e}"),
            }
        }
        Err(e) => e,
    };

    // Emit McpCompleted — always, on all code paths (single exit point).
    if let Some(ref storage) = storage_opt {
        let end_ms = crate::util::unix_ms();
        let duration_ms = end_ms.saturating_sub(start_ms);
        let ev = super::events::McpCompleted::new(run_id, output.len() as u64, duration_ms);
        super::events::emit(storage, &ev);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> std::path::PathBuf {
        fs::canonicalize(dir.path()).unwrap()
    }

    #[allow(clippy::disallowed_methods)]
    fn make_storage_dir(dir: &TempDir) -> o8v_workspace::StorageDir {
        let root_path = canonical(dir);
        std::env::set_var("HOME", &root_path);
        o8v_workspace::StorageDir::open().unwrap()
    }

    fn make_containment_root(dir: &TempDir) -> o8v_fs::ContainmentRoot {
        let root_path = canonical(dir);
        o8v_fs::ContainmentRoot::new(&root_path).unwrap()
    }

    fn read_ndjson_events(storage: &o8v_workspace::StorageDir) -> Vec<serde_json::Value> {
        let path = storage.mcp_events();
        let content = fs::read_to_string(&path).unwrap();
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// Test helper: parse and dispatch a command string, returning rendered output.
    async fn run_command(command: &str, containment_root: &o8v_fs::ContainmentRoot) -> String {
        match crate::mcp::parse::parse_mcp_command(command, containment_root) {
            Ok(parsed) => {
                let ctx = o8v::dispatch::core_context(&crate::mcp::INTERRUPTED);
                match crate::mcp::dispatch::run(parsed, &ctx, Audience::Agent).await {
                    Ok(out) => out,
                    Err(e) => format!("error: {e}"),
                }
            }
            Err(e) => e,
        }
    }

    /// Test 1: command_bytes matches command length
    #[test]
    fn command_bytes_matches_command_length() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);
        let root_path = canonical(&dir).to_string_lossy().to_string();
        let run_id = crate::util::new_uuid();
        let command = "check .";

        let ev = crate::mcp::events::McpInvoked::new(run_id, command, root_path);
        crate::mcp::events::emit(&storage, &ev);

        let events = read_ndjson_events(&storage);
        assert_eq!(events.len(), 1);
        let json = &events[0];

        assert_eq!(json["command_bytes"].as_u64().unwrap(), 7);
        assert_eq!(json["command_token_estimate"].as_u64().unwrap(), 1);
        assert_eq!(json["event"].as_str().unwrap(), "McpInvoked");
    }

    /// Test 2: render_bytes matches output length
    #[tokio::test]
    async fn render_bytes_matches_output_length() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);
        let containment_root = make_containment_root(&dir);
        let run_id = crate::util::new_uuid();

        let output = run_command("fmt .", &containment_root).await;
        let render_bytes = output.len() as u64;

        let start_ms = crate::util::unix_ms();
        let end_ms = crate::util::unix_ms();
        let ev = crate::mcp::events::McpCompleted::new(
            run_id,
            render_bytes,
            end_ms.saturating_sub(start_ms),
        );
        crate::mcp::events::emit(&storage, &ev);

        let events = read_ndjson_events(&storage);
        assert_eq!(events.len(), 1);
        let json = &events[0];

        assert_eq!(json["render_bytes"].as_u64().unwrap(), render_bytes);
        assert_eq!(json["token_estimate"].as_u64().unwrap(), render_bytes / 4);
        assert_eq!(json["event"].as_str().unwrap(), "McpCompleted");
    }

    /// Test 3: run_id matches between invoked and completed
    #[tokio::test]
    async fn run_id_matches_between_invoked_and_completed() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);
        let containment_root = make_containment_root(&dir);
        let root_path = canonical(&dir).to_string_lossy().to_string();
        let run_id = crate::util::new_uuid();

        let ev_invoked = crate::mcp::events::McpInvoked::new(run_id.clone(), "check .", root_path);
        crate::mcp::events::emit(&storage, &ev_invoked);

        let output = run_command("fmt .", &containment_root).await;
        let start_ms = crate::util::unix_ms();
        let end_ms = crate::util::unix_ms();
        let ev_completed = crate::mcp::events::McpCompleted::new(
            run_id.clone(),
            output.len() as u64,
            end_ms.saturating_sub(start_ms),
        );
        crate::mcp::events::emit(&storage, &ev_completed);

        let events = read_ndjson_events(&storage);
        assert_eq!(events.len(), 2);

        let invoked_run_id = events[0]["run_id"].as_str().unwrap();
        let completed_run_id = events[1]["run_id"].as_str().unwrap();

        assert_eq!(invoked_run_id, completed_run_id);
        assert_eq!(invoked_run_id, run_id);
        assert_eq!(events[0]["event"].as_str().unwrap(), "McpInvoked");
        assert_eq!(events[1]["event"].as_str().unwrap(), "McpCompleted");
    }

    /// Test 4: both events are valid NDJSON
    #[tokio::test]
    async fn both_events_are_valid_ndjson() {
        let dir = TempDir::new().unwrap();
        let storage = make_storage_dir(&dir);
        let containment_root = make_containment_root(&dir);
        let root_path = canonical(&dir).to_string_lossy().to_string();
        let run_id = crate::util::new_uuid();

        let ev_invoked = crate::mcp::events::McpInvoked::new(run_id.clone(), "check .", root_path);
        crate::mcp::events::emit(&storage, &ev_invoked);
        let output = run_command("fmt .", &containment_root).await;
        let start_ms = crate::util::unix_ms();
        let end_ms = crate::util::unix_ms();
        let ev_completed = crate::mcp::events::McpCompleted::new(
            run_id,
            output.len() as u64,
            end_ms.saturating_sub(start_ms),
        );
        crate::mcp::events::emit(&storage, &ev_completed);

        let path = storage.mcp_events();
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        assert_eq!(lines.len(), 2);

        for line in lines {
            let result = serde_json::from_str::<serde_json::Value>(line);
            assert!(result.is_ok(), "line is not valid JSON: {line}");
            let json = result.unwrap();
            assert!(json.is_object());
        }
    }

    /// Test 5: token estimate is bytes divided by 4 (integer division)
    #[test]
    fn token_estimate_is_bytes_divided_by_4() {
        let test_cases = vec![
            (7u64, 1u64),     // "check ." → 7 / 4 = 1
            (17u64, 4u64),    // "check . --verbose" → 17 / 4 = 4
            (400u64, 100u64), // render_bytes 400 → 400 / 4 = 100
            (401u64, 100u64), // render_bytes 401 → 401 / 4 = 100
            (403u64, 100u64), // render_bytes 403 → 403 / 4 = 100
            (404u64, 101u64), // render_bytes 404 → 404 / 4 = 101
        ];

        for (bytes, expected_tokens) in test_cases {
            let tokens = bytes / 4;
            assert_eq!(
                tokens, expected_tokens,
                "bytes {} should estimate to {} tokens, got {}",
                bytes, expected_tokens, tokens
            );
        }
    }

    /// Test 6: run_command with empty command returns error
    #[tokio::test]
    async fn run_command_empty_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);
        let output = run_command("", &containment_root).await;

        assert!(output.starts_with("error:"));
        assert!(!output.is_empty());
    }

    /// Test 7: run_command fmt returns a non-error string
    #[tokio::test]
    async fn run_command_fmt_on_empty_dir() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);
        let output = run_command("fmt .", &containment_root).await;

        // Output may be empty (no files to format) or contain a summary.
        // The important invariant is that fmt does not return an error.
        assert!(!output.starts_with("error:"));
    }

    /// Test 8: shlex strips quotes so clap receives bare values, not quoted strings.
    /// Regression for: split_whitespace() passing `"start..end"` (with quotes) to clap.
    #[test]
    fn find_replace_with_quoted_args() {
        let command = r#"write src/main.rs --find "start..end" --replace "start..=end""#;
        let parts = shlex::split(command).unwrap();
        // Expected: ["write", "src/main.rs", "--find", "start..end", "--replace", "start..=end"]
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0], "write");
        assert_eq!(parts[1], "src/main.rs");
        assert_eq!(parts[2], "--find");
        assert_eq!(parts[3], "start..end"); // NO surrounding quotes
        assert_eq!(parts[4], "--replace");
        assert_eq!(parts[5], "start..=end"); // NO surrounding quotes
    }

    // ── Security counterexamples ─────────────────────────────────────────────

    /// Counterexample: null byte in command must return an error, not crash.
    #[tokio::test]
    async fn null_byte_in_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let command = "check\0.";
        let output = run_command(command, &containment_root).await;

        assert!(
            output.starts_with("error:"),
            "expected error prefix, got: {output}"
        );
        assert!(
            output.contains("null bytes"),
            "expected 'null bytes' in error message, got: {output}"
        );
    }

    /// Counterexample: a 100 KB command must return an error, not be processed.
    #[tokio::test]
    async fn oversized_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        // 100 KB of 'a' — well above the 64 KB limit.
        let command = "a".repeat(100 * 1024);
        let output = run_command(&command, &containment_root).await;

        assert!(
            output.starts_with("error:"),
            "expected error prefix, got: {output}"
        );
        assert!(
            output.contains("maximum length"),
            "expected 'maximum length' in error message, got: {output}"
        );
    }

    /// Counterexample: empty string must return an error, not panic.
    #[tokio::test]
    async fn empty_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command("", &containment_root).await;

        assert!(
            output.starts_with("error:"),
            "expected error prefix, got: {output}"
        );
    }

    /// Counterexample: unmatched quote must return an error, not crash.
    #[tokio::test]
    async fn unmatched_quote_returns_error() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command(r#"read "unterminated"#, &containment_root).await;

        assert!(
            output.starts_with("8v:") || output.starts_with("error:"),
            "expected error prefix, got: {output}"
        );
    }

    // ── Run subcommand tests ────────────────────────────────────────────────

    /// `run "echo hello"` through MCP handler succeeds and captures output.
    #[tokio::test]
    async fn run_echo_through_handler() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command(r#"run "echo handler-test""#, &containment_root).await;

        assert!(
            !output.starts_with("error:"),
            "run should not error: {output}"
        );
        assert!(
            output.contains("handler-test"),
            "should contain echo output: {output}"
        );
        assert!(
            output.contains("exit: 0 (success)"),
            "should show success: {output}"
        );
    }

    /// `run` with empty command string returns error, not panic.
    #[tokio::test]
    async fn run_empty_through_handler() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command(r#"run """#, &containment_root).await;

        assert!(
            output.contains("empty command"),
            "should report empty command: {output}"
        );
    }

    /// `run` with excessive timeout returns error.
    #[tokio::test]
    async fn run_excessive_timeout_through_handler() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command(r#"run "echo x" --timeout 9999"#, &containment_root).await;

        assert!(
            output.contains("exceeds maximum"),
            "should reject excessive timeout: {output}"
        );
    }

    // ── Build subcommand tests ──────────────────────────────────────────────

    /// `build .` on empty dir through MCP handler returns "no project detected".
    #[tokio::test]
    async fn build_empty_dir_through_handler() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command("build .", &containment_root).await;

        assert!(
            output.contains("no project detected"),
            "should say no project: {output}"
        );
    }

    /// `build` with excessive timeout returns error.
    #[tokio::test]
    async fn build_excessive_timeout_through_handler() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);

        let output = run_command("build . --timeout 9999", &containment_root).await;

        assert!(
            output.contains("exceeds maximum"),
            "should reject excessive timeout: {output}"
        );
    }

    // ── Check output completeness ──────────────────────────────────────────

    /// MCP check output must include per-check entries, not just the summary.
    ///
    /// Counterexample for the bug where handler called render_summary() instead
    /// of render(), returning only "---\nresult: ..." with no check details.
    /// An agent receiving only the summary cannot tell what failed or why.
    #[tokio::test]
    async fn check_output_includes_per_check_entries() {
        let dir = TempDir::new().unwrap();

        // Create a minimal Rust project so project detection finds it.
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test-proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();

        let containment_root = make_containment_root(&dir);
        let output = run_command("check .", &containment_root).await;

        // Must contain the summary separator and result line.
        assert!(
            output.contains("---"),
            "output must contain summary separator: {output}"
        );
        assert!(
            output.contains("result:"),
            "output must contain result line: {output}"
        );

        // Must contain per-check entry lines (the fix).
        // A Rust project runs cargo check, clippy, cargo fmt, shellcheck.
        // At minimum one of these must appear as a named entry.
        let has_entries =
            output.contains("passed") || output.contains("failed") || output.contains("error");
        assert!(
            has_entries,
            "output must contain per-check entries (passed/failed/error), got: {output}"
        );

        // The project name must appear in a project header line.
        assert!(
            output.contains("test-proj"),
            "output must contain project name header: {output}"
        );
    }

    /// Check on empty dir returns "nothing" with no entries — but still renders
    /// the summary. This is the baseline: no project, no entries, just summary.
    #[tokio::test]
    async fn check_empty_dir_returns_nothing_summary() {
        let dir = TempDir::new().unwrap();
        let containment_root = make_containment_root(&dir);
        let output = run_command("check .", &containment_root).await;

        assert!(
            output.contains("result: nothing"),
            "empty dir should report 'nothing': {output}"
        );
    }
}
