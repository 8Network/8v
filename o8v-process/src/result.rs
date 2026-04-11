//! Process execution result types.

use crate::format::format_duration;
use std::time::Duration;

/// How the process exited.
///
/// Stack-agnostic: the caller interprets what Success/Failed mean for their
/// domain (e.g., go vet exits 0 with findings — the caller promotes that).
#[derive(Debug)]
#[non_exhaustive]
pub enum ExitOutcome {
    /// Exit code 0.
    Success,
    /// Non-zero exit code.
    Failed { code: i32 },
    /// Killed by signal (Unix only). No exit code.
    Signal { signal: i32 },
    /// Exceeded timeout. Process group was killed.
    Timeout { elapsed: Duration },
    /// Could not spawn. Preserves `io::ErrorKind` for caller classification.
    SpawnError {
        cause: String,
        kind: std::io::ErrorKind,
    },
    /// Interrupted by caller (via `AtomicBool` flag).
    Interrupted,
    /// OS error during `try_wait` — rare (PID recycled, external reap).
    WaitError { cause: String },
}

impl std::fmt::Display for ExitOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => f.write_str("success"),
            Self::Failed { code } => write!(f, "failed (exit {code})"),
            Self::Signal { signal } => write!(f, "killed by signal {signal}"),
            Self::Timeout { elapsed } => write!(f, "timed out after {}", format_duration(*elapsed)),
            Self::SpawnError { cause, .. } => write!(f, "spawn error: {cause}"),
            Self::Interrupted => f.write_str("interrupted"),
            Self::WaitError { cause } => write!(f, "wait error: {cause}"),
        }
    }
}

/// Complete result of running a process.
#[derive(Debug)]
#[must_use]
pub struct ProcessResult {
    /// How the process exited.
    pub outcome: ExitOutcome,
    /// Captured stdout (up to `max_stdout`, UTF-8 lossy).
    pub stdout: String,
    /// Captured stderr (up to `max_stderr`, UTF-8 lossy).
    pub stderr: String,
    /// Wall-clock duration from spawn to reap.
    pub duration: Duration,
    /// True if stdout was truncated at the capture limit.
    pub stdout_truncated: bool,
    /// True if stderr was truncated at the capture limit.
    pub stderr_truncated: bool,
}
