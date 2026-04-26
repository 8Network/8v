// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for run command.

/// A progressive run event — stdout/stderr lines as they arrive.
pub enum RunEvent {
    /// A line of output from the running process.
    OutputLine {
        line: String,
        stream: super::test::OutputStream,
    },
}
