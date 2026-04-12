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
) -> Result<crate::commands::Command, String> {
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
    let mut args: Vec<String> = Vec::new();

    // Strip leading "8v" token if the agent included the binary name.
    // Tool description shows `8v check .` so agents often include "8v".
    let parts = if parts.first().map(|s| s.as_str()) == Some("8v") {
        &parts[1..]
    } else {
        &parts[..]
    };

    if parts.is_empty() {
        return Err("error: empty command".to_string());
    }

    args.push(parts[0].clone()); // subcommand (check, fmt, etc.)

    if parts.len() > 1 {
        // Find the index where flags begin (first arg starting with '-')
        let flag_start = parts[1..]
            .iter()
            .position(|p| p.starts_with('-'))
            .map(|i| i + 1);

        match flag_start {
            Some(idx) => {
                // Everything before flags is the path (join with spaces)
                let path = parts[1..idx].join(" ");
                args.push(path);
                // Add all flags
                for flag in &parts[idx..] {
                    args.push(flag.clone());
                }
            }
            None => {
                // No flags found — everything after subcommand is the path
                let path = parts[1..].join(" ");
                args.push(path);
            }
        }
    }

    // Resolve and validate path argument using the already-built ContainmentRoot.
    // For 8v, the path is the second argument (after the command like "check" or "fmt").
    // Exception: "search" — args[1] is the regex pattern, not a path.
    // SearchCommand handles its own path resolution internally.
    let subcommand = args[0].as_str();
    if args.len() > 1 && subcommand != "search" && subcommand != "ls" && subcommand != "run" {
        super::path::resolve_command_path(&mut args, containment_root)?;
    }

    // Convert to &str for clap parsing.
    // try_parse_from requires the binary name as the first element.
    let args_refs: Vec<&str> = std::iter::once("8v")
        .chain(args.iter().map(|s| s.as_str()))
        .collect();

    match crate::cli::Cli::try_parse_from(args_refs) {
        Ok(cli) => Ok(cli.command),
        Err(e) => Err(parse_error(e)),
    }
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
}
