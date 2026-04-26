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

        let prepend = "# 8v tool

`mcp__8v__8v` is a deferred MCP tool. ON YOUR FIRST TURN, before any other tool call, run `ToolSearch` with query `select:mcp__8v__8v` to load it. Then use it for ALL file operations.

Native Read, Edit, Write, Glob, Grep are blocked in this project — use `mcp__8v__8v` instead. Bash is allowed for git, processes, and environment.

Cheatsheet (every command accepts a single `command` string):
- `8v ls --tree` — list files
- `8v read <path>` — symbol map; add `:start-end` for a line range, `--full` for whole file
- `8v read a b c` — batch multiple files in one call
- `8v search <regex> [path]` — content search
- `8v write <path>:<line> \"content\"` — replace line; also `--insert`, `--delete`, `--find/--replace`, `--append`
- `8v test .` / `8v check .` / `8v build .` / `8v fmt .` — verify
"
        .to_string();

        Ok(ProfileArtifacts {
            mcp_json_fragment: Some(fragment),
            claude_md_prepend: Some(prepend),
            env: Default::default(),
        })
    }

    fn cleanup(&self, _workspace: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
