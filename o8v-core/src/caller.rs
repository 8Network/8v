// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Caller identity — who invoked the command.

use serde::Serialize;

/// Who invoked the command. Determines default audience and is recorded in events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Caller {
    /// Command-line interface.
    Cli,
    /// MCP server (AI agent).
    Mcp,
}

impl Caller {
    /// The default audience for this caller.
    ///
    /// CLI defaults to Human (overridable by --json/--plain flags).
    /// MCP is always Agent.
    pub fn default_audience(self) -> crate::render::Audience {
        match self {
            Caller::Cli => crate::render::Audience::Human,
            Caller::Mcp => crate::render::Audience::Agent,
        }
    }

    /// String representation for event serialization.
    pub fn as_str(self) -> &'static str {
        match self {
            Caller::Cli => "cli",
            Caller::Mcp => "mcp",
        }
    }
}

impl std::fmt::Display for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
