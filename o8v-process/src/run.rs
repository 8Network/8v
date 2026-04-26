//! Core execution pipeline — the `run()` function.

use crate::capture::{collect_pair, spawn_capture};
use crate::config::ProcessConfig;
use crate::format::truncate_with_marker;
use crate::kill::{classify_exit, kill_process_group};
use crate::result::{ExitOutcome, ProcessResult};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Run a command safely. Handles all known subprocess failure modes.
///
/// **Callers set program, args, env, and `current_dir` on the `Command`.
/// `run()` unconditionally overrides stdin, stdout, stderr, and process_group.**
///
/// Emits `tracing` events:
/// - `info_span!("run_process")` for the full lifecycle
/// - `debug!` on spawn (with PID), output collected, group killed
/// - `warn!` on timeout, pipe read failure, drain thread timeout, truncation
/// - `error!` on spawn failure
pub fn run(mut cmd: Command, config: &ProcessConfig) -> ProcessResult {
    let _span = tracing::info_span!("run_process").entered();
    let start = Instant::now();

    // Step 1: stdin(null), stdout(piped), stderr(piped)
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Step 2: process_group(0) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    // Step 3: Spawn
    let mut child = match cmd.spawn() {
        Ok(c) => {
            tracing::debug!(pid = c.id(), "spawned");
            c
        }
        Err(e) => {
            tracing::error!(error = %e, "spawn failed");
            return ProcessResult {
                outcome: ExitOutcome::SpawnError {
                    cause: format!("{e}"),
                    kind: e.kind(),
                },
                stdout: String::new(),
                stderr: String::new(),
                duration: start.elapsed(),
                stdout_truncated: false,
                stderr_truncated: false,
            };
        }
    };

    // Step 4: Drain stdout/stderr in threads CONCURRENTLY with wait.
    // Uses shared buffers (Arc<Mutex>) so captured data survives drain thread hangs.
    let stdout_capture = spawn_capture(child.stdout.take(), config.max_stdout);
    let stderr_capture = spawn_capture(child.stderr.take(), config.max_stderr);

    // Step 5: Poll try_wait() against deadline
    let deadline = start + config.timeout;
    let poll_result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break PollResult::Exited(status),
            Ok(None) => {
                if let Some(flag) = config.interrupted {
                    if flag.load(Ordering::Acquire) {
                        tracing::warn!("interrupted — killing process group");
                        break PollResult::Interrupted;
                    }
                }
                if Instant::now() >= deadline {
                    tracing::warn!(timeout = ?config.timeout, "timeout reached, killing process group");
                    break PollResult::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                tracing::warn!(error = %e, "try_wait failed");
                break PollResult::WaitError(e);
            }
        }
    };

    // Step 6: Kill entire process group — ALWAYS, even on normal exit.
    kill_process_group(&mut child);

    // Step 7: Reap zombie with bounded try_wait loop.
    // After killpg, poll try_wait() up to ~20 times with 50ms sleeps (1 second total).
    // Prevents forever-hang if killpg missed the child. If still alive after 1s,
    // call wait() anyway (will likely return quickly since we sent SIGKILL).
    let mut reaped = false;
    for _ in 0..20 {
        match child.try_wait() {
            Ok(Some(_)) => {
                reaped = true;
                break;
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                tracing::warn!(error = %e, "try_wait failed during reap");
                break;
            }
        }
    }

    // If child still hasn't exited after 1 second, call wait() as fallback.
    if !reaped {
        tracing::warn!("child did not exit after 1s, calling wait() fallback");
        match child.wait() {
            Ok(_) => tracing::debug!("process group killed and reaped (fallback)"),
            Err(e) => tracing::warn!(error = %e, "failed to reap zombie"),
        }
    } else {
        tracing::debug!("process group killed and reaped");
    }

    // Measure duration BEFORE drain wait — the drain timeout is 8v overhead,
    // not the tool's execution time. Prevents inflating reported elapsed.
    let duration = start.elapsed();

    // Step 8: Collect drain thread results with a SHARED deadline.
    // Both stdout and stderr drain threads share the same wall-clock deadline,
    // not separate sequential timeouts. This prevents double-dipping the timeout
    // budget: if stdout drain takes 1.5s, stderr drain only gets 0.5s.
    // Each drain computes remaining_time = deadline.saturating_duration_since(now()),
    // so the total elapsed cannot exceed the drain_budget regardless of how the
    // time is split between them.
    let drain_budget = Duration::from_secs(2);
    let captured_pair = collect_pair(stdout_capture, stderr_capture, drain_budget);

    tracing::debug!(
        stdout_bytes = captured_pair.stdout.data.len(),
        stderr_bytes = captured_pair.stderr.data.len(),
        "output collected"
    );

    // Step 9: Convert to UTF-8 lossy + truncate with marker.
    let mut stdout = String::from_utf8_lossy(&captured_pair.stdout.data).into_owned();
    let mut stderr = String::from_utf8_lossy(&captured_pair.stderr.data).into_owned();

    let stdout_truncated = captured_pair.stdout.truncated
        || truncate_with_marker(&mut stdout, config.max_stdout, "stdout");
    let stderr_truncated = captured_pair.stderr.truncated
        || truncate_with_marker(&mut stderr, config.max_stderr, "stderr");

    // Step 10: Classify exit status → ExitOutcome
    let outcome = match poll_result {
        PollResult::Exited(status) => classify_exit(status),
        PollResult::TimedOut => ExitOutcome::Timeout { elapsed: duration },
        PollResult::Interrupted => ExitOutcome::Interrupted,
        PollResult::WaitError(e) => ExitOutcome::WaitError {
            cause: format!("{e}"),
        },
    };

    ProcessResult {
        outcome,
        stdout,
        stderr,
        duration,
        stdout_truncated,
        stderr_truncated,
    }
}

/// Internal poll loop result — before classification into ExitOutcome.
enum PollResult {
    Exited(std::process::ExitStatus),
    TimedOut,
    Interrupted,
    WaitError(std::io::Error),
}
