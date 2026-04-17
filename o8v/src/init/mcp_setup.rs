// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v::workspace::to_io;
use o8v_fs::FsConfig;

/// Write `.mcp.json` registering the 8v server, optionally with a custom
/// command path.
///
/// `command` is the executable path written into the `"command"` field.
/// In production this is `"8v"` (found on PATH). In tests it is the absolute
/// path of the built test binary so the spawned MCP server is the same code
/// under test.
pub(super) fn setup_mcp_json(
    mcp_path: &std::path::Path,
    root: &o8v_fs::ContainmentRoot,
    command: &str,
) -> std::io::Result<()> {
    match o8v_fs::safe_exists(mcp_path, root) {
        Err(e) => return Err(to_io(e)),
        Ok(true) => {
            let existing =
                o8v_fs::safe_read(mcp_path, root, &FsConfig::default()).map_err(to_io)?;
            let existing = existing.content();
            if existing.trim().is_empty() {
                o8v_fs::safe_write(mcp_path, root, mcp_template(command).as_bytes())
                    .map_err(to_io)?;
                return Ok(());
            }
            match serde_json::from_str::<serde_json::Value>(existing) {
                Ok(mut doc) => {
                    if let Some(obj) = doc.as_object_mut() {
                        let servers = obj
                            .entry("mcpServers")
                            .or_insert_with(|| serde_json::json!({}));
                        if let Some(servers) = servers.as_object_mut() {
                            if !servers.contains_key("8v") {
                                servers.insert(
                                    "8v".to_string(),
                                    serde_json::json!({
                                        "command": command,
                                        "args": ["mcp"]
                                    }),
                                );
                            }
                        } else {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "mcpServers is not a JSON object in .mcp.json",
                            ));
                        }
                    }
                    let content =
                        serde_json::to_string_pretty(&doc).map_err(std::io::Error::other)?;
                    o8v_fs::safe_write(mcp_path, root, (content + "\n").as_bytes())
                        .map_err(to_io)?;
                }
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "existing .mcp.json is not valid JSON",
                    ));
                }
            }
        }
        Ok(false) => {
            o8v_fs::safe_write(mcp_path, root, mcp_template(command).as_bytes()).map_err(to_io)?;
        }
    }

    Ok(())
}

fn mcp_template(command: &str) -> String {
    let doc = serde_json::json!({
        "mcpServers": {
            "8v": {
                "command": command,
                "args": ["mcp"],
            },
        },
    });
    serde_json::to_string_pretty(&doc).expect("serialize .mcp.json") + "\n"
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
    fn mcp_json_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(root.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(doc["mcpServers"]["8v"]["command"].as_str(), Some("8v"));
    }

    #[test]
    fn mcp_json_merges_with_existing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let mcp_path = root.join(".mcp.json");
        fs::write(
            &mcp_path,
            r#"{"mcpServers": {"other": {"command": "other", "args": []}}}"#,
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(&mcp_path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            doc["mcpServers"]["other"]["command"].as_str(),
            Some("other")
        );
        assert_eq!(doc["mcpServers"]["8v"]["command"].as_str(), Some("8v"));
    }

    #[test]
    fn mcp_json_preserves_existing_8v_config() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        let mcp_path = root.join(".mcp.json");
        fs::write(
            &mcp_path,
            r#"{"mcpServers": {"8v": {"command": "custom-8v", "args": ["--custom"]}}}"#,
        )
        .unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(&mcp_path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            doc["mcpServers"]["8v"]["command"].as_str(),
            Some("custom-8v")
        );
    }

    #[test]
    fn mcp_json_handles_empty_file() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::write(root.join(".mcp.json"), "").unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(root.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(doc["mcpServers"]["8v"]["command"].as_str(), Some("8v"));
    }

    #[test]
    fn mcp_json_rejects_invalid_json() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::write(root.join(".mcp.json"), "not json").unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn mcp_json_adds_mcp_servers_key_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::write(root.join(".mcp.json"), r#"{"otherKey": true}"#).unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(root.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(doc["otherKey"].as_bool(), Some(true));
        assert_eq!(doc["mcpServers"]["8v"]["command"].as_str(), Some("8v"));
    }

    #[test]
    fn mcp_json_whitespace_only_treated_as_empty() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::write(root.join(".mcp.json"), "   \n\t  \n").unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v").unwrap();

        let content = fs::read_to_string(root.join(".mcp.json")).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(doc["mcpServers"]["8v"]["command"].as_str(), Some("8v"));
    }

    #[test]
    fn mcp_json_errors_on_null_mcp_servers() {
        let dir = TempDir::new().unwrap();
        let root = canonical(&dir);
        fs::write(root.join(".mcp.json"), r#"{"mcpServers": null}"#).unwrap();

        let containment_root = o8v_fs::ContainmentRoot::new(&root).unwrap();
        let result = setup_mcp_json(&root.join(".mcp.json"), &containment_root, "8v");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }
}
