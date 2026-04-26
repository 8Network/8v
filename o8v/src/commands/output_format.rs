// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Shared output format flags — one struct, all commands.

use o8v_core::render::Audience;

/// Output format flags embedded in every command's Args via `#[command(flatten)]`.
#[derive(clap::Args, Debug, Default)]
pub struct OutputFormat {
    /// Plain text output for AI agents and pipes.
    #[arg(long, conflicts_with_all = ["json", "human"])]
    pub plain: bool,

    /// JSON output for tools and CI.
    #[arg(long, conflicts_with_all = ["plain", "human"])]
    pub json: bool,

    /// Human-readable output (escape hatch; overrides agent-mode default).
    #[arg(long, conflicts_with_all = ["json", "plain"])]
    pub human: bool,

    /// Disable colored output.
    #[arg(long)]
    pub no_color: bool,
}

impl OutputFormat {
    /// Resolve audience, using `default` when no explicit flag was passed.
    ///
    /// Precedence: `--json` > `--human` > `--plain` > `default`
    ///
    /// - `--json` → `Audience::Machine` (always, regardless of caller)
    /// - `--human` → `Audience::Human` (escape hatch for CLI users in agent mode)
    /// - `--plain` → `Audience::Agent` (always, regardless of caller)
    /// - no flag → `default` (resolved once at process entry: `Human` for CLI,
    ///   `Agent` for MCP; `_8V_AGENT` env var is read there, not here)
    pub fn audience_with_default(&self, default: Audience) -> Audience {
        if self.json {
            Audience::Machine
        } else if self.human {
            Audience::Human
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
        OutputFormat {
            json,
            plain,
            no_color,
            human: false,
        }
    }

    fn fmt_human(human: bool) -> OutputFormat {
        OutputFormat {
            json: false,
            plain: false,
            no_color: false,
            human,
        }
    }

    // 1. No flags + Human default → Human
    #[test]
    fn no_flags_human_default_returns_human() {
        let f = fmt(false, false, false);
        assert_eq!(
            f.audience_with_default(Audience::Human),
            Audience::Human,
            "no flags + Human default"
        );
    }

    // 2. No flags + Agent default → Agent
    #[test]
    fn no_flags_agent_default_returns_agent() {
        let f = fmt(false, false, false);
        assert_eq!(
            f.audience_with_default(Audience::Agent),
            Audience::Agent,
            "no flags + Agent default"
        );
    }

    // 3. --json + Human default → Machine
    #[test]
    fn json_flag_human_default_returns_machine() {
        let f = fmt(true, false, false);
        assert_eq!(
            f.audience_with_default(Audience::Human),
            Audience::Machine,
            "--json + Human default"
        );
    }

    // 4. --json + Agent default → Machine
    #[test]
    fn json_flag_agent_default_returns_machine() {
        let f = fmt(true, false, false);
        assert_eq!(
            f.audience_with_default(Audience::Agent),
            Audience::Machine,
            "--json + Agent default"
        );
    }

    // 5. --plain + Human default → Agent
    #[test]
    fn plain_flag_human_default_returns_agent() {
        let f = fmt(false, true, false);
        assert_eq!(
            f.audience_with_default(Audience::Human),
            Audience::Agent,
            "--plain + Human default"
        );
    }

    // 6. --plain + Agent default → Agent
    #[test]
    fn plain_flag_agent_default_returns_agent() {
        let f = fmt(false, true, false);
        assert_eq!(
            f.audience_with_default(Audience::Agent),
            Audience::Agent,
            "--plain + Agent default"
        );
    }

    // 7. Default trait → Human audience (all false)
    #[test]
    fn default_trait_gives_human_audience() {
        let f = OutputFormat::default();
        assert_eq!(
            f.audience_with_default(Audience::Human),
            Audience::Human,
            "default trait + Human"
        );
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

    // 9. _8V_AGENT=1 at entry resolves to Agent default; --plain is NOT needed
    //    Entry point passes Audience::Agent as default — no flags → Agent
    #[test]
    fn agent_default_no_flags_returns_agent() {
        let f = fmt(false, false, false);
        // Simulates: entry resolved _8V_AGENT=1 → default = Audience::Agent
        assert_eq!(
            f.audience_with_default(Audience::Agent),
            Audience::Agent,
            "_8V_AGENT=1 resolved at entry → Agent default → Agent"
        );
    }

    // 10. _8V_AGENT=1 at entry + --human flag → Human (escape hatch wins)
    #[test]
    fn agent_default_human_flag_returns_human() {
        let f = fmt_human(true);
        // Simulates: entry resolved _8V_AGENT=1 → default = Audience::Agent,
        // but user passed --human explicitly
        assert_eq!(
            f.audience_with_default(Audience::Agent),
            Audience::Human,
            "_8V_AGENT=1 resolved at entry + --human flag → Human"
        );
    }

    // --- --human flag tests ---

    // 11. --human + Human default → Human
    #[test]
    fn human_flag_no_env_returns_human() {
        let f = fmt_human(true);
        assert_eq!(f.audience_with_default(Audience::Agent), Audience::Human);
    }

    // 12. --human overrides agent default (human takes precedence)
    #[test]
    fn human_flag_overrides_default() {
        let f = fmt_human(true);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
    }
}
