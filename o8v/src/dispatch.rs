// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Command dispatch — the single entry point for all 8v commands.
//!
//! Interfaces (CLI, MCP) call into this module. They provide the parsed command,
//! who they are (Caller), and the interrupted flag. Dispatch does everything
//! else: context, bus, subscribers, execute, render, return.

pub use crate::workspace::{resolve_workspace, ContextError};

use o8v_core::caller::Caller;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::event_bus::EventBus;
use o8v_core::events::{CommandCompleted, CommandStarted};
use o8v_core::extensions::Extensions;
use o8v_core::render::{Audience, Renderable};
use o8v_core::task::TaskId;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// RAII guard that guarantees `CommandCompleted` is emitted even on panic.
///
/// Created after `CommandStarted` is emitted. Holds a weak reference to the
/// bus (to avoid keeping the bus alive past its intended lifetime) and the
/// `run_id`. On `drop()`, if `complete()` was never called, emits
/// `CommandCompleted` with `success=false` and `output_bytes=0`.
///
/// Normal paths call `complete()` explicitly, which arms the guard's
/// "already emitted" flag and emits the real `CommandCompleted` (with accurate
/// output_bytes and success). The `Drop` impl then does nothing.
struct CommandGuard {
    run_id: String,
    start_ms: u64,
    bus: std::sync::Weak<EventBus>,
    /// Set to true once `complete()` is called. If false at drop time, the
    /// guard emits a synthetic CommandCompleted to prevent an orphan Started.
    completed: bool,
}

impl CommandGuard {
    fn new(run_id: String, start_ms: u64, bus: &Arc<EventBus>) -> Self {
        Self {
            run_id,
            start_ms,
            bus: Arc::downgrade(bus),
            completed: false,
        }
    }

    /// Emit the real `CommandCompleted` and mark the guard as complete so
    /// `drop()` does not emit a duplicate.
    fn complete(&mut self, output_bytes: u64, success: bool) {
        let duration_ms = Self::elapsed_ms(self.start_ms);
        if let Some(bus) = self.bus.upgrade() {
            let ev = CommandCompleted::new(self.run_id.clone(), output_bytes, duration_ms, success);
            bus.emit(&ev);
        }
        self.completed = true;
    }

    fn elapsed_ms(start_ms: u64) -> u64 {
        match std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH) {
            Ok(d) => (d.as_millis() as u64).saturating_sub(start_ms),
            Err(_) => 0,
        }
    }
}

impl Drop for CommandGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        // Panic path or early return without calling complete() — emit a
        // synthetic CommandCompleted so the event log has a matching pair.
        let duration_ms = Self::elapsed_ms(self.start_ms);
        if let Some(bus) = self.bus.upgrade() {
            let ev = CommandCompleted::new(self.run_id.clone(), 0, duration_ms, false);
            bus.emit(&ev);
        }
    }
}

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
    // Both failures emit a tracing::warn! so operators can identify why storage
    // is unavailable, rather than seeing a silent no-workspace fallback.
    match std::env::current_dir() {
        Ok(cwd) => {
            match resolve_workspace(cwd.to_string_lossy().as_ref()) {
                Ok(ws) => {
                    // Subscribe StorageSubscriber before any events are emitted.
                    let sub = Arc::new(crate::storage_subscriber::StorageSubscriber::new(
                        ws.storage.clone(),
                    ));
                    bus.subscribe(sub);

                    // Insert WorkspaceRoot — the trust boundary for all file I/O in commands.
                    if let Ok(workspace_root) =
                        crate::workspace::WorkspaceRoot::new(ws.root.to_string())
                    {
                        extensions.insert(workspace_root);
                    }

                    extensions.insert(ws.root);
                    extensions.insert(ws.storage);
                    extensions.insert(ws.config);
                }
                Err(e) => {
                    tracing::warn!(
                        cwd = %cwd.display(),
                        error = %e,
                        "dispatch: could not resolve workspace from cwd;                          storage will be unavailable for this command"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "dispatch: could not get current_dir;                  workspace resolution skipped, storage will be unavailable"
            );
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
    argv: &[String],
) -> Result<(String, TaskId, C::Report), CommandError>
where
    C::Report: Renderable,
{
    let task_id = TaskId::new();
    let run_id = task_id.to_string();
    let start_ms = match std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
    {
        Ok(d) => d.as_millis() as u64,
        Err(e) => {
            tracing::warn!(error = %e, "dispatch: system clock is before UNIX_EPOCH, using 0 for start_ms");
            0
        }
    };

    // Project path from extensions (if available).
    let project_path = ctx
        .extensions
        .get::<o8v_core::project::ProjectRoot>()
        .map(|r| r.to_string());

    // Emit CommandStarted (with agent identity if available from MCP handshake).
    if let Some(bus) = ctx.extensions.get::<Arc<EventBus>>() {
        let agent_info = ctx.extensions.get::<o8v_core::caller::AgentInfo>().cloned();
        let ev = CommandStarted::new(
            run_id.clone(),
            caller,
            command_str,
            argv.to_vec(),
            project_path,
        )
        .with_agent_info(agent_info);
        bus.emit(&ev);
    }

    // Arm the RAII guard immediately after CommandStarted is emitted.
    // If execute() or render() panics, Drop fires and emits a synthetic
    // CommandCompleted(success=false) so the event log always has a matching pair.
    let mut guard = ctx
        .extensions
        .get::<Arc<EventBus>>()
        .map(|bus| CommandGuard::new(run_id.clone(), start_ms, bus));

    // Execute the command.
    let result = command.execute(ctx).await;

    // On failure: disarm via guard.complete(0, false), then propagate the error.
    // On success: render first (to get output_len), then disarm via guard.complete.
    let report = match result {
        Err(e) => {
            if let Some(ref mut g) = guard {
                g.complete(0, false);
            }
            return Err(e);
        }
        Ok(report) => report,
    };

    // Render the final report.
    let output = o8v_core::render::render(&report, audience).into_string();

    // Emit CommandCompleted with the real output length.
    if let Some(ref mut g) = guard {
        g.complete(output.len() as u64, true);
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
        async fn execute(&self, _ctx: &CommandContext) -> Result<(), CommandError> {
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
        let result = dispatch(&cmd, &ctx, Audience::Agent, Caller::Cli, "noop", &[]).await;
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
        async fn execute(&self, _ctx: &CommandContext) -> Result<(), CommandError> {
            Err(CommandError::Execution(
                "intentional test failure".to_string(),
            ))
        }
    }

    /// A command that panics inside execute() — used to verify CommandCompleted
    /// is still emitted by the CommandGuard RAII drop handler.
    struct PanicCommand;
    impl o8v_core::command::Command for PanicCommand {
        type Report = ();
        async fn execute(&self, _ctx: &CommandContext) -> Result<(), CommandError> {
            panic!("intentional panic in PanicCommand");
        }
    }

    /// F5 regression: CommandCompleted must be emitted even when execute() panics.
    ///
    /// Pre-fix behaviour: dispatch() called bus.emit(CommandCompleted) only on
    /// explicit Err and Ok paths. A panic bypassed both branches, leaving an
    /// orphan CommandStarted with no matching CommandCompleted in the event log.
    /// Post-fix: CommandGuard::drop() fires on panic unwind and emits the event.
    ///
    /// Strategy: run dispatch on a dedicated thread with its own single-threaded
    /// tokio runtime so we can catch the panic via std::thread::spawn's join handle
    /// without fighting the outer test runtime's `block_on` restrictions.
    #[test]
    fn dispatch_emits_completed_on_panic() {
        // Shared recorder — Arc so we can move into the thread and read after join.
        let recorder = Arc::new(RecordingSubscriber::new());
        let recorder_clone = recorder.clone();

        static PANIC_INTERRUPTED: AtomicBool = AtomicBool::new(false);

        let handle = std::thread::spawn(move || {
            // Build a fresh single-threaded runtime for this thread.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");

            rt.block_on(async {
                let ctx = build_context(&PANIC_INTERRUPTED);
                let bus = ctx.extensions.get::<Arc<EventBus>>().unwrap().clone();
                bus.subscribe(recorder_clone.clone());

                let cmd = PanicCommand;
                let _ = dispatch(&cmd, &ctx, Audience::Agent, Caller::Cli, "panic", &[]).await;
            });
        });

        // join() returns Err if the thread panicked — that is expected here.
        let _ = handle.join();

        // After the panic + unwind, CommandGuard::drop() must have fired.
        let events = recorder.events.lock().unwrap();
        assert!(
            events.contains(&"CommandStarted"),
            "CommandStarted must be recorded; events: {:?}",
            *events
        );
        assert!(
            events.contains(&"CommandCompleted"),
            "CommandCompleted must be emitted by guard drop on panic; events: {:?}",
            *events
        );
    }

    #[tokio::test]
    async fn dispatch_emits_completed_on_failure() {
        let ctx = build_context(&TEST_INTERRUPTED);
        let bus = ctx.extensions.get::<Arc<EventBus>>().unwrap().clone();
        let recorder = Arc::new(RecordingSubscriber::new());
        bus.subscribe(recorder.clone());

        let cmd = FailCommand;
        let result = dispatch(&cmd, &ctx, Audience::Agent, Caller::Cli, "fail", &[]).await;
        assert!(result.is_err(), "command must return an error");

        let events = recorder.events.lock().unwrap();
        assert_eq!(
            events.len(),
            2,
            "must emit CommandStarted and CommandCompleted even on failure"
        );
        assert_eq!(events[0], "CommandStarted");
        assert_eq!(events[1], "CommandCompleted");
    }
}
