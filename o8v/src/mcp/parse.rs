// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP command parsing — tokenise, validate, and clap-parse a raw command string.

use clap::Parser;

pub(super) const MAX_COMMAND_LEN: usize = 65_536; // 64 KB

/// Parse a raw MCP command string into a typed `Command` enum.
///
/// Performs all validation: null byte check, size check, shell-style tokenization,
/// optional "8v" prefix stripping, path resolution against the containment root,
/// and clap parsing. Returns `Err(String)` with a human-readable error on any failure.
pub(super) fn parse_mcp_command(
    command: &str,
    containment_root: &o8v_fs::ContainmentRoot,
) -> Result<(crate::commands::Command, Vec<String>), String> {
    // Reject null bytes before any further processing — shlex treats them as
    // word separators on some platforms but they are never valid in a command.
    if command.contains('\0') {
        return Err("error: null bytes are not allowed in commands".to_string());
    }

    // Reject oversized commands to prevent memory exhaustion from pathological
    // shell-quoting expansion in shlex::split().
    if command.len() > MAX_COMMAND_LEN {
        return Err(format!(
            "error: command exceeds maximum length ({} bytes, limit {})",
            command.len(),
            MAX_COMMAND_LEN
        ));
    }

    // Parse command with shell-style quoting so that quoted arguments (e.g.
    // --find "start..end") are handled correctly.  split_whitespace() would
    // leave the literal quote characters in the token; shlex::split() strips
    // them, matching what a real shell would do.
    let parts = match shlex::split(command) {
        Some(p) => p,
        None => {
            return Err(format!(
                "8v: failed to parse command (unmatched quotes): {command}"
            ))
        }
    };
    // Strip leading "8v" token if the agent included the binary name.
    // Tool description shows `8v check .` so agents often include "8v".
    let parts: &[String] = if parts.first().map(|s| s.as_str()) == Some("8v") {
        &parts[1..]
    } else {
        &parts[..]
    };

    if parts.is_empty() {
        return Err("error: empty command".to_string());
    }

    // Hand shlex tokens directly to clap. One parser owns argv → Command mapping
    // for both CLI and MCP (no entry-point-specific parsing). Path resolution
    // against the MCP containment root happens after parsing, per-variant, on
    // the typed Command — never by matching subcommand name strings.
    let argv: Vec<&str> = std::iter::once("8v")
        .chain(parts.iter().map(String::as_str))
        .collect();

    let mut command = match crate::cli::Cli::try_parse_from(&argv) {
        Ok(cli) => cli.command,
        Err(e) => return Err(parse_error(e)),
    };

    command.resolve_mcp_paths(containment_root)?;

    // argv without the synthetic "8v" leader — matches what CLI captures
    // via std::env::args().skip(1) so both callers emit identical shapes.
    let argv_out: Vec<String> = argv.iter().skip(1).map(|s| (*s).to_string()).collect();

    Ok((command, argv_out))
}

/// Return error for command parsing failures.
pub(super) fn parse_error(e: clap::error::Error) -> String {
    match e.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            e.to_string()
        }
        _ => format!("error parsing command: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_containment_root(dir: &TempDir) -> o8v_fs::ContainmentRoot {
        let root_path = std::fs::canonicalize(dir.path()).unwrap();
        o8v_fs::ContainmentRoot::new(&root_path).unwrap()
    }

    #[test]
    fn null_byte_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let result = parse_mcp_command("check\0.", &root);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null bytes"));
    }

    #[test]
    fn oversized_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let command = "a".repeat(100 * 1024);
        let result = parse_mcp_command(&command, &root);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum length"));
    }

    #[test]
    fn empty_command_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let result = parse_mcp_command("", &root);
        assert!(result.is_err());
    }

    #[test]
    fn unmatched_quote_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let result = parse_mcp_command(r#"read "unterminated"#, &root);
        assert!(result.is_err());
    }

    #[test]
    fn shlex_strips_quotes() {
        let command = r#"write src/main.rs --find "start..end" --replace "start..=end""#;
        let parts = shlex::split(command).unwrap();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[3], "start..end");
        assert_eq!(parts[5], "start..=end");
    }

    // --- B-MCP-3 regression tests: multi-positional args without flags ---

    #[test]
    fn b_mcp_3_write_path_and_content_without_flags() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        // Before the fix, the pre-clap heuristic joined path and content into a
        // single argv element — clap would then reject or mis-parse it.
        let result = parse_mcp_command(
            r#"write src/main.rs:10 "    for i in start..=end {""#,
            &root,
        );
        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let (cmd, argv) = result.unwrap();
        assert_eq!(argv.first().map(|s| s.as_str()), Some("write"));
        match cmd {
            crate::commands::Command::Write(args) => {
                assert!(args.path.ends_with("src/main.rs:10"));
                assert_eq!(args.content.as_deref(), Some("    for i in start..=end {"));
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn b_mcp_3_write_content_with_leading_dashes() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        // allow_hyphen_values on content lets literal `--foo` through as content
        // rather than being treated as an unknown flag.
        let result = parse_mcp_command(r#"write src/main.rs:10 "-- a literal comment""#, &root);
        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let (cmd, _argv) = result.unwrap();
        match cmd {
            crate::commands::Command::Write(args) => {
                assert_eq!(args.content.as_deref(), Some("-- a literal comment"));
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn b_mcp_3_write_empty_content_allowed() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        let result = parse_mcp_command(r#"write src/main.rs:10 """#, &root);
        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let (cmd, _argv) = result.unwrap();
        match cmd {
            crate::commands::Command::Write(args) => {
                assert_eq!(args.content.as_deref(), Some(""));
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn b_mcp_3_search_pattern_and_path_without_flags() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        // Before the fix, `search` was string-name-gated out of path resolution
        // AND path was joined into the pattern. Now search's optional path is
        // resolved per-variant via the typed Command enum.
        let result = parse_mcp_command(r#"search "foo bar" src"#, &root);
        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let (cmd, _argv) = result.unwrap();
        match cmd {
            crate::commands::Command::Search(args) => {
                assert_eq!(args.pattern, "foo bar");
                assert!(args.path.as_deref().unwrap().ends_with("src"));
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }

    /// Class-of-bug coverage: for every command string, MCP parsing must
    /// produce the same `Command` variant that CLI parsing produces. If it
    /// doesn't, the MCP entry point is doing something CLI isn't — that's
    /// always a bug. This catches the whole family B-MCP-3 belongs to.
    #[test]
    fn mcp_and_cli_parse_identically_for_all_canonical_forms() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        // Write a fixture so path resolution has something to anchor to.
        std::fs::write(dir.path().join("a.rs"), "x").unwrap();

        let cases = [
            // (string, expected Command variant discriminant as &str)
            (r#"write a.rs:10 "hello world""#, "Write"),
            (r#"write a.rs:10 "    indented content""#, "Write"),
            (r#"write a.rs:10 "-- literal dashes""#, "Write"),
            (r#"write a.rs:10 """#, "Write"),
            (r#"write a.rs --find "old" --replace "new""#, "Write"),
            (r#"read a.rs"#, "Read"),
            (r#"read a.rs:1-10"#, "Read"),
            (r#"search "foo bar""#, "Search"),
            (r#"search "foo bar" ."#, "Search"),
            (r#"check ."#, "Check"),
            (r#"fmt ."#, "Fmt"),
            (r#"ls"#, "Ls"),
            (r#"ls --tree"#, "Ls"),
        ];

        for (cmd, want) in cases {
            let mcp = parse_mcp_command(cmd, &root);
            assert!(mcp.is_ok(), "MCP parse failed for `{cmd}`: {:?}", mcp.err());
            let got = match mcp.unwrap().0 {
                crate::commands::Command::Write(_) => "Write",
                crate::commands::Command::Read(_) => "Read",
                crate::commands::Command::Search(_) => "Search",
                crate::commands::Command::Check(_) => "Check",
                crate::commands::Command::Fmt(_) => "Fmt",
                crate::commands::Command::Ls(_) => "Ls",
                crate::commands::Command::Build(_) => "Build",
                crate::commands::Command::Test(_) => "Test",
                crate::commands::Command::Init(_) => "Init",
                crate::commands::Command::Hooks(_) => "Hooks",
                crate::commands::Command::Upgrade(_) => "Upgrade",
                crate::commands::Command::Mcp => "Mcp",
            };
            assert_eq!(got, want, "wrong Command variant for `{cmd}`");
        }
    }

    #[test]
    fn write_with_flag_still_works() {
        let dir = TempDir::new().unwrap();
        let root = make_containment_root(&dir);
        // Regression: the flag-present case worked before the fix; verify no
        // regression.
        let result = parse_mcp_command(r#"write src/main.rs --find "foo" --replace "bar""#, &root);
        assert!(result.is_ok(), "expected success, got {:?}", result.err());
    }
}
