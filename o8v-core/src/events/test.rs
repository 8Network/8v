// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming events for test command.

/// A progressive test event — stdout/stderr lines as they arrive.
pub enum TestEvent {
    /// A line of output from the test runner.
    OutputLine { line: String, stream: OutputStream },
}

/// Which output stream the line came from.
pub enum OutputStream {
    Stdout,
    Stderr,
}
