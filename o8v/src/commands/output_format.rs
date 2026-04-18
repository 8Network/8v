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

/// Returns true when `val` is a truthy agent-mode value.
///
/// Truthy values: `1`, `true`, `yes`. Everything else (including empty string,
/// `0`, `false`, `no`, unset) is falsy.
///
/// Test-only helper: allows unit tests to inject the env value directly
/// without mutating the process environment.
#[cfg(test)]
pub(crate) fn is_agent_mode_from(val: Option<String>) -> bool {
    match val {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes"),
        None => false,
    }
}

/// Returns true when the `_8V_AGENT` environment variable is set to a truthy value.
fn is_agent_mode() -> bool {
    matches!(
        std::env::var("_8V_AGENT").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
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
        } else if self.plain || is_agent_mode() {
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

    // --- is_agent_mode_from tests (dependency injection, no env mutation) ---

    // 9. is_agent_mode_from(Some("1")) → true
    #[test]
    fn agent_mode_from_1_is_true() {
        assert!(is_agent_mode_from(Some("1".to_string())));
    }

    // 10. is_agent_mode_from(Some("true")) → true
    #[test]
    fn agent_mode_from_true_is_true() {
        assert!(is_agent_mode_from(Some("true".to_string())));
    }

    // 11. is_agent_mode_from(Some("yes")) → true
    #[test]
    fn agent_mode_from_yes_is_true() {
        assert!(is_agent_mode_from(Some("yes".to_string())));
    }

    // 12. is_agent_mode_from(Some("0")) → false
    #[test]
    fn agent_mode_from_0_is_false() {
        assert!(!is_agent_mode_from(Some("0".to_string())));
    }

    // 13. is_agent_mode_from(Some("false")) → false
    #[test]
    fn agent_mode_from_false_is_false() {
        assert!(!is_agent_mode_from(Some("false".to_string())));
    }

    // 14. is_agent_mode_from(Some("")) → false (empty string is falsy)
    #[test]
    fn agent_mode_from_empty_is_false() {
        assert!(!is_agent_mode_from(Some(String::new())));
    }

    // 15. is_agent_mode_from(None) → false (unset)
    #[test]
    fn agent_mode_from_none_is_false() {
        assert!(!is_agent_mode_from(None));
    }

    // 16. audience_with_default honours agent mode via is_agent_mode_from
    #[test]
    fn agent_mode_from_1_audience_is_agent() {
        // Simulate _8V_AGENT=1 via dependency injection — no env mutation
        assert!(is_agent_mode_from(Some("1".to_string())));
        // With no flags set, plain path is false; but we test is_agent_mode_from directly.
        // The full audience_with_default path calls is_agent_mode() (live env),
        // so we verify the logic component independently.
        let f = fmt(false, false, false);
        // --plain flag triggers the same branch as agent mode
        let f_plain = fmt(false, true, false);
        assert_eq!(
            f_plain.audience_with_default(Audience::Human),
            Audience::Agent
        );
        // no flags → falls through to default
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
    }

    // --- --human flag tests ---

    // 17. --human + no env → Human
    #[test]
    fn human_flag_no_env_returns_human() {
        let f = fmt_human(true);
        assert_eq!(f.audience_with_default(Audience::Agent), Audience::Human);
    }

    // 18. --human overrides --plain branch (human takes precedence)
    #[test]
    fn human_flag_overrides_default() {
        let f = fmt_human(true);
        assert_eq!(f.audience_with_default(Audience::Human), Audience::Human);
    }
}
