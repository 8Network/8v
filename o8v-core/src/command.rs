// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use std::future::Future;
use std::sync::atomic::AtomicBool;

use crate::event_channel::EventChannel;
use crate::render::Renderable;

/// Typed errors for the command pipeline.
#[derive(Debug)]
pub enum CommandError {
    /// Path does not exist, is not a directory, etc.
    Path(String),
    /// Context building failed (storage, config, etc.)
    Context(String),
    /// Command-specific failure (e.g., "no test runner for this stack").
    Execution(String),
    /// Process exceeded timeout.
    Timeout,
    /// User cancelled via interrupt signal.
    Interrupted,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(msg) => write!(f, "path error: {msg}"),
            Self::Context(msg) => write!(f, "context error: {msg}"),
            Self::Execution(msg) => write!(f, "{msg}"),
            Self::Timeout => write!(f, "command timed out"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Framework-level cancellation mode. Commands don't see this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelMode {
    /// Set interrupted flag, let command finish, drain events.
    Graceful,
    /// Abort the task immediately. Events in transit may be lost.
    Force,
}

/// The contract every command fulfills.
///
/// A command takes context, sends events through a channel,
/// and returns a final report. Args are part of `&self`.
pub trait Command: Send + Sync {
    /// The final result — structured facts about what happened.
    type Report: Renderable + Send;
    /// Progressive events emitted during execution.
    type Event: Renderable + Send;

    /// Execute the command.
    ///
    /// - `ctx`: the environment (project, tools, containment, interrupted flag)
    /// - `events`: channel for progressive events
    fn execute(
        &self,
        ctx: &CommandContext,
        events: EventChannel<Self::Event>,
    ) -> impl Future<Output = Result<Self::Report, CommandError>> + Send;
}

/// The environment available to every command.
///
/// Built by the dispatch layer from the CLI/MCP handler. Commands receive this
/// by reference — they observe, not own.
pub struct CommandContext {
    /// Shared interruption flag — set by Ctrl+C, MCP cancel, or transport failure.
    pub interrupted: &'static AtomicBool,
    /// Containment root for filesystem operations.
    pub containment: Option<o8v_fs::ContainmentRoot>,
    /// Detected stack, if applicable.
    pub stack: Option<o8v_project::Stack>,
    /// Detected project root, if applicable.
    pub project_root: Option<o8v_project::ProjectRoot>,
}
