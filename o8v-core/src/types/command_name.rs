// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `CommandName` — closed enumeration of every user-visible 8v subcommand.
//!
//! Using a typed enum instead of raw strings in aggregation/rendering
//! prevents typos (`"write"` vs `"wrtie"`), eliminates silent string
//! comparisons, and forces exhaustiveness when new commands are added.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandName {
    Read,
    Write,
    Search,
    Check,
    Fmt,
    Ls,
    Build,
    Test,
    Init,
    Hooks,
    Upgrade,
    Mcp,
    Log,
    Stats,
}

impl CommandName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Search => "search",
            Self::Check => "check",
            Self::Fmt => "fmt",
            Self::Ls => "ls",
            Self::Build => "build",
            Self::Test => "test",
            Self::Init => "init",
            Self::Hooks => "hooks",
            Self::Upgrade => "upgrade",
            Self::Mcp => "mcp",
            Self::Log => "log",
            Self::Stats => "stats",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown command name: {0}")]
pub struct ParseCommandNameError(pub String);

impl FromStr for CommandName {
    type Err = ParseCommandNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "read" => Self::Read,
            "write" => Self::Write,
            "search" => Self::Search,
            "check" => Self::Check,
            "fmt" => Self::Fmt,
            "ls" => Self::Ls,
            "build" => Self::Build,
            "test" => Self::Test,
            "init" => Self::Init,
            "hooks" => Self::Hooks,
            "upgrade" => Self::Upgrade,
            "mcp" => Self::Mcp,
            "log" => Self::Log,
            "stats" => Self::Stats,
            other => return Err(ParseCommandNameError(other.to_string())),
        })
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() {
        for &c in &[
            CommandName::Read,
            CommandName::Write,
            CommandName::Search,
            CommandName::Check,
            CommandName::Fmt,
            CommandName::Ls,
            CommandName::Build,
            CommandName::Test,
            CommandName::Init,
            CommandName::Hooks,
            CommandName::Upgrade,
            CommandName::Mcp,
            CommandName::Log,
            CommandName::Stats,
        ] {
            let s = c.as_str();
            let parsed: CommandName = s.parse().unwrap();
            assert_eq!(parsed, c);
        }
    }

    #[test]
    fn unknown_rejected() {
        assert!("wrtie".parse::<CommandName>().is_err());
        assert!("".parse::<CommandName>().is_err());
    }

    #[test]
    fn display_equals_as_str() {
        assert_eq!(format!("{}", CommandName::Write), "write");
    }

    #[test]
    fn serde_serializes_as_lowercase_variant() {
        let json = serde_json::to_string(&CommandName::Stats).unwrap();
        assert_eq!(json, "\"stats\"");
    }
}
