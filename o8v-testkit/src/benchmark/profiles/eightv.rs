use std::path::Path;

use serde_json::json;

use crate::benchmark::types::Agent;

use super::{ProfileArtifacts, ToolProfileHarness};

/// Resolved at runtime so no personal paths are baked into source.
fn eightv_binary() -> &'static str {
    option_env!("EIGHTV_BINARY").unwrap_or("8v")
}

pub struct EightVProfile;

impl ToolProfileHarness for EightVProfile {
    fn id(&self) -> &'static str {
        "8v"
    }

    fn version(&self) -> String {
        let root = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(v) => v,
            Err(_) => ".".to_string(),
        };
        let output = std::process::Command::new("git")
            .args(["-C", &root, "rev-parse", "--short", "HEAD"])
            .output();

        match output {
            Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
                Ok(s) => s.trim().to_string(),
                Err(e) => {
                    eprintln!("8v profile: git SHA not utf-8: {e}");
                    "unknown".to_string()
                }
            },
            Ok(o) => {
                eprintln!(
                    "8v profile: git rev-parse exited {}: {}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr)
                );
                "unknown".to_string()
            }
            Err(e) => {
                eprintln!("8v profile: failed to spawn git: {e}");
                "unknown".to_string()
            }
        }
    }

    fn setup(&self, _workspace: &Path, _agent: Agent) -> anyhow::Result<ProfileArtifacts> {
        let fragment = json!({
            "mcpServers": {
                "8v": {
                    "command": eightv_binary(),
                    "args": ["mcp"]
                }
            }
        });

        Ok(ProfileArtifacts {
            mcp_json_fragment: Some(fragment),
            claude_md_prepend: None,
            env: Default::default(),
        })
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
