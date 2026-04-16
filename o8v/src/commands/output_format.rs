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

    /// Human-readable output (overrides _8V_AGENT env var).
    #[arg(long, conflicts_with_all = ["json", "plain"])]
    pub human: bool,

    /// Disable colored output.
    #[arg(long)]
    pub no_color: bool,
}

/// Returns true when the `_8V_AGENT` environment variable is set to a truthy value.
///
/// Truthy values: `1`, `true`, `yes`. Everything else (including empty string,
/// `0`, `false`, `no`, unset) is falsy.
fn is_agent_mode() -> bool {
    std::env::var("_8V_AGENT")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

impl OutputFormat {
    /// Resolve audience, using `default` when no explicit flag was passed.
    ///
    /// Precedence: `--json` > `--human` > `--plain` > `_8V_AGENT` > `default`
    ///
    /// - `--json` → `Audience::Machine` (always, regardless of caller)
    /// - `--human` → `Audience::Human` (escape hatch overriding _8V_AGENT)
    /// - `--plain` → `Audience::Agent` (always, regardless of caller)
    /// - `_8V_AGENT` truthy + no explicit flag → `Audience::Agent`
    /// - no flag + no env → `default` (caller decides: Human for CLI, Agent for MCP)
    pub fn audience_with_default(&self, default: Audience) -> Audience {
        if self.json {
            Audience::Machine
        } else if self.human {
            Audience::Human
        } else if self.plain {
            Audience::Agent
        } else if is_agent_mode() {
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
    use serial_test::serial;

    fn fmt(json: bool, plain: bool, no_color: bool) -> OutputFormat {
        OutputFormat { json, plain, no_color, human: false }
    }

    fn fmt_human(human: bool) -> OutputFormat {
        OutputFormat { json: false, plain: false, no_color: false, human }
    }

    fn with_agent_env<F: FnOnce()>(value: Option<&str>, f: F) {
        let key = "_8V_AGENT";
        let original = std::env::var(key).ok();
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        f();
        match original {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    // 1. No flags + Human default → Human
    #[test]
    #[serial]
    fn no_flags_human_default_returns_human() {
        with_agent_env(None, || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 2. No flags + Agent default → Agent
    #[test]
    #[serial]
    fn no_flags_agent_default_returns_agent() {
        with_agent_env(None, || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Agent), Audience::Agent);
        });
    }

    // 3. --json + Human default → Machine
    #[test]
    #[serial]
    fn json_flag_human_default_returns_machine() {
        with_agent_env(None, || {
            let f = fmt(true, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Machine);
        });
    }

    // 4. --json + Agent default → Machine
    #[test]
    #[serial]
    fn json_flag_agent_default_returns_machine() {
        with_agent_env(None, || {
            let f = fmt(true, false, false);
            assert_eq!(f.audience_with_default(Audience::Agent), Audience::Machine);
        });
    }

    // 5. --plain + Human default → Agent
    #[test]
    #[serial]
    fn plain_flag_human_default_returns_agent() {
        with_agent_env(None, || {
            let f = fmt(false, true, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
        });
    }

    // 6. --plain + Agent default → Agent
    #[test]
    #[serial]
    fn plain_flag_agent_default_returns_agent() {
        with_agent_env(None, || {
            let f = fmt(false, true, false);
            assert_eq!(f.audience_with_default(Audience::Agent), Audience::Agent);
        });
    }

    // 7. Default trait → Human audience (all false)
    #[test]
    #[serial]
    fn default_trait_gives_human_audience() {
        with_agent_env(None, || {
            let f = OutputFormat::default();
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 8. no_color doesn't affect audience
    #[test]
    #[serial]
    fn no_color_does_not_affect_audience() {
        with_agent_env(None, || {
            let f = fmt(false, false, true);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
            let f = fmt(true, false, true);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Machine);
            let f = fmt(false, true, true);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
        });
    }

    // --- _8V_AGENT env var tests ---

    // 9. _8V_AGENT=1 + no flags + Human default → Agent
    #[test]
    #[serial]
    fn agent_env_1_human_default_returns_agent() {
        with_agent_env(Some("1"), || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
        });
    }

    // 10. _8V_AGENT=1 + no flags + Agent default → Agent (MCP, unchanged)
    #[test]
    #[serial]
    fn agent_env_1_agent_default_returns_agent() {
        with_agent_env(Some("1"), || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Agent), Audience::Agent);
        });
    }

    // 11. _8V_AGENT=1 + --json → Machine (flag wins)
    #[test]
    #[serial]
    fn agent_env_1_json_flag_returns_machine() {
        with_agent_env(Some("1"), || {
            let f = fmt(true, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Machine);
        });
    }

    // 12. _8V_AGENT=1 + --plain → Agent (flag wins, same result)
    #[test]
    #[serial]
    fn agent_env_1_plain_flag_returns_agent() {
        with_agent_env(Some("1"), || {
            let f = fmt(false, true, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Agent);
        });
    }

    // 13. _8V_AGENT=0 + no flags → Human (falsy value)
    #[test]
    #[serial]
    fn agent_env_0_no_flags_returns_human() {
        with_agent_env(Some("0"), || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 14. _8V_AGENT=false + no flags → Human
    #[test]
    #[serial]
    fn agent_env_false_no_flags_returns_human() {
        with_agent_env(Some("false"), || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 15. _8V_AGENT= (empty) + no flags → Human
    #[test]
    #[serial]
    fn agent_env_empty_no_flags_returns_human() {
        with_agent_env(Some(""), || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 16. Unset _8V_AGENT + no flags → Human
    #[test]
    #[serial]
    fn agent_env_unset_no_flags_returns_human() {
        with_agent_env(None, || {
            let f = fmt(false, false, false);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // --- --human flag tests ---

    // 17. _8V_AGENT=1 + --human → Human (escape hatch overrides env var)
    #[test]
    #[serial]
    fn agent_env_1_human_flag_returns_human() {
        with_agent_env(Some("1"), || {
            let f = fmt_human(true);
            assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
        });
    }

    // 18. --human + no env → Human
    #[test]
    #[serial]
    fn human_flag_no_env_returns_human() {
        with_agent_env(None, || {
            let f = fmt_human(true);
            assert_eq!(f.audience_with_default(Audience::Agent), Audience::Human);
        });
    }
}
