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

// ─── F-3: CLAUDE.md duplication ─────────────────────────────────────────────

/// F-3: When AGENTS.md already has the 8v block, init must NOT append it to CLAUDE.md.
#[test]
fn init_skips_claude_md_when_agents_md_has_8v_block() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    // Pre-populate AGENTS.md with the current-version 8v versioned sentinel block.
    // file_has_current_block checks for the HTML comment sentinels, not the old "# 8v" marker.
    let agents_md = path.join("AGENTS.md");
    let version = env!("CARGO_PKG_VERSION");
    let agents_sentinel =
        format!("<!-- 8v:begin v{version} -->\nAlready has 8v instructions.\n<!-- 8v:end -->\n");
    std::fs::write(&agents_md, &agents_sentinel).expect("write AGENTS.md");

    // CLAUDE.md exists but does NOT have the 8v block
    let claude_md = path.join("CLAUDE.md");
    std::fs::write(&claude_md, "# My Project\n\nExisting instructions.\n")
        .expect("write CLAUDE.md");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init should still exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // CLAUDE.md must NOT contain the versioned 8v sentinel (AGENTS.md already has it)
    let claude_content = std::fs::read_to_string(&claude_md).expect("read CLAUDE.md");
    assert!(
        !claude_content.contains("<!-- 8v:begin"),
        "CLAUDE.md must NOT get the 8v block when AGENTS.md already has it;\ngot:\n{claude_content}"
    );

    // AGENTS.md must be unchanged (still has the same versioned sentinel we wrote)
    let agents_content = std::fs::read_to_string(&agents_md).expect("read AGENTS.md");
    assert_eq!(
        agents_content, agents_sentinel,
        "AGENTS.md must be unchanged"
    );
}

/// F-3 idempotency: running init twice must not duplicate the 8v block in CLAUDE.md.
#[test]
fn init_idempotent_no_duplicate_8v_block() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    // Run init once
    let out1 = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes (first)");

    assert_eq!(
        out1.status.code(),
        Some(0),
        "first init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    // Run init again (idempotency check)
    let out2 = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes (second)");

    assert_eq!(
        out2.status.code(),
        Some(0),
        "second init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    // CLAUDE.md must contain "# 8v" exactly once
    let claude_md = path.join("CLAUDE.md");
    let claude_content = std::fs::read_to_string(&claude_md).expect("read CLAUDE.md");
    let count = claude_content.matches("# 8v").count();
    assert_eq!(
        count, 1,
        "CLAUDE.md must contain '# 8v' exactly once after two inits; got {count} occurrences:\n{claude_content}"
    );
}

// ─── F-4: Stack undetected warning ──────────────────────────────────────────

/// F-4: init on a path with no stack-identifying files must exit 0 BUT warn the user.
#[test]
fn init_warns_when_no_stack_detected() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    // No Cargo.toml, no package.json, no go.mod, etc. — pure empty dir.
    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init must still exit 0 on unknown stack\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");

    assert!(
        combined.contains("warning: no stack detected"),
        "init must warn when no stack is detected;\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

// ─── Versioned sentinel tests ────────────────────────────────────────────────

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// T-1: Fresh install — no sentinels present → init writes block with current version sentinel.
#[test]
fn versioned_sentinel_fresh_install() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let agents_md = path.join("AGENTS.md");
    let content = std::fs::read_to_string(&agents_md).expect("read AGENTS.md");
    let expected_begin = format!("<!-- 8v:begin v{} -->", CURRENT_VERSION);
    assert!(
        content.contains(&expected_begin),
        "AGENTS.md must contain begin sentinel with current version '{}';\ngot:\n{content}",
        CURRENT_VERSION
    );
    assert!(
        content.contains("<!-- 8v:end -->"),
        "AGENTS.md must contain end sentinel;\ngot:\n{content}"
    );
}

/// T-2: Already current — file has current-version block → init is a no-op, message says "already current".
#[test]
fn versioned_sentinel_already_current() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let agents_md = path.join("AGENTS.md");
    let begin = format!("<!-- 8v:begin v{} -->", CURRENT_VERSION);
    let existing = format!("{begin}\nsome 8v content\n<!-- 8v:end -->\n");
    std::fs::write(&agents_md, &existing).expect("write AGENTS.md");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // File must be unchanged
    let content = std::fs::read_to_string(&agents_md).expect("read AGENTS.md");
    assert_eq!(
        content, existing,
        "AGENTS.md must be unchanged when version is already current"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.to_lowercase().contains("already current")
            || combined.to_lowercase().contains("up to date")
            || combined.to_lowercase().contains("up-to-date"),
        "init must print 'already current' or similar;\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

/// T-3: Outdated → upgrade — old version block gets replaced with current version.
#[test]
fn versioned_sentinel_upgrade_old_version() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let agents_md = path.join("AGENTS.md");
    let old_content = "# Preamble\n\n<!-- 8v:begin v0.0.1 -->\nOLD CONTENT HERE\n<!-- 8v:end -->\n\n# Postamble\n";
    std::fs::write(&agents_md, old_content).expect("write AGENTS.md");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let content = std::fs::read_to_string(&agents_md).expect("read AGENTS.md");

    // Must have current version begin sentinel
    let expected_begin = format!("<!-- 8v:begin v{} -->", CURRENT_VERSION);
    assert!(
        content.contains(&expected_begin),
        "AGENTS.md must contain new begin sentinel;\ngot:\n{content}"
    );

    // Must NOT have old version sentinel
    assert!(
        !content.contains("<!-- 8v:begin v0.0.1 -->"),
        "AGENTS.md must NOT contain old begin sentinel;\ngot:\n{content}"
    );

    // Old content must be gone
    assert!(
        !content.contains("OLD CONTENT HERE"),
        "AGENTS.md must NOT contain old block content;\ngot:\n{content}"
    );

    // Preamble and postamble must be preserved byte-for-byte
    assert!(
        content.starts_with("# Preamble\n\n"),
        "preamble must be preserved;\ngot:\n{content}"
    );
    assert!(
        content.contains("\n\n# Postamble\n"),
        "postamble must be preserved;\ngot:\n{content}"
    );

    // Print upgrade message
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("0.0.1") || combined.to_lowercase().contains("updated"),
        "init must print an upgrade message mentioning old version or 'updated';\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

/// T-4: AGENTS.md dedup — AGENTS.md has current-version block → CLAUDE.md gets no block.
#[test]
fn versioned_sentinel_agents_dedup_skips_claude() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let agents_md = path.join("AGENTS.md");
    let begin = format!("<!-- 8v:begin v{} -->", CURRENT_VERSION);
    let agents_content = format!("{begin}\nsome 8v content\n<!-- 8v:end -->\n");
    std::fs::write(&agents_md, &agents_content).expect("write AGENTS.md");

    let claude_md = path.join("CLAUDE.md");
    std::fs::write(&claude_md, "# My Project\n\nExisting.\n").expect("write CLAUDE.md");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let claude_content = std::fs::read_to_string(&claude_md).expect("read CLAUDE.md");
    assert!(
        !claude_content.contains("<!-- 8v:begin"),
        "CLAUDE.md must NOT get versioned sentinel when AGENTS.md already has current block;\ngot:\n{claude_content}"
    );
}

/// T-5: Malformed state — begin sentinel present but no end sentinel → init fails loudly.
#[test]
fn versioned_sentinel_malformed_no_end() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let agents_md = path.join("AGENTS.md");
    let malformed = "# Preamble\n\n<!-- 8v:begin v0.1.0 -->\nsome content\n# NO END SENTINEL\n";
    std::fs::write(&agents_md, malformed).expect("write AGENTS.md");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    // Must NOT exit 0
    assert_ne!(
        out.status.code(),
        Some(0),
        "init must fail on malformed state (begin without end)\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.to_lowercase().contains("malformed")
            || combined.to_lowercase().contains("missing")
            || combined.contains("8v:end"),
        "init must print a clear error about malformed state;\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

/// T-6: AGENTS.md unreadable (directory) → init must print warning: line, not silently skip.
#[test]
fn versioned_sentinel_agents_md_unreadable_warns() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    // Create AGENTS.md as a directory so read_to_string fails
    let agents_md = path.join("AGENTS.md");
    std::fs::create_dir(&agents_md).expect("create AGENTS.md as directory");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");

    assert!(
        combined.contains("warning:") && combined.to_lowercase().contains("agents.md"),
        "init must print 'warning: ... AGENTS.md ...' when AGENTS.md is unreadable;\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

// ─── I-1: honest hook-install reporting in non-git dirs ─────────────────────
//
// Pre-fix bug: `8v init --yes` in a directory without a .git/ always printed
//   ✓ Pre-commit hook installed
//   ✓ Commit-msg hook installed
// because install_git_{pre_commit,commit_msg} return Ok(()) on the "no .git"
// branch. The success line lies — nothing was installed. I-1 forces the init
// driver to report the actual outcome.

/// `8v init --yes` in a non-git dir must not claim the pre-commit hook was
/// installed — there's no .git/hooks/ to install into.
#[test]
fn i1_init_yes_does_not_claim_pre_commit_hook_installed_in_non_git_dir() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();
    assert!(
        !path.join(".git").exists(),
        "preflight: tmpdir must not have .git"
    );

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");

    assert_eq!(
        out.status.code(),
        Some(0),
        "init --yes in a non-git dir should still exit 0 (other files still created)\nstderr: {stderr}"
    );
    assert!(
        !combined.contains("✓ Pre-commit hook installed"),
        "init in non-git dir must not falsely claim '✓ Pre-commit hook installed'\nstderr: {stderr}"
    );
    assert!(
        !path.join(".git").exists(),
        "init must not create .git/ as a side effect"
    );
}

/// Same contract for the commit-msg hook.
#[test]
fn i1_init_yes_does_not_claim_commit_msg_hook_installed_in_non_git_dir() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");

    assert!(
        !combined.contains("✓ Commit-msg hook installed"),
        "init in non-git dir must not falsely claim '✓ Commit-msg hook installed'\nstderr: {stderr}"
    );
}

// ─── I-2: re-run message names the right file ───────────────────────────────
//
// Pre-fix bug: on re-run, CLAUDE.md also has the current 8v block (from the
// first run), but init's CLAUDE.md step only checks AGENTS.md for dedup and
// prints "Skipped CLAUDE.md (AGENTS.md already has current 8v block)" — as if
// CLAUDE.md was skipped because of a dedup rule. The honest message on re-run
// is "CLAUDE.md already current (vX)" since CLAUDE.md itself is current.

/// On re-run (where both files are already current), init must report
/// "CLAUDE.md already current" for the CLAUDE.md step, not the dedup-skip
/// message which blames AGENTS.md.
#[test]
fn i2_init_yes_rerun_reports_claude_md_already_current() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();

    // First run: installs 8v blocks into both CLAUDE.md and AGENTS.md.
    let first = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run first 8v init --yes");
    assert_eq!(first.status.code(), Some(0), "first init must exit 0");

    // Second run: CLAUDE.md is already current; we want to see that reported.
    let second = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run second 8v init --yes");
    let stderr = String::from_utf8_lossy(&second.stderr);
    let stdout = String::from_utf8_lossy(&second.stdout);
    let combined = format!("{stderr}{stdout}");

    assert_eq!(second.status.code(), Some(0), "re-run init must exit 0");
    assert!(
        combined.contains("CLAUDE.md already current"),
        "re-run must report 'CLAUDE.md already current' (truthful message);\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !combined.contains("Skipped CLAUDE.md"),
        "re-run must not print the dedup 'Skipped CLAUDE.md' message when CLAUDE.md itself is already current;\ngot stderr:\n{stderr}\nstdout:\n{stdout}"
    );
}

/// With .git present, the success line is still emitted — the fix is scoped
/// to the no-git branch, not a behavior flip.
#[test]
fn i1_init_yes_still_claims_hook_installed_when_git_present() {
    let tmpdir = tempfile::tempdir().expect("tmpdir");
    let path = tmpdir.path();
    std::fs::create_dir_all(path.join(".git/hooks")).expect("mkdir .git/hooks");

    let out = bin()
        .args(["init", path.to_str().unwrap(), "--yes"])
        .output()
        .expect("run 8v init --yes");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");

    assert!(
        combined.contains("✓ Pre-commit hook installed"),
        "init must claim '✓ Pre-commit hook installed' when .git exists\nstderr: {stderr}"
    );
    assert!(
        path.join(".git/hooks/pre-commit").exists(),
        "pre-commit script must exist after successful install"
    );
}
