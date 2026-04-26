// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Caller identity — who invoked the command.

use serde::{Deserialize, Serialize};

/// Identity of the MCP client (AI agent) extracted from the MCP initialize handshake.
///
/// The MCP protocol sends `InitializeRequestParams` during connection setup.
/// We capture everything the client declares: implementation name+version,
/// MCP protocol version, and declared capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub capabilities: Vec<String>,
}

/// Who invoked the command. Determines default audience and is recorded in events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Caller {
    /// Command-line interface.
    Cli,
    /// MCP server (AI agent).
    Mcp,
    /// Claude hook (PreToolUse / PostToolUse native tool capture).
    Hook,
}

impl Caller {
    /// String representation for event serialization.
    pub fn as_str(self) -> &'static str {
        match self {
            Caller::Cli => "cli",
            Caller::Mcp => "mcp",
            Caller::Hook => "hook",
        }
    }
}

impl std::fmt::Display for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Caller {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cli" => Ok(Caller::Cli),
            "mcp" => Ok(Caller::Mcp),
            "hook" => Ok(Caller::Hook),
            other => Err(format!("unknown caller: {other:?}")),
        }
    }
}
