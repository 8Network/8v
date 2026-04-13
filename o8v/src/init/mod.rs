// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The `init` command — interactive project setup for 8v.

mod ai_docs;
mod claude_settings;
mod mcp_setup;

use crate::cli::common::{EXIT_FAIL, EXIT_OK};
use crate::hooks::install::{install_claude_hooks, install_git_commit_msg, install_git_pre_commit};
use ai_docs::{append_section_if_missing, AI_SECTION};
use claude_settings::setup_claude_settings;
use dialoguer::{Confirm, Select};
use mcp_setup::setup_mcp_json;
use o8v_core::project::{ProjectKind, ProjectRoot};
use o8v_stacks::detect_all;
use o8v::workspace::{register_workspace, WorkspaceDir};
use std::fs;
use std::io::IsTerminal;
use std::process::ExitCode;

// ─── Defaults ────────────────────────────────────────────────────────────────

const DEFAULT_CONFIG: &str = "[git]\nstrip_attribution = true\n";

// ─── InitDir — path value object for init-time project files ────────────────

/// Owns path knowledge for files written during `8v init`.
/// All `.join("literal")` calls live here; callers use named methods.
struct InitDir {
    mcp_json: std::path::PathBuf,
    claude_md: std::path::PathBuf,
    agents_md: std::path::PathBuf,
}

impl InitDir {
    const MCP_JSON: &'static str = ".mcp.json";
    const CLAUDE_MD: &'static str = "CLAUDE.md";
    const AGENTS_MD: &'static str = "AGENTS.md";

    fn new(root: &o8v_fs::ContainmentRoot) -> Self {
        let base = root.as_path();
        Self {
            mcp_json: base.join(Self::MCP_JSON),
            claude_md: base.join(Self::CLAUDE_MD),
            agents_md: base.join(Self::AGENTS_MD),
        }
    }

    fn mcp_json(&self) -> &std::path::Path {
        &self.mcp_json
    }
    fn claude_md(&self) -> &std::path::Path {
        &self.claude_md
    }
    fn agents_md(&self) -> &std::path::Path {
        &self.agents_md
    }
}

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Directory to initialize [default: current directory]
    pub path: Option<String>,

    /// Non-interactive mode: say yes to all prompts.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

// ─── Run ────────────────────────────────────────────────────────────────────

pub fn run(args: &Args) -> ExitCode {
    let path_str = args.path.as_deref().unwrap_or(".");
    let root = match ProjectRoot::new(path_str) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", o8v_core::sanitize(&e.to_string()));
            return ExitCode::from(EXIT_FAIL);
        }
    };

    let containment_root = match root.as_containment_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", o8v_core::sanitize(&e.to_string()));
            return ExitCode::from(EXIT_FAIL);
        }
    };

    if !args.yes && !std::io::stdin().is_terminal() {
        eprintln!("error: 8v init requires an interactive terminal");
        return ExitCode::from(EXIT_FAIL);
    }

    // Track what was completed for final summary
    let mut completed: Vec<&str> = Vec::new();

    // Step 1: Detect projects
    let detect_result = detect_all(&root);

    for err in detect_result.errors() {
        // sanitize: err.to_string() may contain file paths with ANSI sequences.
        eprintln!("  warning: {}", o8v_core::sanitize(&err.to_string()));
    }

    let projects = detect_result.projects();
    if projects.is_empty() {
        eprintln!("  No projects detected");
    } else {
        for p in projects {
            let kind = match p.kind() {
                ProjectKind::Standalone => "",
                ProjectKind::Compound { .. } => " (compound)",
                _ => "",
            };
            // sanitize: project name comes from manifest files (package.json, Cargo.toml, …)
            // and may contain ANSI escape sequences from a malicious project.
            let name = o8v_core::sanitize(p.name());
            eprintln!(
                "  {} ({}){}",
                name,
                p.stack().to_string().to_lowercase(),
                kind
            );
        }
    }

    // Step 2: Config location
    let location = if args.yes {
        match WorkspaceDir::local(&root) {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match prompt_config_location(&root) {
            Ok(loc) => loc,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(EXIT_FAIL);
            }
        }
    };

    if let Err(e) = location.create() {
        eprintln!("error: cannot create {}: {e}", location.display());
        return ExitCode::from(EXIT_FAIL);
    }
    eprintln!("✓ Created {}", location.display());

    // Run a baseline check so the first `8v check` has a prior snapshot.
    if let Err(e) = run_baseline_check(&containment_root) {
        tracing::warn!("baseline check failed: {e}");
    }

    // Write default config.toml if it doesn't exist (local config only)
    if !location.is_home() {
        let config_dir = containment_root.as_path().join(".8v");
        let config_toml_path = config_dir.join("config.toml");
        if !config_toml_path.exists() {
            if let Err(e) = fs::write(&config_toml_path, DEFAULT_CONFIG) {
                eprintln!("error: failed to write .8v/config.toml: {e}");
                return ExitCode::from(EXIT_FAIL);
            }
            eprintln!("✓ Created .8v/config.toml");
        }
    }

    if location.is_home() {
        if let Err(e) = register_workspace(&root) {
            eprintln!("error: failed to update workspaces.toml: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Registered workspace in ~/.8v/workspaces.toml");
    }

    // In --yes mode, print what will be done upfront
    if args.yes {
        eprintln!();
        eprintln!("8v init --yes will:");
        eprintln!("  • Register MCP server in .mcp.json");
        eprintln!("  • Add 8v section to CLAUDE.md and AGENTS.md");
        eprintln!("  • Grant mcp__8v__8v permission in .claude/settings.json");
        eprintln!("  • Install git pre-commit hook (runs 8v check on commit)");
        eprintln!("  • Install git commit-msg hook (strips Co-Authored-By)");
        eprintln!();
    }

    let project_root = &containment_root;
    let init_dir = InitDir::new(project_root);

    // Step 3: MCP
    if confirm("Set up MCP?", args.yes) {
        if let Err(e) = setup_mcp_json(init_dir.mcp_json(), project_root) {
            eprintln!("error: failed to setup .mcp.json: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Updated .mcp.json");
        completed.push(".mcp.json — MCP server registered");
    }

    // Step 4: CLAUDE.md
    if confirm("Add 8v to CLAUDE.md?", args.yes) {
        if let Err(e) =
            append_section_if_missing(init_dir.claude_md(), "# 8v\n", AI_SECTION, project_root)
        {
            eprintln!("error: failed to update CLAUDE.md: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Updated CLAUDE.md");
        completed.push("CLAUDE.md — 8v instructions added");
    }

    // Step 5: AGENTS.md
    if confirm("Add 8v to AGENTS.md?", args.yes) {
        if let Err(e) =
            append_section_if_missing(init_dir.agents_md(), "# 8v\n", AI_SECTION, project_root)
        {
            eprintln!("error: failed to update AGENTS.md: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Updated AGENTS.md");
        completed.push("AGENTS.md — 8v instructions added");
    }

    // Step 6: Pre-commit hook
    if confirm("Set up pre-commit hook?", args.yes) {
        if let Err(e) = install_git_pre_commit(project_root) {
            eprintln!("error: failed to setup pre-commit hook: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Pre-commit hook installed");
        completed.push(".git/hooks/pre-commit — runs 8v check before commit");
    }

    // Step 6b: Commit-msg hook
    if confirm(
        "Set up commit-msg hook? (strips Co-Authored-By lines)",
        args.yes,
    ) {
        if let Err(e) = install_git_commit_msg(project_root) {
            eprintln!("error: failed to setup commit-msg hook: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Commit-msg hook installed");
        completed.push(".git/hooks/commit-msg — strips AI attribution");
    }

    // Step 6c: Claude Code tool enforcement hooks
    // Aggressive — only in interactive mode, user must explicitly opt in.
    // With --yes, skip this step.
    if !args.yes
        && confirm(
            "Set up Claude Code tool enforcement hooks? (blocks native Read/Edit/Write/Bash)",
            false,
        )
    {
        if let Err(e) = install_claude_hooks(project_root) {
            eprintln!("error: failed to setup Claude Code hooks: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
        eprintln!("✓ Claude Code hooks installed");
        completed.push(".claude/hooks.json — tool enforcement enabled");
    }

    // Step 7: Claude Code settings (ALWAYS runs, required for MCP tool)
    if let Err(e) = setup_claude_settings(project_root) {
        eprintln!("error: failed to setup .claude/settings.json: {e}");
        return ExitCode::from(EXIT_FAIL);
    }
    eprintln!("✓ Updated .claude/settings.json");
    completed.push(".claude/settings.json — mcp__8v__8v permission granted");

    eprintln!();
    eprintln!("✓ 8v init complete");

    if !completed.is_empty() {
        eprintln!();
        eprintln!("Files modified:");
        for item in &completed {
            eprintln!("  {item}");
        }
    }

    ExitCode::from(EXIT_OK)
}

// ─── Prompts ────────────────────────────────────────────────────────────────

fn confirm(prompt: &str, yes: bool) -> bool {
    if yes {
        true
    } else {
        match Confirm::new().with_prompt(prompt).default(true).interact() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("warning: prompt failed ({e}), skipping");
                false
            }
        }
    }
}

// ─── Baseline ───────────────────────────────────────────────────────────────

/// Run a baseline check during `8v init`.
///
/// The first `8v check` after init will write `last-check.json`. This run
/// establishes the initial snapshot so subsequent checks compute a valid delta.
fn run_baseline_check(containment_root: &o8v_fs::ContainmentRoot) -> Result<(), String> {
    let project_root =
        ProjectRoot::new(containment_root.as_path()).map_err(|e| e.to_string())?;

    let interrupted = Box::leak(Box::new(std::sync::atomic::AtomicBool::new(false)));
    let check_config = o8v_core::CheckConfig {
        timeout: None,
        interrupted,
    };

    let _report = o8v_check::check(&project_root, &check_config, |_| {});

    Ok(())
}

fn prompt_config_location(root: &ProjectRoot) -> Result<WorkspaceDir, String> {
    let items = &[format!("Local — {root}/.8v/"), "Home  — ~/.8v/".to_string()];

    let selection = Select::new()
        .with_prompt("Where should 8v store config?")
        .items(items)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    match selection {
        0 => WorkspaceDir::local(root).map_err(|e| e.to_string()),
        _ => WorkspaceDir::home().map_err(|e| e.to_string()),
    }
}
