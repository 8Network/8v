use std::path::Path;

use crate::benchmark::types::Agent;

use super::{ProfileArtifacts, ToolProfileHarness};

pub struct NativeProfile;

impl ToolProfileHarness for NativeProfile {
    fn id(&self) -> &'static str {
        "native"
    }

    fn version(&self) -> String {
        "native-baseline".to_string()
    }

    fn setup(&self, _workspace: &Path, _agent: Agent) -> anyhow::Result<ProfileArtifacts> {
        Ok(ProfileArtifacts::default())
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
