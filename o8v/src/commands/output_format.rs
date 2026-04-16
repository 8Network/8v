// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Shared output format flags — one struct, all commands.

use o8v_core::render::Audience;

/// Output format flags embedded in every command's Args via `#[command(flatten)]`.
#[derive(clap::Args, Debug, Default)]
pub struct OutputFormat {
    /// Plain text output for AI agents and pipes.
    #[arg(long, conflicts_with = "json")]
    pub plain: bool,

    /// JSON output for tools and CI.
    #[arg(long, conflicts_with = "plain")]
    pub json: bool,

    /// Disable colored output.
    #[arg(long)]
    pub no_color: bool,
}

impl OutputFormat {
    /// Resolve audience, using `default` when no explicit flag was passed.
    ///
    /// - `--json` → `Audience::Machine` (always, regardless of caller)
    /// - `--plain` → `Audience::Agent` (always, regardless of caller)
    /// - no flag → `default` (caller decides: Human for CLI, Agent for MCP)
    pub fn audience_with_default(&self, default: Audience) -> Audience {
        if self.json {
            Audience::Machine
        } else if self.plain {
            Audience::Agent
        } else {
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::render::Audience;

    fn fmt(json: bool, plain: bool, no_color: bool) -> OutputFormat {
        OutputFormat { json, plain, no_color }
    }

    // 1. No flags + Human default → Human
    #[test]
    fn no_flags_human_default_returns_human() {
        let f = fmt(false, false, false);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
    }

    // 2. No flags + Agent default → Agent
    #[test]
    fn no_flags_agent_default_returns_agent() {
        let f = fmt(false, false, false);
        assert_eq!(f.audience_with_default(Audience::Agent), Audience::Agent);
    }

    // 3. --json + Human default → Machine
    #[test]
    fn json_flag_human_default_returns_machine() {
        let f = fmt(true, false, false);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Machine);
    }

    // 4. --json + Agent default → Machine
    #[test]
    fn json_flag_agent_default_returns_machine() {
        let f = fmt(true, false, false);
        assert_eq!(f.audience_with_default(Audience::Agent), Audience::Machine);
    }

    // 5. --plain + Human default → Agent
    #[test]
    fn plain_flag_human_default_returns_agent() {
        let f = fmt(false, true, false);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
    }

    // 6. --plain + Agent default → Agent
    #[test]
    fn plain_flag_agent_default_returns_agent() {
        let f = fmt(false, true, false);
        assert_eq!(f.audience_with_default(Audience::Agent), Audience::Agent);
    }

    // 7. Default trait → Human audience (all false)
    #[test]
    fn default_trait_gives_human_audience() {
        let f = OutputFormat::default();
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
    }

    // 8. no_color doesn't affect audience
    #[test]
    fn no_color_does_not_affect_audience() {
        let f = fmt(false, false, true);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        let f = fmt(true, false, true);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Machine);
        let f = fmt(false, true, true);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
    }
}
