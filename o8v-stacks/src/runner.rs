//! Thin wrapper around `o8v_process::run()`.
//!
//! Converts `ProcessResult` → `CheckOutcome` so stacks don't need to know
//! about o8v-process types. All tool execution goes through `run_tool`.

use o8v_core::diagnostic::ParseStatus;
use o8v_core::{CheckOutcome, ErrorKind};
use o8v_process::{ExitOutcome, ProcessConfig, ProcessResult};
use std::process::Command;
use std::time::Duration;

/// Run a command and return a `CheckOutcome`.
///
/// Delegates to `o8v_process::run()` and converts the neutral `ProcessResult`
/// into the check-specific `CheckOutcome`.
///
/// The `name` is used in tracing spans and error messages.
#[must_use]
pub fn run_tool(
    cmd: Command,
    name: &str,
    timeout: Duration,
    interrupted: &'static std::sync::atomic::AtomicBool,
) -> CheckOutcome {
    let _span = tracing::info_span!("run_tool", tool = name).entered();
    let config = ProcessConfig {
        timeout,
        interrupted: Some(interrupted),
        ..ProcessConfig::default()
    };
    let result = o8v_process::run(cmd, &config);
    process_result_to_outcome(result, name)
}

/// Convert a `ProcessResult` into a `CheckOutcome`.
///
/// Used by `run_tool` and by stacks that call `o8v_process::run()` directly
/// (e.g. NodeToolCheck inspects SpawnError kind before conversion).
pub(crate) fn process_result_to_outcome(result: ProcessResult, name: &str) -> CheckOutcome {
    let parse_status = ParseStatus::Unparsed;

    match result.outcome {
        ExitOutcome::Success => CheckOutcome::passed(
            result.stdout,
            result.stderr,
            parse_status,
            result.stdout_truncated,
            result.stderr_truncated,
        ),
        ExitOutcome::Failed { code } => CheckOutcome::failed(
            Some(code),
            Vec::new(),
            result.stdout,
            result.stderr,
            parse_status,
            result.stdout_truncated,
            result.stderr_truncated,
        ),
        ExitOutcome::Signal { signal } => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!("'{name}' was killed by signal {signal}"),
            result.stdout,
            result.stderr,
        ),
        ExitOutcome::Timeout { elapsed } => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!(
                "'{name}' timed out after {}",
                o8v_process::format_duration(elapsed)
            ),
            result.stdout,
            result.stderr,
        ),
        ExitOutcome::SpawnError { cause, kind } => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!("could not run '{name}': {cause} ({kind:?})"),
            result.stdout,
            result.stderr,
        ),
        ExitOutcome::Interrupted => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!("'{name}' interrupted by user"),
            result.stdout,
            result.stderr,
        ),
        ExitOutcome::WaitError { cause } => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!("failed waiting for '{name}': {cause}"),
            result.stdout,
            result.stderr,
        ),
        #[allow(unreachable_patterns)] // non_exhaustive: future variants
        _ => CheckOutcome::error_with_output(
            ErrorKind::Runtime,
            format!("'{name}' exited with unknown outcome"),
            result.stdout,
            result.stderr,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_process::{ExitOutcome, ProcessResult};
    use std::time::Duration;

    fn make_result(outcome: ExitOutcome) -> ProcessResult {
        ProcessResult {
            outcome,
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            duration: Duration::from_millis(100),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn success_maps_to_passed() {
        let r = process_result_to_outcome(make_result(ExitOutcome::Success), "test");
        assert!(matches!(r, CheckOutcome::Passed { .. }));
    }

    #[test]
    fn failed_maps_with_code() {
        let r = process_result_to_outcome(make_result(ExitOutcome::Failed { code: 42 }), "test");
        match r {
            CheckOutcome::Failed { code, .. } => assert_eq!(code, Some(42)),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn signal_maps_to_error() {
        let r = process_result_to_outcome(make_result(ExitOutcome::Signal { signal: 11 }), "test");
        match r {
            CheckOutcome::Error { cause, .. } => assert!(cause.contains("signal 11")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn timeout_maps_to_error() {
        let r = process_result_to_outcome(
            make_result(ExitOutcome::Timeout {
                elapsed: Duration::from_secs(5),
            }),
            "test",
        );
        match r {
            CheckOutcome::Error { cause, .. } => assert!(cause.contains("timed out")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn spawn_error_maps_to_error() {
        let r = process_result_to_outcome(
            make_result(ExitOutcome::SpawnError {
                cause: "not found".into(),
                kind: std::io::ErrorKind::NotFound,
            }),
            "test",
        );
        match r {
            CheckOutcome::Error { cause, .. } => assert!(cause.contains("not found")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn interrupted_maps_to_error() {
        let r = process_result_to_outcome(make_result(ExitOutcome::Interrupted), "test");
        match r {
            CheckOutcome::Error { cause, .. } => assert!(cause.contains("interrupted")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn wait_error_maps_to_error() {
        let r = process_result_to_outcome(
            make_result(ExitOutcome::WaitError {
                cause: "os error".into(),
            }),
            "test",
        );
        match r {
            CheckOutcome::Error { cause, .. } => assert!(cause.contains("os error")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn truncation_booleans_propagate() {
        let result = ProcessResult {
            outcome: ExitOutcome::Success,
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            duration: Duration::from_millis(100),
            stdout_truncated: true,
            stderr_truncated: true,
        };
        let r = process_result_to_outcome(result, "test");
        match r {
            CheckOutcome::Passed {
                stdout_truncated,
                stderr_truncated,
                ..
            } => {
                assert!(stdout_truncated);
                assert!(stderr_truncated);
            }
            other => panic!("expected Passed, got {other:?}"),
        }
    }
}
