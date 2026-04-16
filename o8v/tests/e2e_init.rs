// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for 8v init command

use std::process::Command;

/// Path to the compiled binary. Set by Cargo for integration tests.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// `8v init --yes` creates .8v/ locally without interactive prompts.
#[test]
fn init_yes_creates_dot8v() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init --yes should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dot8v = path.join(".8v");
    assert!(
        dot8v.exists(),
        ".8v/ must exist after init --yes\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dot8v.is_dir(), ".8v must be a directory");
}

/// `8v init --yes` works without an interactive terminal.
#[test]
fn init_yes_works_without_terminal() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = std::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "{} init {} --yes </dev/null",
                env!("CARGO_BIN_EXE_8v"),
                path.display()
            ),
        ])
        .output()
        .expect("run init --yes with stdin closed");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init --yes should work even with closed stdin\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dot8v = path.join(".8v");
    assert!(dot8v.exists(), ".8v/ must exist when stdin is closed",);
}

/// `8v init --yes` creates MCP and documentation files (no .git, so hooks are skipped).
#[test]
fn init_yes_creates_mcp_and_docs() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init --yes should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mcp_json = path.join(".mcp.json");
    let claude_md = path.join("CLAUDE.md");
    let agents_md = path.join("AGENTS.md");

    assert!(
        mcp_json.exists(),
        ".mcp.json should be created in --yes mode"
    );
    assert!(
        claude_md.exists(),
        "CLAUDE.md should be created in --yes mode"
    );
    assert!(
        agents_md.exists(),
        "AGENTS.md should be created in --yes mode"
    );

    // settings.json must allow 8v and deny native tools
    let settings_path = path.join(".claude/settings.json");
    assert!(
        settings_path.exists(),
        ".claude/settings.json should be created"
    );
    let settings = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let v: serde_json::Value = serde_json::from_str(&settings).expect("parse settings.json");
    let allow = v["permissions"]["allow"]
        .as_array()
        .expect("permissions.allow must be an array");
    assert!(
        allow.iter().any(|v| v == "mcp__8v__8v"),
        "must allow mcp__8v__8v"
    );
    let deny = v["permissions"]["deny"]
        .as_array()
        .expect("permissions.deny must be an array");
    for tool in ["Read", "Edit", "Write", "Glob", "Grep", "NotebookEdit"] {
        assert!(
            deny.iter().any(|v| v == tool),
            "must deny {tool} — got: {settings}"
        );
    }
}
