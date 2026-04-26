use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::benchmark::types::Agent;

pub mod eightv;
pub mod native;

#[derive(Clone, Debug, Default)]
pub struct ProfileArtifacts {
    pub mcp_json_fragment: Option<Value>,
    pub claude_md_prepend: Option<String>,
    pub env: HashMap<String, String>,
}

/// Default profile version for backward-compatible deserialization.
pub fn default_profile_version() -> String {
    "pre-2026-04".to_string()
}

pub trait ToolProfileHarness: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> String;
    fn setup(&self, workspace: &Path, agent: Agent) -> anyhow::Result<ProfileArtifacts>;
    fn cleanup(&self, workspace: &Path) -> anyhow::Result<()>;
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ToolProfile {
    #[default]
    Native,
    EightV,
}

impl ToolProfile {
    pub fn harness(&self) -> Box<dyn ToolProfileHarness> {
        match self {
            ToolProfile::Native => Box::new(native::NativeProfile),
            ToolProfile::EightV => Box::new(eightv::EightVProfile),
        }
    }
}
