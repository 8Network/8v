// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Formatter execution — detect projects, find formatters, run them.
//!
//! Types (FmtReport, FmtEntry, FmtOutcome, FmtConfig) live in o8v-core
//! because render depends on them. This module contains the orchestration.

use crate::stack_tools::FormatTool;
use o8v_core::diagnostic::sanitize;
use o8v_core::{FmtConfig, FmtEntry, FmtOutcome, FmtReport};
use crate::detect_all;
use o8v_core::project::{ProjectRoot, Stack};
use std::process::Command;
use std::sync::atomic::Ordering;

/// Find a binary, first trying PATH, then returning a "not found" error message.
///
/// This is a fallback for formatters that don't use node_modules resolution.
/// If the binary is not in PATH, returns None and the caller should report NotFound.
fn find_in_path(program: &str) -> Option<String> {
    // Try the program as-is (might be an absolute path or in PATH).
    if Command::new(program).arg("--version").output().is_ok() {
        return Some(program.to_string());
    }
    // If it fails, the binary is not found.
    None
}

/// Resolve the binary path for a formatter.
///
/// - If `needs_node_resolution`: walk up from `root` to find in `node_modules/.bin`
/// - Otherwise: use the program name as-is (caller should resolve from PATH)
///
/// Returns the binary path (absolute or relative as appropriate) or None if not found.
fn resolve_formatter_binary(root: &o8v_fs::ContainmentRoot, tool: &FormatTool) -> Option<String> {
    if tool.needs_node_resolution {
        // Walk up from root to find in node_modules/.bin
        crate::tool_resolution::find_node_bin(root, tool.program)
            .map(|p| p.to_string_lossy().to_string())
    } else {
        // For non-node tools, try PATH.
        find_in_path(tool.program).or_else(|| {
            // Fallback: return the program name and let the caller handle spawn error.
            Some(tool.program.to_string())
        })
    }
}

/// Run a single formatter on a project.
fn run_fmt(
    root: &o8v_fs::ContainmentRoot,
    tool: &FormatTool,
    process_config: &o8v_process::ProcessConfig,
    check_mode: bool,
) -> FmtOutcome {
    // Resolve the binary path
    let Some(binary) = resolve_formatter_binary(root, tool) else {
        return FmtOutcome::NotFound {
            program: tool.program.to_string(),
        };
    };

    // Build the command with appropriate args
    let mut cmd = Command::new(&binary);
    let args = if check_mode {
        tool.check_args
    } else {
        tool.format_args
    };
    cmd.args(args).current_dir(root.as_path());

    // Run the process
    let start = std::time::Instant::now();
    let result = o8v_process::run(cmd, process_config);
    let duration = start.elapsed();

    // Handle spawn errors (binary not found, permissions, etc.)
    if let o8v_process::ExitOutcome::SpawnError { cause, .. } = &result.outcome {
        return FmtOutcome::Error {
            cause: sanitize(cause),
            stderr: sanitize(&result.stderr),
        };
    }

    // Handle timeout and interruption
    if let o8v_process::ExitOutcome::Timeout { .. } = &result.outcome {
        return FmtOutcome::Error {
            cause: sanitize(&format!("timed out after {:?}", result.duration)),
            stderr: sanitize(&result.stderr),
        };
    }

    if let o8v_process::ExitOutcome::Interrupted = &result.outcome {
        return FmtOutcome::Error {
            cause: sanitize("interrupted"),
            stderr: sanitize(&result.stderr),
        };
    }

    // Handle signals
    if let o8v_process::ExitOutcome::Signal { signal } = &result.outcome {
        return FmtOutcome::Error {
            cause: sanitize(&format!("killed by signal {}", signal)),
            stderr: sanitize(&result.stderr),
        };
    }

    // In check mode: determine if files are dirty
    if check_mode {
        if tool.check_dirty_on_stdout {
            // For tools like gofmt that exit 0 but use stdout to list dirty files
            if !result.stdout.is_empty() {
                return FmtOutcome::Dirty { duration };
            }
            // stdout is empty means nothing to format
            FmtOutcome::Ok { duration }
        } else {
            // For tools where exit code determines success
            match &result.outcome {
                o8v_process::ExitOutcome::Success => FmtOutcome::Ok { duration },
                o8v_process::ExitOutcome::Failed { .. } => FmtOutcome::Dirty { duration },
                _ => unreachable!("already handled above"),
            }
        }
    } else {
        // In write mode: exit code determines success
        match &result.outcome {
            o8v_process::ExitOutcome::Success => FmtOutcome::Ok { duration },
            o8v_process::ExitOutcome::Failed { .. } => FmtOutcome::Error {
                cause: sanitize("formatter exited with error"),
                stderr: sanitize(&result.stderr),
            },
            _ => unreachable!("already handled above"),
        }
    }
}

/// Format a directory. Detects projects, finds formatters, and runs them.
pub fn fmt(root: &ProjectRoot, config: &FmtConfig) -> FmtReport {
    let _root_span = tracing::info_span!("fmt_run", path = %root).entered();

    // Early out if already interrupted
    if config.interrupted.load(Ordering::Acquire) {
        return FmtReport {
            entries: Vec::new(),
            detection_errors: Vec::new(),
        };
    }

    // Detect all projects
    let detected = {
        let _detect_span = tracing::info_span!("detect").entered();
        detect_all(root)
    };

    let (projects, detection_errors) = detected.into_parts();

    for err in &detection_errors {
        tracing::warn!(error = %err, "detection error");
    }

    tracing::info!(projects = projects.len(), "detection complete");

    // Calculate effective timeout
    let effective_timeout = match config.timeout {
        Some(user_cap) => std::cmp::min(o8v_process::DEFAULT_TIMEOUT, user_cap),
        None => o8v_process::DEFAULT_TIMEOUT,
    };

    let process_config = o8v_process::ProcessConfig {
        timeout: effective_timeout,
        interrupted: Some(config.interrupted),
        ..o8v_process::ProcessConfig::default()
    };

    // Deduplicate by (stack, root) — if the same stack appears in multiple projects
    // at the same root, run the formatter only once.
    let mut seen = Vec::new();
    let mut entries = Vec::new();

    for project in &projects {
        if config.interrupted.load(Ordering::Acquire) {
            tracing::info!("interrupted — skipping remaining projects");
            break;
        }

        let key = (project.stack(), project.path().to_string());
        if seen.iter().any(|k: &(Stack, String)| k == &key) {
            tracing::info!(
                stack = %project.stack(),
                path = %project.path(),
                "skipping duplicate stack+root"
            );
            continue;
        }
        seen.push(key);

        let _project_span = tracing::info_span!(
            "fmt_project",
            stack = %project.stack(),
            path = %project.path(),
        )
        .entered();

        let tools = crate::stacks::tools_for(project.stack());
        let Some(formatter) = tools.formatter else {
            tracing::info!(stack = %project.stack(), "no formatter for stack");
            continue;
        };

        let tool_name = formatter.program.to_string();
        tracing::info!(tool = %tool_name, "running formatter");

        let containment = match project.path().as_containment_root() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(path = %project.path(), "containment root unavailable: {e}");
                continue;
            }
        };
        let outcome = run_fmt(&containment, &formatter, &process_config, config.check_mode);

        match &outcome {
            FmtOutcome::Ok { duration } => {
                tracing::info!(tool = %tool_name, ?duration, "formatted successfully");
            }
            FmtOutcome::Dirty { duration } => {
                tracing::info!(tool = %tool_name, ?duration, "files need formatting");
            }
            FmtOutcome::Error { cause, .. } => {
                tracing::warn!(tool = %tool_name, cause, "formatter error");
            }
            FmtOutcome::NotFound { program } => {
                tracing::warn!(program, "formatter not found");
            }
        }

        entries.push(FmtEntry {
            stack: project.stack(),
            project_root: project.path().clone(),
            tool: tool_name,
            outcome,
        });
    }

    FmtReport {
        entries,
        detection_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::project::Stack;

    #[test]
    fn rust_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Rust);
        let formatter = tools.formatter.expect("rust has a formatter");
        assert_eq!(formatter.program, "cargo");
        assert_eq!(formatter.format_args, &["fmt", "--all"]);
        assert_eq!(formatter.check_args, &["fmt", "--all", "--check"]);
        assert!(!formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn go_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Go);
        let formatter = tools.formatter.expect("go has a formatter");
        assert_eq!(formatter.program, "gofmt");
        assert_eq!(formatter.format_args, &["-w", "."]);
        assert_eq!(formatter.check_args, &["-l", "."]);
        assert!(formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn typescript_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::TypeScript);
        let formatter = tools.formatter.expect("typescript has a formatter");
        assert_eq!(formatter.program, "prettier");
        assert!(formatter.needs_node_resolution);
        assert!(!formatter.check_dirty_on_stdout);
    }

    #[test]
    fn python_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Python);
        let formatter = tools.formatter.expect("python has a formatter");
        assert_eq!(formatter.program, "ruff");
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn javascript_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::JavaScript);
        let formatter = tools.formatter.expect("javascript has a formatter");
        assert_eq!(formatter.program, "prettier");
        assert!(formatter.needs_node_resolution);
        assert!(!formatter.check_dirty_on_stdout);
    }

    #[test]
    fn deno_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Deno);
        let formatter = tools.formatter.expect("deno has a formatter");
        assert_eq!(formatter.program, "deno");
        assert_eq!(formatter.format_args, &["fmt"]);
        assert_eq!(formatter.check_args, &["fmt", "--check"]);
        assert!(!formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn dotnet_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::DotNet);
        let formatter = tools.formatter.expect("dotnet has a formatter");
        assert_eq!(formatter.program, "dotnet");
        assert_eq!(formatter.format_args, &["format"]);
        assert_eq!(formatter.check_args, &["format", "--verify-no-changes"]);
        assert!(!formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn kotlin_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Kotlin);
        let formatter = tools.formatter.expect("kotlin has a formatter");
        assert_eq!(formatter.program, "ktlint");
        assert_eq!(formatter.format_args, &["--format"]);
        assert_eq!(formatter.check_args, &["--reporter=json"]);
        assert!(!formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    #[test]
    fn terraform_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Terraform);
        let formatter = tools.formatter.expect("terraform has a formatter");
        assert_eq!(formatter.program, "terraform");
        assert_eq!(formatter.format_args, &["fmt", "-recursive"]);
        assert_eq!(formatter.check_args, &["fmt", "-check", "-recursive"]);
        assert!(!formatter.check_dirty_on_stdout);
        assert!(!formatter.needs_node_resolution);
    }

    // Stacks without formatters
    #[test]
    fn ruby_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Ruby);
        let f = tools.formatter.expect("ruby has a formatter");
        assert_eq!(f.program, "rubocop");
        assert_eq!(f.format_args, &["-a"]);
    }

    #[test]
    fn swift_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Swift);
        let f = tools.formatter.expect("swift has a formatter");
        assert_eq!(f.program, "swiftformat");
    }

    #[test]
    fn java_formatter_config() {
        let tools = crate::stacks::tools_for(Stack::Java);
        let f = tools.formatter.expect("java has a formatter");
        assert_eq!(f.program, "google-java-format");
    }

    #[test]
    fn dockerfile_no_formatter() {
        let tools = crate::stacks::tools_for(Stack::Dockerfile);
        assert!(tools.formatter.is_none());
    }

    #[test]
    fn helm_no_formatter() {
        let tools = crate::stacks::tools_for(Stack::Helm);
        assert!(tools.formatter.is_none());
    }

    #[test]
    fn kustomize_no_formatter() {
        let tools = crate::stacks::tools_for(Stack::Kustomize);
        assert!(tools.formatter.is_none());
    }
}
