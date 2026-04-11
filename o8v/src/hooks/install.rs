// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Hook installation — writes scripts into .git/hooks/ and entries into .claude/settings.json.

use dialoguer::Select;
use o8v_fs::FsConfig;
use o8v_workspace::to_io;
use serde::{Deserialize, Serialize};

// ─── Git hook constants ───────────────────────────────────────────────────────

const HOOK_LINE: &str = "8v hooks git on-commit";
const HOOK_TEMPLATE: &str = "#!/bin/sh\n8v hooks git on-commit\n";
const COMMIT_MSG_HOOK_LINE: &str = "8v hooks git on-commit-msg \"$1\"";
const COMMIT_MSG_HOOK_TEMPLATE: &str = "#!/bin/sh\n8v hooks git on-commit-msg \"$1\"\n";

// ─── Claude hook constants ────────────────────────────────────────────────────

const MCP_TOOL_PERMISSION: &str = "mcp__8v__8v";

/// All 8 Claude Code hook events and the 8v command they map to.
const CLAUDE_HOOK_EVENTS: &[(&str, &str)] = &[
    ("PreToolUse", "8v hooks claude pre-tool-use"),
    ("PostToolUse", "8v hooks claude post-tool-use"),
    (
        "PostToolUseFailure",
        "8v hooks claude post-tool-use-failure",
    ),
    ("UserPromptSubmit", "8v hooks claude user-prompt-submit"),
    ("SessionStart", "8v hooks claude session-start"),
    ("Stop", "8v hooks claude stop"),
    ("SubagentStart", "8v hooks claude subagent-start"),
    ("SubagentStop", "8v hooks claude subagent-stop"),
];

// ─── Typed structs for .claude/settings.json ─────────────────────────────────

/// Claude Code settings.json — only the fields we manage.
/// Unknown fields are preserved via `extra`.
#[derive(Debug, Serialize, Deserialize, Default)]
struct ClaudeSettings {
    #[serde(default)]
    permissions: Permissions,
    #[serde(default)]
    hooks: HookEvents,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Permissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deny: Vec<String>,
}

/// All Claude Code hook events we install.
///
/// Each field uses an explicit `#[serde(rename)]` to match the exact PascalCase
/// keys Claude Code expects. This avoids surprises from `rename_all` transformations.
#[derive(Debug, Serialize, Deserialize, Default)]
struct HookEvents {
    #[serde(rename = "PreToolUse", default, skip_serializing_if = "Vec::is_empty")]
    pre_tool_use: Vec<HookEntry>,
    #[serde(rename = "PostToolUse", default, skip_serializing_if = "Vec::is_empty")]
    post_tool_use: Vec<HookEntry>,
    #[serde(
        rename = "PostToolUseFailure",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    post_tool_use_failure: Vec<HookEntry>,
    #[serde(
        rename = "UserPromptSubmit",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    user_prompt_submit: Vec<HookEntry>,
    #[serde(
        rename = "SessionStart",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    session_start: Vec<HookEntry>,
    #[serde(rename = "Stop", default, skip_serializing_if = "Vec::is_empty")]
    stop: Vec<HookEntry>,
    #[serde(
        rename = "SubagentStart",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    subagent_start: Vec<HookEntry>,
    #[serde(
        rename = "SubagentStop",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    subagent_stop: Vec<HookEntry>,
    /// Preserve unknown hook events that we don't explicitly manage.
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookEntry {
    matcher: String,
    hooks: Vec<Hook>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Hook {
    #[serde(rename = "type")]
    hook_type: String,
    command: String,
}

// ─── GitDir — path value object for .git/ ────────────────────────────────────

struct GitDir {
    hooks_dir: std::path::PathBuf,
    pre_commit: std::path::PathBuf,
    commit_msg: std::path::PathBuf,
}

impl GitDir {
    const GIT: &'static str = ".git";
    const HOOKS: &'static str = "hooks";
    const PRE_COMMIT: &'static str = "pre-commit";
    const COMMIT_MSG: &'static str = "commit-msg";

    fn open(root: &o8v_fs::ContainmentRoot) -> std::io::Result<Option<Self>> {
        let git = root.as_path().join(Self::GIT);
        match o8v_fs::safe_exists(&git, root) {
            Ok(false) | Err(_) => {
                // .git doesn't exist or can't be verified — return Ok(None) (not an error)
                return Ok(None);
            }
            Ok(true) => {}
        }
        let hooks_dir = git.join(Self::HOOKS);
        let pre_commit = hooks_dir.join(Self::PRE_COMMIT);
        let commit_msg = hooks_dir.join(Self::COMMIT_MSG);
        Ok(Some(Self {
            hooks_dir,
            pre_commit,
            commit_msg,
        }))
    }

    fn hooks_dir(&self) -> &std::path::Path {
        &self.hooks_dir
    }
    fn pre_commit(&self) -> &std::path::Path {
        &self.pre_commit
    }
    fn commit_msg(&self) -> &std::path::Path {
        &self.commit_msg
    }
}

// ─── Git hook installation ────────────────────────────────────────────────────

pub fn install_git_pre_commit(root: &o8v_fs::ContainmentRoot) -> std::io::Result<()> {
    let git_dir = match GitDir::open(root)? {
        Some(d) => d,
        None => return Ok(()), // .git doesn't exist; skip installation gracefully
    };

    o8v_fs::safe_create_dir(git_dir.hooks_dir(), root).map_err(to_io)?;

    match o8v_fs::safe_exists(git_dir.pre_commit(), root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(git_dir.pre_commit(), root, &FsConfig::default())
                .map_err(to_io)?;
            let existing = guarded.content();
            if existing.contains("8v hooks git on-commit") {
                eprintln!("  (hook already contains 8v)");
                return Ok(());
            }

            let items = &["Before existing hook", "After existing hook", "Skip"];
            let selection = Select::new()
                .with_prompt("Pre-commit hook already exists. Add 8v check?")
                .items(items)
                .default(0)
                .interact()
                .map_err(std::io::Error::other)?;

            match selection {
                0 => {
                    let new = format!("{HOOK_LINE}\n{existing}");
                    o8v_fs::safe_write(git_dir.pre_commit(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                1 => {
                    let new = format!("{existing}\n{HOOK_LINE}\n");
                    o8v_fs::safe_write(git_dir.pre_commit(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                _ => {
                    eprintln!("  → Pre-commit hook skipped");
                    return Ok(());
                }
            }
        }
        Ok(false) => {
            o8v_fs::safe_write(git_dir.pre_commit(), root, HOOK_TEMPLATE.as_bytes())
                .map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }

    #[cfg(unix)]
    o8v_fs::safe_set_permissions(git_dir.pre_commit(), root, 0o755).map_err(to_io)?;

    Ok(())
}

pub fn install_git_commit_msg(root: &o8v_fs::ContainmentRoot) -> std::io::Result<()> {
    let git_dir = match GitDir::open(root)? {
        Some(d) => d,
        None => return Ok(()), // .git doesn't exist; skip installation gracefully
    };

    o8v_fs::safe_create_dir(git_dir.hooks_dir(), root).map_err(to_io)?;

    match o8v_fs::safe_exists(git_dir.commit_msg(), root) {
        Ok(true) => {
            let guarded = o8v_fs::safe_read(git_dir.commit_msg(), root, &FsConfig::default())
                .map_err(to_io)?;
            let existing = guarded.content();
            if existing.contains(COMMIT_MSG_HOOK_LINE) {
                eprintln!("  (hook already contains 8v)");
                return Ok(());
            }

            let items = &["Before existing hook", "After existing hook", "Skip"];
            let selection = Select::new()
                .with_prompt("Commit-msg hook already exists. Add 8v commit-msg handler?")
                .items(items)
                .default(0)
                .interact()
                .map_err(std::io::Error::other)?;

            match selection {
                0 => {
                    let new = format!("{COMMIT_MSG_HOOK_LINE}\n{existing}");
                    o8v_fs::safe_write(git_dir.commit_msg(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                1 => {
                    let new = format!("{existing}\n{COMMIT_MSG_HOOK_LINE}\n");
                    o8v_fs::safe_write(git_dir.commit_msg(), root, new.as_bytes())
                        .map_err(to_io)?;
                }
                _ => {
                    eprintln!("  → Commit-msg hook skipped");
                    return Ok(());
                }
            }
        }
        Ok(false) => {
            o8v_fs::safe_write(
                git_dir.commit_msg(),
                root,
                COMMIT_MSG_HOOK_TEMPLATE.as_bytes(),
            )
            .map_err(to_io)?;
        }
        Err(e) => return Err(to_io(e)),
    }

    #[cfg(unix)]
    o8v_fs::safe_set_permissions(git_dir.commit_msg(), root, 0o755).map_err(to_io)?;

    Ok(())
}

// ─── Claude hook installation ─────────────────────────────────────────────────

/// Returns the matcher string for a given hook event name.
///
/// Tool events (`PreToolUse`, `PostToolUse`, `PostToolUseFailure`) require `".*"`
/// to match any tool. All other events use `""` (no matcher field needed by Claude Code).
fn matcher_for(event: &str) -> &'static str {
    match event {
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => ".*",
        _ => "",
    }
}

/// Returns a mutable reference to the Vec<HookEntry> for the named event.
fn event_entries_mut<'a>(hooks: &'a mut HookEvents, event: &str) -> Option<&'a mut Vec<HookEntry>> {
    match event {
        "PreToolUse" => Some(&mut hooks.pre_tool_use),
        "PostToolUse" => Some(&mut hooks.post_tool_use),
        "PostToolUseFailure" => Some(&mut hooks.post_tool_use_failure),
        "UserPromptSubmit" => Some(&mut hooks.user_prompt_submit),
        "SessionStart" => Some(&mut hooks.session_start),
        "Stop" => Some(&mut hooks.stop),
        "SubagentStart" => Some(&mut hooks.subagent_start),
        "SubagentStop" => Some(&mut hooks.subagent_stop),
        _ => None,
    }
}

/// Install all 8 Claude Code hook events into `.claude/settings.json`.
///
/// Merges with existing settings — never overwrites user content.
/// Idempotent: running twice does not duplicate entries.
pub fn install_claude_hooks(root: &o8v_fs::ContainmentRoot) -> std::io::Result<()> {
    let claude_dir = root.as_path().join(".claude");
    let settings_path = claude_dir.join("settings.json");

    o8v_fs::safe_create_dir(&claude_dir, root).map_err(to_io)?;

    let mut settings: ClaudeSettings = match o8v_fs::safe_exists(&settings_path, root) {
        Err(e) => return Err(to_io(e)),
        Ok(true) => {
            let guarded =
                o8v_fs::safe_read(&settings_path, root, &FsConfig::default()).map_err(to_io)?;
            let content = guarded.content();
            if content.trim().is_empty() {
                ClaudeSettings::default()
            } else {
                serde_json::from_str(content).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("existing .claude/settings.json is not valid JSON: {e}"),
                    )
                })?
            }
        }
        Ok(false) => ClaudeSettings::default(),
    };

    // For each hook event, add our entry if not already present.
    for (event, command) in CLAUDE_HOOK_EVENTS {
        let entries = event_entries_mut(&mut settings.hooks, event)
            .ok_or_else(|| std::io::Error::other(format!("unrecognized hook event: {event}")))?;

        let already_present = entries.iter().any(|entry| {
            entry
                .hooks
                .iter()
                .any(|h| h.command.starts_with("8v hooks claude"))
        });

        if !already_present {
            entries.push(HookEntry {
                matcher: matcher_for(event).to_string(),
                hooks: vec![Hook {
                    hook_type: "command".to_string(),
                    command: command.to_string(),
                }],
            });
        }
    }

    // Ensure MCP permission is present.
    if !settings
        .permissions
        .allow
        .iter()
        .any(|v| v == MCP_TOOL_PERMISSION)
    {
        settings
            .permissions
            .allow
            .push(MCP_TOOL_PERMISSION.to_string());
    }

    let content = serde_json::to_string_pretty(&settings).map_err(std::io::Error::other)?;
    let bytes = (content + "\n").into_bytes();
    o8v_fs::safe_write(&settings_path, root, &bytes).map_err(to_io)?;

    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> PathBuf {
        std::fs::canonicalize(dir.path()).unwrap()
    }

    // ── Git pre-commit ──────────────────────────────────────────────────────

    #[test]
    fn pre_commit_hook_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/pre-commit")).unwrap();
        assert_eq!(content, HOOK_TEMPLATE);
    }

    #[test]
    #[cfg(unix)]
    fn pre_commit_hook_is_executable() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(root.join(".git/hooks/pre-commit"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "hook must be executable");
    }

    #[test]
    fn pre_commit_hook_succeeds_without_git_dir() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = install_git_pre_commit(&containment_root);
        // Missing .git is not an error; installation is gracefully skipped
        assert!(result.is_ok());
    }

    #[test]
    fn pre_commit_hook_idempotent_when_8v_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let hooks_dir = root.join(".git/hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let original = "#!/bin/sh\n8v hooks git on-commit\necho other\n";
        fs::write(hooks_dir.join("pre-commit"), original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        let content = fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn pre_commit_hook_creates_hooks_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_pre_commit(&containment_root).unwrap();

        assert!(root.join(".git/hooks/pre-commit").exists());
    }

    // ── Git commit-msg ──────────────────────────────────────────────────────

    #[test]
    fn commit_msg_hook_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".git/hooks/commit-msg")).unwrap();
        assert_eq!(content, COMMIT_MSG_HOOK_TEMPLATE);
    }

    #[test]
    #[cfg(unix)]
    fn commit_msg_hook_is_executable() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(root.join(".git/hooks/commit-msg"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "hook must be executable");
    }

    #[test]
    fn commit_msg_hook_succeeds_without_git_dir() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = install_git_commit_msg(&containment_root);
        // Missing .git is not an error; installation is gracefully skipped
        assert!(result.is_ok());
    }

    #[test]
    fn commit_msg_hook_idempotent_when_8v_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let hooks_dir = root.join(".git/hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let original = "#!/bin/sh\n8v hooks git on-commit-msg \"$1\"\necho other\n";
        fs::write(hooks_dir.join("commit-msg"), original).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        let content = fs::read_to_string(hooks_dir.join("commit-msg")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn commit_msg_hook_creates_hooks_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".git")).unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_git_commit_msg(&containment_root).unwrap();

        assert!(root.join(".git/hooks/commit-msg").exists());
    }

    // ── Claude hooks ────────────────────────────────────────────────────────

    #[test]
    fn claude_hooks_creates_settings_when_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_claude_hooks(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let hooks = v["hooks"].as_object().unwrap();
        assert!(hooks.contains_key("PreToolUse"));
        assert!(hooks.contains_key("PostToolUse"));
        assert!(hooks.contains_key("Stop"));
        assert_eq!(hooks.len(), CLAUDE_HOOK_EVENTS.len());
    }

    #[test]
    fn claude_hooks_adds_mcp_permission() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_claude_hooks(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let allow = v["permissions"]["allow"].as_array().unwrap();
        assert!(allow
            .iter()
            .any(|a| a.as_str() == Some(MCP_TOOL_PERMISSION)));
    }

    #[test]
    fn claude_hooks_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_claude_hooks(&containment_root).unwrap();
        install_claude_hooks(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let hooks = v["hooks"].as_object().unwrap();
        // Each event should have exactly one entry
        for (event, _) in CLAUDE_HOOK_EVENTS {
            let entries = hooks[*event].as_array().unwrap();
            assert_eq!(
                entries.len(),
                1,
                "event {event} must have exactly 1 entry after 2 installs"
            );
        }
    }

    #[test]
    fn claude_hooks_merges_with_existing_settings() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["other__tool"]}, "otherKey": true}"#,
        )
        .unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_claude_hooks(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        // Existing keys preserved
        assert_eq!(v["otherKey"].as_bool(), Some(true));
        let allow = v["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|a| a.as_str() == Some("other__tool")));
        assert!(allow
            .iter()
            .any(|a| a.as_str() == Some(MCP_TOOL_PERMISSION)));
        // All hook events present
        let hooks = v["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), CLAUDE_HOOK_EVENTS.len());
    }

    #[test]
    fn claude_hooks_preserves_user_pre_tool_use_hooks() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"hooks": {"PreToolUse": [{"matcher": "UserHook", "hooks": [{"type": "command", "command": "echo user"}]}]}}"#,
        )
        .unwrap();
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();

        install_claude_hooks(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let pre_tool_use = v["hooks"]["PreToolUse"].as_array().unwrap();
        // Should have both user's hook and our hook
        assert_eq!(pre_tool_use.len(), 2);
        assert!(pre_tool_use
            .iter()
            .any(|e| e["matcher"].as_str() == Some("UserHook")));
        assert!(pre_tool_use.iter().any(|e| {
            e["hooks"]
                .as_array()
                .map(|h| {
                    h.iter()
                        .any(|h| h["command"].as_str() == Some("8v hooks claude pre-tool-use"))
                })
                .unwrap_or(false)
        }));
    }
}
