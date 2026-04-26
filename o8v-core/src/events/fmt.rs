// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for fmt command.

/// A progressive fmt event — emitted per formatter result.
pub enum FmtEvent {
    /// A formatter finished on a project.
    Done {
        stack: String,
        tool: String,
        status: String,
        duration_ms: u64,
    },
}
