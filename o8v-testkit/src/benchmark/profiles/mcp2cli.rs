use std::path::Path;

use crate::benchmark::types::Agent;

use super::{ProfileArtifacts, ToolProfileHarness};

const MCP2CLI_PREPEND: &str = include_str!("assets/mcp2cli_prepend.md");

pub struct Mcp2cliProfile;

impl ToolProfileHarness for Mcp2cliProfile {
    fn id(&self) -> &'static str {
        "mcp2cli"
    }

    fn version(&self) -> String {
        "3.0.2".to_string()
    }

    fn setup(&self, _workspace: &Path, _agent: Agent) -> anyhow::Result<ProfileArtifacts> {
        Ok(ProfileArtifacts {
            mcp_json_fragment: None,
            claude_md_prepend: Some(MCP2CLI_PREPEND.to_string()),
            env: Default::default(),
        })
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
