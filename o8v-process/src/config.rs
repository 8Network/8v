//! Process execution configuration.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Default timeout: 5 minutes.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Default capture limit per stream: 10 MB.
///
/// Tools like ruff and eslint produce single JSON arrays — truncation mid-array
/// destroys all diagnostics. 1 MB was too small for real projects (ruff on
/// psf/black produces ~3 MB). 10 MB handles large monorepos with room to spare.
/// Memory impact: 20 MB peak per subprocess (2 streams × 10 MB), one at a time.
pub const DEFAULT_MAX_OUTPUT: usize = 10 * 1024 * 1024;

/// Configuration for safe process execution.
///
/// Has a sensible `Default`: 5 min timeout, 10 MB capture per stream.
pub struct ProcessConfig {
    /// Maximum time before the process group is killed.
    pub timeout: Duration,
    /// Maximum bytes captured from stdout.
    pub max_stdout: usize,
    /// Maximum bytes captured from stderr.
    pub max_stderr: usize,
    /// Optional interruption flag. Checked during the poll loop.
    /// When set, treated as a timeout (kills group, returns `Interrupted`).
    /// The caller owns signal registration — this crate never registers
    /// process-global handlers.
    ///
    /// **Warning:** If the flag is already `true` when `run()` is called,
    /// the process is spawned and immediately killed. The caller must clear
    /// the flag before each `run()` call if reusing it across invocations.
    pub interrupted: Option<&'static AtomicBool>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_stdout: DEFAULT_MAX_OUTPUT,
            max_stderr: DEFAULT_MAX_OUTPUT,
            interrupted: None,
        }
    }
}
