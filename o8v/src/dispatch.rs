// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Command dispatch — the single entry point for all 8v commands.
//!
//! Both CLI (main.rs) and MCP (handler.rs) call `build_context()`.
//! It builds CommandContext from the path argument: detects project root,
//! opens storage, loads config. Neither entrypoint does this work itself.
//!
//! This is the first step toward a full `CommandHandler` as described in
//! `docs/design/storage-and-context.md`. The full dispatch with a `Command`
//! enum is a future step once command signatures take `&CommandContext`.

pub use o8v_workspace::{CommandContext, ContextError};

use o8v_core::command::{Command, CommandContext as CoreCommandContext, CommandError};
use o8v_core::render::{Audience, Renderable};
use o8v_core::task::TaskId;
use std::sync::atomic::AtomicBool;

/// Build a `CommandContext` from a path argument.
///
/// This is the single point where project root detection, storage opening,
/// and config loading happens. Both CLI and MCP call this.
///
/// # Errors
///
/// Returns `ContextError` if the path cannot be resolved, no project root
/// is detected, or storage cannot be opened. Every error is visible — no
/// silent fallbacks.
pub fn build_context(path: &str) -> Result<CommandContext, ContextError> {
    CommandContext::from_path(path)
}

/// Build a CoreCommandContext from the workspace context.
pub fn core_context(interrupted: &'static AtomicBool) -> CoreCommandContext {
    CoreCommandContext {
        interrupted,
        containment: None,
        stack: None,
        project_root: None,
    }
}

/// Dispatch a command: execute it, render the report, return the output string.
///
/// This is the generic dispatch function that replaces the match-on-Command
/// pattern in handler.rs. Any type implementing the Command trait can be
/// dispatched here.
pub async fn dispatch<C: Command>(
    command: &C,
    ctx: &CoreCommandContext,
    audience: Audience,
) -> Result<(String, TaskId, C::Report), CommandError>
where
    C::Report: Renderable,
    C::Event: 'static,
{
    let task_id = TaskId::new();

    // Create a channel for events (even if the command doesn't use it).
    let (events, mut rx) = o8v_core::event_channel::create_event_channel::<C::Event>(64);

    // Spawn event consumer (drains events to prevent channel blocking).
    let event_handle = tokio::spawn(async move {
        let mut rendered_events = Vec::new();
        while let Some(event) = rx.recv().await {
            let output = o8v_core::render::render(&event, audience);
            let s = output.into_string();
            if !s.is_empty() {
                rendered_events.push(s);
            }
        }
        rendered_events
    });

    // Execute the command.
    let report = command.execute(ctx, events).await?;

    // Wait for event consumer to finish and collect rendered events.
    let rendered_events = event_handle.await.unwrap_or_default();

    // Render the final report.
    let report_output = o8v_core::render::render(&report, audience).into_string();

    // Combine: progressive events first, then final report.
    let output = if rendered_events.is_empty() {
        report_output
    } else {
        let mut combined = rendered_events.join("");
        combined.push_str(&report_output);
        combined
    };

    Ok((output, task_id, report))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn build_context_for_valid_project() {
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        // Create a Cargo.toml so the directory is recognised as a project root.
        std::fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // Redirect HOME so StorageDir::open() doesn't touch the real home.
        std::env::set_var("HOME", home.path());

        let result = build_context(project.path().to_str().unwrap());
        assert!(
            result.is_ok(),
            "expected Ok for valid project, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn build_context_for_invalid_path() {
        let result = build_context("/nonexistent/path/that/does/not/exist");
        assert!(result.is_err(), "expected Err for nonexistent path, got Ok");
        match result {
            Err(ContextError::PathResolution(_)) => {}
            Err(e) => panic!("expected PathResolution error, got: {e}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }
}
