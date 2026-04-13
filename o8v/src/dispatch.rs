// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Command dispatch — the single entry point for all 8v commands.
//!
//! Interfaces (CLI, MCP) call into this module. They provide the parsed command,
//! who they are (Caller), and the interrupted flag. Dispatch does everything
//! else: context, bus, subscribers, execute, render, return.

pub use o8v_workspace::{resolve_workspace, ContextError};

use o8v_core::caller::Caller;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::events::{CommandCompleted, CommandStarted};
use o8v_core::event_bus::EventBus;
use o8v_core::extensions::Extensions;
use o8v_core::render::{Audience, Renderable};
use o8v_core::task::TaskId;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Build a fully-wired CommandContext.
///
/// 1. Creates Extensions with EventBus
/// 2. Resolves workspace (project root, storage, config) — best effort
/// 3. Subscribes StorageSubscriber if storage available
///
/// This is the ONE place context is built. Interfaces never touch it.
pub fn build_context(interrupted: &'static AtomicBool) -> CommandContext {
    let mut extensions = Extensions::new();
    let bus = Arc::new(EventBus::new());

    // Best-effort workspace resolution from CWD.
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok((project_root, storage, config)) =
            resolve_workspace(cwd.to_string_lossy().as_ref())
        {
            // Subscribe StorageSubscriber before any events are emitted.
            let sub = Arc::new(crate::storage_subscriber::StorageSubscriber::new(
                storage.clone(),
            ));
            bus.subscribe(sub);

            // Insert WorkspaceRoot — the trust boundary for all file I/O in commands.
            if let Ok(workspace_root) =
                o8v_workspace::WorkspaceRoot::new(project_root.to_string())
            {
                extensions.insert(workspace_root);
            }

            extensions.insert(project_root);
            extensions.insert(storage);
            extensions.insert(config);
        }
    }

    extensions.insert(bus);
    CommandContext {
        interrupted,
        extensions,
    }
}

/// Execute a typed command: emit lifecycle events, execute, render.
///
/// Called by `dispatch_command()` after matching on the Command enum.
/// Caller and command_str are for lifecycle events only.
pub async fn dispatch<C: Command>(
    command: &C,
    ctx: &CommandContext,
    audience: Audience,
    caller: Caller,
    command_str: &str,
) -> Result<(String, TaskId, C::Report), CommandError>
where
    C::Report: Renderable,
{
    let task_id = TaskId::new();
    let run_id = task_id.to_string();
    let start_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Project path from extensions (if available).
    let project_path = ctx
        .extensions
        .get::<o8v_project::ProjectRoot>()
        .map(|r| r.to_string());

    // Emit CommandStarted.
    if let Some(bus) = ctx.extensions.get::<Arc<EventBus>>() {
        let ev = CommandStarted::new(run_id.clone(), caller, command_str, project_path);
        bus.emit(&ev);
    }

    // Execute the command.
    let result = command.execute(ctx).await;
    let success = result.is_ok();

    // Compute duration.
    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let duration_ms = end_ms.saturating_sub(start_ms);

    // On failure: emit CommandCompleted with success=false, then propagate the error.
    // On success: render first (to get output_len), then emit CommandCompleted.
    let report = match result {
        Err(e) => {
            if let Some(bus) = ctx.extensions.get::<Arc<EventBus>>() {
                let ev = CommandCompleted::new(run_id, 0, duration_ms, false);
                bus.emit(&ev);
            }
            return Err(e);
        }
        Ok(report) => report,
    };

    // Render the final report.
    let output = o8v_core::render::render(&report, audience).into_string();

    // Emit CommandCompleted with the real output length.
    if let Some(bus) = ctx.extensions.get::<Arc<EventBus>>() {
        let ev = CommandCompleted::new(run_id, output.len() as u64, duration_ms, success);
        bus.emit(&ev);
    }

    Ok((output, task_id, report))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_INTERRUPTED: AtomicBool = AtomicBool::new(false);

    #[test]
    fn build_context_has_event_bus() {
        let ctx = build_context(&TEST_INTERRUPTED);
        assert!(
            ctx.extensions.get::<Arc<EventBus>>().is_some(),
            "context must include an EventBus"
        );
    }

    /// A test subscriber that records event type names.
    struct RecordingSubscriber {
        events: std::sync::Mutex<Vec<&'static str>>,
    }
    impl RecordingSubscriber {
        fn new() -> Self {
            Self {
                events: std::sync::Mutex::new(Vec::new()),
            }
        }
    }
    impl o8v_core::event_bus::Subscriber for RecordingSubscriber {
        fn on_event(&self, message: &[u8]) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(message) {
                if let Some(event_type) = v.get("event").and_then(|e| e.as_str()) {
                    match event_type {
                        "CommandStarted" => self.events.lock().unwrap().push("CommandStarted"),
                        "CommandCompleted" => self.events.lock().unwrap().push("CommandCompleted"),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Minimal command for testing dispatch. Report is () which has Renderable.
    struct NoopCommand;
    impl o8v_core::command::Command for NoopCommand {
        type Report = ();
        async fn execute(
            &self,
            _ctx: &CommandContext,
        ) -> Result<(), CommandError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn dispatch_emits_lifecycle_events() {
        let ctx = build_context(&TEST_INTERRUPTED);
        let bus = ctx.extensions.get::<Arc<EventBus>>().unwrap().clone();
        let recorder = Arc::new(RecordingSubscriber::new());
        bus.subscribe(recorder.clone());

        let cmd = NoopCommand;
        let result = dispatch(&cmd, &ctx, Audience::Agent, Caller::Cli, "noop").await;
        assert!(result.is_ok());

        let events = recorder.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "CommandStarted");
        assert_eq!(events[1], "CommandCompleted");
    }

    /// A command that always fails — used to verify CommandCompleted is emitted on error.
    struct FailCommand;
    impl o8v_core::command::Command for FailCommand {
        type Report = ();
        async fn execute(
            &self,
            _ctx: &CommandContext,
        ) -> Result<(), CommandError> {
            Err(CommandError::Execution("intentional test failure".to_string()))
        }
    }

    #[tokio::test]
    async fn dispatch_emits_completed_on_failure() {
        let ctx = build_context(&TEST_INTERRUPTED);
        let bus = ctx.extensions.get::<Arc<EventBus>>().unwrap().clone();
        let recorder = Arc::new(RecordingSubscriber::new());
        bus.subscribe(recorder.clone());

        let cmd = FailCommand;
        let result = dispatch(&cmd, &ctx, Audience::Agent, Caller::Cli, "fail").await;
        assert!(result.is_err(), "command must return an error");

        let events = recorder.events.lock().unwrap();
        assert_eq!(events.len(), 2, "must emit CommandStarted and CommandCompleted even on failure");
        assert_eq!(events[0], "CommandStarted");
        assert_eq!(events[1], "CommandCompleted");
    }
}
