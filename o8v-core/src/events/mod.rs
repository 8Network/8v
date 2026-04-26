// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Progressive event types for streaming commands.
//!
//! Each event implements Renderable so the framework can render events
//! as they arrive, per audience.

pub mod benchmark;
pub mod build;
pub mod check;
pub mod fmt;
pub mod lifecycle;
pub mod run;
pub mod test;
pub mod upgrade;

pub use benchmark::{BenchmarkRunFinished, BenchmarkRunStarted};
pub use lifecycle::{CommandCompleted, CommandStarted};

/// A typed event read back from the event store.
#[derive(Debug, Clone)]
pub enum Event {
    CommandStarted(lifecycle::CommandStarted),
    CommandCompleted(lifecycle::CommandCompleted),
    /// Event type not recognized — forward compatibility with newer 8v versions.
    /// Stores the full raw payload so it can be forwarded or logged without data loss.
    Unknown {
        event_type: String,
        raw: serde_json::Value,
    },
}
