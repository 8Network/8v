// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for build command.

/// A progressive build event — compiler output lines.
pub enum BuildEvent {
    /// A line of output from the build tool.
    OutputLine {
        line: String,
        stream: super::test::OutputStream,
    },
}
