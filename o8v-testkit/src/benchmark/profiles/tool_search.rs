use std::path::Path;

use crate::benchmark::types::Agent;

use super::{ProfileArtifacts, ToolProfileHarness};

pub struct ToolSearchProfile;

impl ToolProfileHarness for ToolSearchProfile {
    fn id(&self) -> &'static str {
        "tool-search"
    }

    fn version(&self) -> String {
        "anthropic-builtin".to_string()
    }

    fn setup(&self, _workspace: &Path, _agent: Agent) -> anyhow::Result<ProfileArtifacts> {
        let mut env = std::collections::HashMap::new();
        env.insert("ENABLE_TOOL_SEARCH".to_string(), "true".to_string());
        Ok(ProfileArtifacts {
            mcp_json_fragment: None,
            claude_md_prepend: None,
            env,
        })
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
