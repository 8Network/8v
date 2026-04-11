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

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: GitCommand,
}

#[derive(clap::Subcommand)]
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
        path: None,      // current directory
        verbose: false,  // hooks should be quiet
        plain: true,     // plain text for parsing
        json: false,     // hooks use plain output
        no_color: false, // will be overridden by render_config
        limit: 10,       // default limit for error detail
        timeout: Some(std::time::Duration::from_secs(60)),
    };

    // Run check and get report.
    let report = crate::commands::check::run(&check_args, interrupted);

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
fn strip_attribution(content: &str) -> String {
    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains("Co-Authored-By"))
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
