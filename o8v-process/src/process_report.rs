// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the MIT License. See LICENSE file in this crate's directory.

use std::time::Duration;

/// Human-readable exit label for plain output.
pub fn exit_label(outcome: &crate::ExitOutcome) -> String {
    match outcome {
        crate::ExitOutcome::Success => "0 (success)".to_string(),
        crate::ExitOutcome::Failed { code } => format!("{code} (failed)"),
        crate::ExitOutcome::Timeout { elapsed } => {
            format!("timeout after {}", crate::format_duration(*elapsed))
        }
        crate::ExitOutcome::SpawnError { cause, .. } => {
            format!("spawn error: {cause}")
        }
        crate::ExitOutcome::Interrupted => "interrupted".to_string(),
        crate::ExitOutcome::Signal { signal } => format!("signal {signal}"),
        crate::ExitOutcome::WaitError { cause } => format!("wait error: {cause}"),
    }
}

/// Numeric exit code for JSON output.
pub fn exit_code_number(outcome: &crate::ExitOutcome) -> i32 {
    match outcome {
        crate::ExitOutcome::Success => 0,
        crate::ExitOutcome::Failed { code } => *code,
        crate::ExitOutcome::Timeout { .. } => 124,
        crate::ExitOutcome::Interrupted => 130,
        crate::ExitOutcome::SpawnError { .. } => 1,
        crate::ExitOutcome::Signal { signal } => 128 + signal,
        crate::ExitOutcome::WaitError { .. } => 1,
    }
}

/// Structured interpretation of a process execution.
/// Shared by test, build, and run reports.
#[derive(Debug, Clone)]
pub struct ProcessReport {
    /// The command that was run (from call site, not from ProcessResult).
    pub command: String,
    /// Numeric exit code.
    pub exit_code: i32,
    /// Whether the process succeeded (exit code 0).
    pub success: bool,
    /// Human-readable exit label, e.g. "0 (success)", "1 (failed)", "timeout after 5s".
    pub exit_label: String,
    /// How long the process ran.
    pub duration: Duration,
    /// How long the process ran, formatted for display, e.g. "1.23s".
    pub duration_display: String,
    /// Captured stdout (may be truncated).
    pub stdout: String,
    /// Captured stderr (may be truncated).
    pub stderr: String,
    /// Whether stdout was truncated.
    pub stdout_truncated: bool,
    /// Whether stderr was truncated.
    pub stderr_truncated: bool,
}
