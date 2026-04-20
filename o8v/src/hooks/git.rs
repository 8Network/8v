// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Git hook handlers — what runs when git fires a hook.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use crate::cli::common::{EXIT_FAIL, EXIT_INTERRUPTED, EXIT_OK};

// ─── Config ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct Config {
    #[serde(default)]
    git: GitConfig,
}

#[derive(Deserialize)]
struct GitConfig {
    #[serde(default = "default_true")]
    strip_attribution: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            strip_attribution: true,
        }
    }
}

fn default_true() -> bool {
    true
}

// ─── Args ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: GitCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum GitCommand {
    /// Run pre-commit checks
    OnCommit,
    /// Process commit message file
    OnCommitMsg {
        /// Path to the commit message file
        file: PathBuf,
    },
}

// ─── Run ─────────────────────────────────────────────────────────────────────

pub fn run(args: &Args, interrupted: &'static AtomicBool) -> ExitCode {
    match &args.command {
        GitCommand::OnCommit => on_commit(interrupted),
        GitCommand::OnCommitMsg { file } => {
            let root = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("8v: cannot determine working directory: {e}");
                    return ExitCode::from(EXIT_FAIL);
                }
            };
            on_commit_msg(file, &root)
        }
    }
}

/// Run `8v check .` as a pre-commit hook.
///
/// Constructs check::Args with sensible defaults for hook execution
/// (plain output, current directory, no verbosity).
/// Executes the check, renders output, and returns the exit code.
pub fn on_commit(interrupted: &'static AtomicBool) -> ExitCode {
    // Build check args for hook execution.
    let check_args = crate::commands::check::Args {
        path: None,     // current directory
        verbose: false, // hooks should be quiet
        format: crate::commands::output_format::OutputFormat {
            plain: true,
            ..Default::default()
        },
        limit: 10, // default limit for error detail
        page: 1,   // default to first page
        timeout: Some(std::time::Duration::from_secs(60)),
    };

    // Build context (resolves workspace from CWD, wires StorageSubscriber).
    let ctx = crate::dispatch::build_context(interrupted);

    // Run check and get report.
    let report = crate::commands::check::run(&check_args, &ctx);

    // Render and print the report (plain text for hooks).
    let report = match report {
        Ok(r) => {
            use o8v_core::render::Renderable;
            let output = r.render_plain();
            print!("{output}");
            r
        }
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(EXIT_FAIL);
        }
    };

    // Compute exit code from the report.
    // In a pre-commit hook context, "nothing to check" is not an error.
    if interrupted.load(std::sync::atomic::Ordering::Acquire) {
        ExitCode::from(EXIT_INTERRUPTED)
    } else if report.results().is_empty() && report.detection_errors().is_empty() {
        // Nothing to check — not an error in hook context.
        ExitCode::from(EXIT_OK)
    } else if report.is_ok() {
        ExitCode::from(EXIT_OK)
    } else {
        ExitCode::from(EXIT_FAIL)
    }
}

/// Run the commit-msg hook: load config, strip attribution if enabled.
pub fn on_commit_msg(file: &Path, root: &Path) -> ExitCode {
    let config = load_config(root);

    if !config.git.strip_attribution {
        return ExitCode::SUCCESS;
    }

    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("8v: cannot read commit message file: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
    };

    let modified = strip_attribution(&content);

    if modified != content {
        if let Err(e) = fs::write(file, &modified) {
            eprintln!("8v: cannot write commit message file: {e}");
            return ExitCode::from(EXIT_FAIL);
        }
    }

    ExitCode::SUCCESS
}

/// Load config from `.8v/config.toml` within the given root. Returns default if missing or invalid.
fn load_config(root: &Path) -> Config {
    let config_path = root.join(".8v/config.toml");
    match fs::read_to_string(&config_path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("8v: invalid .8v/config.toml: {e}");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

/// Remove lines containing "Co-Authored-By" from `content`.
/// Preserves a trailing newline if the original had one.
/// Git-trailer keys (case-insensitive) that indicate AI-assisted authorship.
/// Matched only in trailer position — `<key>:` at the start of a line
/// (ignoring leading whitespace). Inline prose mentions are preserved.
///
/// Non-AI trailers (Signed-off-by, Reviewed-by, Tested-by, …) are
/// deliberately NOT in this list — they carry legal or review attestations.
const AI_TRAILER_KEYS: &[&str] = &[
    "Co-Authored-By",
    "Co-Written-By",
    "Generated-By",
    "Generated-With",
    "Assisted-By",
    "AI-Assistant",
    "AI-Generated",
];

fn is_ai_trailer_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(colon_idx) = trimmed.find(':') else {
        return false;
    };
    let key = &trimmed[..colon_idx];
    AI_TRAILER_KEYS
        .iter()
        .any(|expected| key.eq_ignore_ascii_case(expected))
}

fn strip_attribution(content: &str) -> String {
    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| !is_ai_trailer_line(line))
        .collect();
    let mut result = filtered.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn strip_attribution_removes_co_authored_by() {
        let input = "feat: add feature\n\nSome description\nCo-Authored-By: AI <ai@example.com>\n";
        let result = strip_attribution(input);
        assert_eq!(result, "feat: add feature\n\nSome description\n");
    }

    #[test]
    fn strip_attribution_preserves_content_without_co_authored_by() {
        let input = "feat: add feature\n\nSome description\n";
        let result = strip_attribution(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_attribution_handles_no_trailing_newline() {
        let input = "feat: add feature\nCo-Authored-By: AI";
        let result = strip_attribution(input);
        assert_eq!(result, "feat: add feature");
    }

    // ── H-3: broaden AI-attribution stripping beyond Co-Authored-By ─────────

    #[test]
    fn h3_strips_generated_by_trailer() {
        let input = "feat: X\n\nbody\nGenerated-By: Claude\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_strips_generated_with_trailer() {
        let input = "feat: X\n\nbody\nGenerated-With: Claude Code\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_strips_co_written_by_trailer() {
        let input = "feat: X\n\nbody\nCo-Written-By: AI <ai@example.com>\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_strips_assisted_by_trailer() {
        let input = "feat: X\n\nbody\nAssisted-By: Claude\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_strips_ai_assistant_trailer() {
        let input = "feat: X\n\nbody\nAI-Assistant: Claude Code\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_strips_ai_generated_trailer() {
        let input = "feat: X\n\nbody\nAI-Generated: true\n";
        assert_eq!(strip_attribution(input), "feat: X\n\nbody\n");
    }

    #[test]
    fn h3_case_insensitive_trailer_key() {
        // Git trailers are case-insensitive on the key. "co-authored-by:" must
        // be stripped just like "Co-Authored-By:".
        let input = "feat: X\n\ngenerated-by: Claude\nCO-AUTHORED-BY: AI\nco-authored-by: bot\n";
        assert_eq!(strip_attribution(input), "feat: X\n\n");
    }

    #[test]
    fn h3_preserves_signed_off_by_trailer() {
        // DCO / legal attestation must never be stripped.
        let input = "feat: X\n\nbody\nSigned-off-by: Dev <dev@example.com>\n";
        assert_eq!(strip_attribution(input), input);
    }

    #[test]
    fn h3_preserves_reviewed_and_tested_by_trailers() {
        let input = "feat: X\n\nReviewed-by: Alice\nTested-by: Bob\n";
        assert_eq!(strip_attribution(input), input);
    }

    #[test]
    fn h3_preserves_inline_mention_of_co_authored_by() {
        // Inline prose mentions of the trailer name (without the trailer
        // `key:` shape) must be preserved — the scope is trailers only, not
        // any word that looks like one.
        let input = "feat: X\n\nThe Co-Authored-By convention is documented here.\n";
        assert_eq!(strip_attribution(input), input);
    }

    #[test]
    fn h3_preserves_inline_mention_of_generated_by() {
        let input = "feat: X\n\nThis file was Generated-By the old tool (now replaced).\n";
        assert_eq!(strip_attribution(input), input);
    }

    #[test]
    fn config_defaults_to_strip_attribution_true() {
        let dir = TempDir::new().unwrap();
        let config = load_config(dir.path());
        assert!(config.git.strip_attribution);
    }

    #[test]
    fn config_strip_attribution_false_is_respected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".8v")).unwrap();
        fs::write(
            dir.path().join(".8v/config.toml"),
            "[git]\nstrip_attribution = false\n",
        )
        .unwrap();
        let config = load_config(dir.path());
        assert!(!config.git.strip_attribution);
    }

    #[test]
    fn on_commit_msg_strips_when_enabled() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".8v")).unwrap();
        fs::write(
            dir.path().join(".8v/config.toml"),
            "[git]\nstrip_attribution = true\n",
        )
        .unwrap();

        let msg_file = dir.path().join("COMMIT_EDITMSG");
        fs::write(
            &msg_file,
            "feat: something\n\nCo-Authored-By: AI <ai@test.com>\n",
        )
        .unwrap();

        let result = on_commit_msg(&msg_file, dir.path());
        assert_eq!(result, ExitCode::SUCCESS);

        let content = fs::read_to_string(&msg_file).unwrap();
        assert_eq!(content, "feat: something\n\n");
        assert!(!content.contains("Co-Authored-By"));
    }

    #[test]
    fn on_commit_msg_skips_when_disabled() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".8v")).unwrap();
        fs::write(
            dir.path().join(".8v/config.toml"),
            "[git]\nstrip_attribution = false\n",
        )
        .unwrap();

        let msg_file = dir.path().join("COMMIT_EDITMSG");
        let original = "feat: something\n\nCo-Authored-By: AI <ai@test.com>\n";
        fs::write(&msg_file, original).unwrap();

        let result = on_commit_msg(&msg_file, dir.path());
        assert_eq!(result, ExitCode::SUCCESS);

        let content = fs::read_to_string(&msg_file).unwrap();
        assert_eq!(content, original);
    }
}
