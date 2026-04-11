// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Claude Code settings.json — permissions for 8v MCP tool.
//!
//! 8v replaces native tools (Read, Edit, Write, Bash, Grep, Glob) with a
//! single MCP tool. This module sets up permissions.allow for the 8v tool
//! and permissions.deny for the native tools it replaces.

use o8v_fs::FsConfig;
use o8v_workspace::to_io;
use serde::{Deserialize, Serialize};

const MCP_TOOL_PERMISSION: &str = "mcp__8v__8v";

/// Native tools that 8v replaces. These are denied so the agent uses 8v instead.
/// Without this, the agent will always prefer Bash over MCP — making 8v useless.
const DENIED_NATIVE_TOOLS: &[&str] = &["Read", "Edit", "Write", "Bash", "Grep", "Glob"];

/// Claude Code settings.json structure.
///
/// `extra` captures all unknown keys so we don't destroy fields we don't own.
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    permissions: Permissions,

    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Permissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allow: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deny: Vec<String>,
}

/// Ensure `.claude/settings.json` grants 8v MCP tool and denies native tools.
///
/// Creates `.claude/` directory if missing. Creates `settings.json` if missing.
/// If the file already exists, merges permissions without overwriting existing ones.
///
/// Sets:
/// - `permissions.allow`: `["mcp__8v__8v"]`
/// - `permissions.deny`: `["Read", "Edit", "Write", "Bash", "Grep", "Glob"]`
pub(super) fn setup_claude_settings(root: &o8v_fs::ContainmentRoot) -> std::io::Result<()> {
    let claude_dir = root.as_path().join(".claude");
    let settings_path = claude_dir.join("settings.json");

    o8v_fs::safe_create_dir(&claude_dir, root).map_err(to_io)?;

    match o8v_fs::safe_exists(&settings_path, root) {
        Err(e) => return Err(to_io(e)),
        Ok(true) => {
            let existing =
                o8v_fs::safe_read(&settings_path, root, &FsConfig::default()).map_err(to_io)?;
            let existing = existing.content();

            if existing.trim().is_empty() {
                let settings = create_default_settings();
                write_settings(&settings_path, root, &settings)?;
                return Ok(());
            }

            match serde_json::from_str::<ClaudeSettings>(existing) {
                Ok(mut settings) => {
                    merge_permissions(&mut settings);
                    write_settings(&settings_path, root, &settings)?;
                }
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("existing .claude/settings.json is not valid JSON: {}", e),
                    ));
                }
            }
        }
        Ok(false) => {
            let settings = create_default_settings();
            write_settings(&settings_path, root, &settings)?;
        }
    }

    Ok(())
}

/// Create default settings with 8v permissions.
fn create_default_settings() -> ClaudeSettings {
    let mut settings = ClaudeSettings {
        permissions: Permissions::default(),
        extra: serde_json::Map::new(),
    };
    merge_permissions(&mut settings);
    settings
}

/// Add 8v allow + native deny entries if not already present.
fn merge_permissions(settings: &mut ClaudeSettings) {
    if !settings
        .permissions
        .allow
        .contains(&MCP_TOOL_PERMISSION.to_string())
    {
        settings
            .permissions
            .allow
            .push(MCP_TOOL_PERMISSION.to_string());
    }

    for tool in DENIED_NATIVE_TOOLS {
        let tool = tool.to_string();
        if !settings.permissions.deny.contains(&tool) {
            settings.permissions.deny.push(tool);
        }
    }
}

/// Write settings to the path with pretty formatting.
fn write_settings(
    settings_path: &std::path::Path,
    root: &o8v_fs::ContainmentRoot,
    settings: &ClaudeSettings,
) -> std::io::Result<()> {
    let content = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    let bytes = (content + "\n").into_bytes();
    o8v_fs::safe_write(settings_path, root, &bytes).map_err(to_io)?;
    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn canonical(dir: &TempDir) -> PathBuf {
        std::fs::canonicalize(dir.path()).unwrap()
    }

    #[test]
    fn creates_new_settings_when_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == MCP_TOOL_PERMISSION));
        // Must deny native tools 8v replaces
        for tool in DENIED_NATIVE_TOOLS {
            assert!(
                settings.permissions.deny.iter().any(|v| v == *tool),
                "missing deny for {tool}"
            );
        }
    }

    #[test]
    fn adds_permission_to_existing_settings() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["other__tool"]}}"#,
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == "other__tool"));
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == MCP_TOOL_PERMISSION));
        for tool in DENIED_NATIVE_TOOLS {
            assert!(
                settings.permissions.deny.iter().any(|v| v == *tool),
                "missing deny for {tool}"
            );
        }
    }

    #[test]
    fn preserves_existing_permissions() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["tool_a", "tool_b"]}}"#,
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert!(settings.permissions.allow.iter().any(|v| v == "tool_a"));
        assert!(settings.permissions.allow.iter().any(|v| v == "tool_b"));
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == MCP_TOOL_PERMISSION));
    }

    #[test]
    fn skips_when_already_present() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            format!(
                r#"{{"permissions": {{"allow": ["{}", "other__tool"]}}}}"#,
                MCP_TOOL_PERMISSION
            ),
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        let count = settings
            .permissions
            .allow
            .iter()
            .filter(|v| v.as_str() == MCP_TOOL_PERMISSION)
            .count();
        assert_eq!(count, 1, "permission must not be duplicated");
    }

    #[test]
    fn creates_permissions_key_when_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/settings.json"), r#"{"otherKey": true}"#).unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(
            settings.extra.get("otherKey").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == MCP_TOOL_PERMISSION));
    }

    #[test]
    fn handles_empty_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/settings.json"), "").unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert!(settings
            .permissions
            .allow
            .iter()
            .any(|v| v == MCP_TOOL_PERMISSION));
    }

    #[test]
    fn preserves_unknown_keys() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::create_dir(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"otherKey": true, "permissions": {"allow": ["other__tool"]}}"#,
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_claude_settings(&containment_root).unwrap();

        let content = fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let settings: ClaudeSettings = serde_json::from_str(&content).unwrap();
        assert_eq!(
            settings.extra.get("otherKey").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}
