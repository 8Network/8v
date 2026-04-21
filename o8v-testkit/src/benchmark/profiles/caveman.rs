use std::path::Path;

use crate::benchmark::types::Agent;

use super::{ProfileArtifacts, ToolProfileHarness};

const CAVEMAN_SKILL: &str = include_str!("assets/caveman_skill.md");

pub struct CavemanProfile;

impl ToolProfileHarness for CavemanProfile {
    fn id(&self) -> &'static str {
        "caveman"
    }

    fn version(&self) -> String {
        "upstream-2026-04-18".to_string()
    }

    fn setup(&self, _workspace: &Path, _agent: Agent) -> anyhow::Result<ProfileArtifacts> {
        Ok(ProfileArtifacts {
            mcp_json_fragment: None,
            claude_md_prepend: Some(CAVEMAN_SKILL.to_string()),
            env: Default::default(),
        })
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
