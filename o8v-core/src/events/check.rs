// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for check command.

/// A progressive check event — emitted as each tool finishes.
pub enum StreamCheckEvent {
    /// A project was detected and checking begins.
    ProjectStart {
        name: String,
        stack: String,
        path: String,
    },
    /// A single check tool finished.
    ToolDone {
        name: String,
        outcome: String,
        duration_ms: u64,
        diagnostic_count: usize,
    },
    /// A detection error occurred.
    DetectionError { message: String },
}
