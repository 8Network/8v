// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v init --help` output — correct arg descriptions, no positionals.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn init_help() -> String {
    let out = bin()
        .args(["init", "--help"])
        .output()
        .expect("run 8v init --help");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Finding 3: `8v init --help` must not show `[MCP_COMMAND]` as a positional argument.
/// Before fix: mcp_command had no #[arg], making it a positional with an empty description.
#[test]
fn init_help_no_mcp_command_positional() {
    let help = init_help();
    assert!(
        !help.contains("[MCP_COMMAND]"),
        "init --help must not show [MCP_COMMAND] positional\nhelp output:\n{help}"
    );
}

/// Finding 2: `--mcp-name` description must not contain mcp-command text.
/// Before fix: both #[arg] attrs were stacked on mcp_name, so its help text
/// was a concatenation of both doc comments.
#[test]
fn init_help_mcp_name_has_correct_description() {
    let help = init_help();

    // --mcp-name must appear as a proper named option
    assert!(
        help.contains("--mcp-name"),
        "init --help must show --mcp-name option\nhelp output:\n{help}"
    );

    // --mcp-command must also appear as a proper named option
    assert!(
        help.contains("--mcp-command"),
        "init --help must show --mcp-command option\nhelp output:\n{help}"
    );

    // The description following --mcp-name must not mention the benchmark harness
    // (that belongs to --mcp-command's description only).
    // We check by finding the --mcp-name section and verifying it doesn't bleed
    // into mcp-command's text.
    let mcp_name_pos = help.find("--mcp-name").expect("--mcp-name present");
    let after_mcp_name = &help[mcp_name_pos..];
    // Find the next option (starts with whitespace + '--')
    // The description for --mcp-name should not say "benchmark harness"
    assert!(
        !after_mcp_name
            .lines()
            .take(6)
            .any(|l| l.contains("benchmark harness")),
        "--mcp-name description must not contain benchmark harness text (belongs to --mcp-command)\nhelp output:\n{help}"
    );
}
