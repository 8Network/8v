//! Tests for `ToolCheck` — the adapter that turns CLI tools into Checks.
//! Uses simple unix commands (true, false, echo, cat) as stand-ins for real tools.

use o8v_core::{Check, CheckContext, CheckOutcome};
use o8v_fs::ContainmentRoot;
use o8v_core::project::ProjectRoot;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

// ─── Helper ───────────────────────────────────────────────────────────────

fn project_dir() -> (tempfile::TempDir, ContainmentRoot) {
    let dir = tempfile::tempdir().unwrap();
    let path = ProjectRoot::new(dir.path()).unwrap();
    let containment = path.as_containment_root().unwrap();
    (dir, containment)
}

/// Default check context for tests — 5 minute timeout, not interrupted.
fn test_ctx() -> CheckContext {
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    CheckContext {
        timeout: Duration::from_secs(300),
        interrupted,
    }
}

// ─── Basic outcomes ───────────────────────────────────────────────────────

#[test]
fn passing_command_returns_passed() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    let check = o8v_stacks::ToolCheck::new("true", "true", &[]);
    assert!(
        matches!(check.run(&path, &ctx), CheckOutcome::Passed { .. }),
        "true command should produce Passed"
    );
}

#[test]
fn failing_command_returns_failed() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    let check = o8v_stacks::ToolCheck::new("false", "false", &[]);
    assert!(
        matches!(check.run(&path, &ctx), CheckOutcome::Failed { .. }),
        "false command should produce Failed"
    );
}

#[test]
fn nonexistent_command_returns_error() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    let check = o8v_stacks::ToolCheck::new("nope", "this-tool-does-not-exist-at-all", &[]);
    match check.run(&path, &ctx) {
        CheckOutcome::Error { cause, .. } => {
            assert!(cause.contains("could not run"), "cause: {cause}");
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

// ─── Output capture ──────────────────────────────────────────────────────

#[test]
fn failed_check_captures_output() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    // sh -c "echo error message >&2; exit 1" — fails with stderr output
    let check = o8v_stacks::ToolCheck::new(
        "fail-with-output",
        "sh",
        &["-c", "echo 'the error message' >&2; exit 1"],
    );
    match check.run(&path, &ctx) {
        CheckOutcome::Failed { raw_stderr, .. } => {
            assert!(
                raw_stderr.contains("the error message"),
                "output should contain the error: {raw_stderr}"
            );
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn failed_check_empty_output_is_still_failed() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    // Exit 42 with no output — should still be Failed (not Passed or Error)
    let check = o8v_stacks::ToolCheck::new("silent-fail", "sh", &["-c", "exit 42"]);
    match check.run(&path, &ctx) {
        CheckOutcome::Failed {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            assert!(raw_stdout.is_empty(), "no stdout expected: {raw_stdout}");
            assert!(raw_stderr.is_empty(), "no stderr expected: {raw_stderr}");
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

// ─── Pipe buffer deadlock (review finding #173) ──────────────────────────

#[test]
fn verbose_command_does_not_deadlock() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    // Generate 1MB+ of output — would deadlock if pipes aren't drained concurrently.
    let check = o8v_stacks::ToolCheck::new(
        "verbose",
        "sh",
        &[
            "-c",
            "dd if=/dev/zero bs=1024 count=200 2>/dev/null | cat; exit 1",
        ],
    );
    match check.run(&path, &ctx) {
        CheckOutcome::Failed { raw_stdout, .. } => {
            assert!(
                !raw_stdout.is_empty(),
                "should capture output from verbose command"
            );
        }
        other => panic!("expected Failed with output, got {other:?}"),
    }
}

// ─── Tool not found — spawn error, not heuristic ─────────────────────────

#[test]
fn tool_output_containing_not_found_is_still_failed() {
    // Review finding #175: "Definition for rule 'custom/not-found' was not found"
    // is a real lint error, not a tool-not-found. Heuristic was removed.
    // Only spawn() failure means tool missing.
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    let check = o8v_stacks::ToolCheck::new(
        "lint-with-not-found",
        "sh",
        &[
            "-c",
            "echo \"Definition for rule 'custom/not-found' was not found\" >&2; exit 1",
        ],
    );
    match check.run(&path, &ctx) {
        CheckOutcome::Failed { raw_stderr, .. } => {
            assert!(
                raw_stderr.contains("not found"),
                "real lint error should pass through: {raw_stderr}"
            );
        }
        other => panic!("expected Failed (not Error) for real lint output, got {other:?}"),
    }
}

// ─── Timeout (injectable — review finding #173) ──────────────────────────

#[test]
fn timeout_kills_slow_command() {
    use std::process::Command;
    use std::time::Duration;

    let (_dir, path) = project_dir();
    let mut cmd = Command::new("sleep");
    cmd.arg("60").current_dir(&path);

    let start = std::time::Instant::now();
    let outcome = o8v_stacks::run_tool(cmd, "slow", Duration::from_secs(1), test_ctx().interrupted);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "timeout should fire in ~1s, took {elapsed:?}"
    );
    match outcome {
        CheckOutcome::Error { cause, .. } => {
            assert!(cause.contains("timed out"), "cause: {cause}");
        }
        other => panic!("expected timeout Error, got {other:?}"),
    }
}

// ─── Background descendant does not block drain (review finding) ─────────

#[test]
fn background_grandchild_does_not_block() {
    use std::process::Command;
    use std::time::Duration;

    let (_dir, path) = project_dir();
    // Spawn a background grandchild that keeps stderr open for 30s, then exit 0.
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("(sleep 30; echo late >&2) & exit 0")
        .current_dir(&path);

    let start = std::time::Instant::now();
    let outcome = o8v_stacks::run_tool(cmd, "bg", Duration::from_secs(5), test_ctx().interrupted);
    let elapsed = start.elapsed();

    // Should complete in ~2 seconds (1s drain timeout), not 30 seconds.
    assert!(
        elapsed < Duration::from_secs(5),
        "should not wait for background grandchild, took {elapsed:?}"
    );
    assert!(
        matches!(outcome, CheckOutcome::Passed { .. }),
        "direct child exited 0, should be Passed: {outcome:?}"
    );
}

// ─── stdin closed — tool must not prompt (review finding) ────────────────

#[test]
fn tool_cannot_read_stdin() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    // "read" from stdin should fail immediately since stdin is /dev/null
    let check =
        o8v_stacks::ToolCheck::new("stdin-reader", "sh", &["-c", "read x; echo got:$x; exit 0"]);
    // Should complete without hanging — stdin is closed, read fails/returns empty
    let outcome = check.run(&path, &ctx);
    // It either passes (read got empty) or fails — but it does NOT hang
    assert!(
        matches!(
            outcome,
            CheckOutcome::Passed { .. } | CheckOutcome::Failed { .. }
        ),
        "should not hang waiting for stdin: {outcome:?}"
    );
}

// ─── Signal death = Error, not Failed (review finding) ───────────────────

#[test]
fn signal_death_is_error_not_failed() {
    let (_dir, path) = project_dir();
    let ctx = test_ctx();
    // kill -SEGV $$ — process dies by signal, no exit code
    let check = o8v_stacks::ToolCheck::new("crasher", "sh", &["-c", "kill -SEGV $$"]);
    match check.run(&path, &ctx) {
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("killed by signal"),
                "should report signal death: {cause}"
            );
        }
        other => panic!("expected Error for signal death, got {other:?}"),
    }
}

// ─── Output preserved with background descendant (review finding #182) ───

#[test]
fn early_output_preserved_with_background_grandchild() {
    use std::process::Command;
    use std::time::Duration;

    let (_dir, path) = project_dir();
    // Direct child prints to stderr and exits 1.
    // Background grandchild keeps stderr open for 5 seconds.
    // The direct child's output must NOT be lost.
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("echo immediate-error >&2; (sleep 5; echo late >&2) & exit 1")
        .current_dir(&path);

    let outcome = o8v_stacks::run_tool(
        cmd,
        "early-output",
        Duration::from_secs(3),
        test_ctx().interrupted,
    );

    match &outcome {
        CheckOutcome::Failed { raw_stderr, .. } => {
            assert!(
                raw_stderr.contains("immediate-error"),
                "direct child's stderr must be captured, got: {raw_stderr}"
            );
        }
        other => panic!("expected Failed with output, got {other:?}"),
    }
}

// ─── Output truncation (>10MB default cap) ──────────────────────────────

#[test]
fn output_truncated_past_cap() {
    use std::process::Command;
    use std::time::Duration;

    let (_dir, path) = project_dir();
    // Generate >10MB on BOTH stdout and stderr — exceeds the default 10MB cap.
    // dd writes 11MB (11264 * 1024 bytes) per stream.
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("dd if=/dev/zero bs=1024 count=11264 2>/dev/null; dd if=/dev/zero bs=1024 count=11264 >&2 2>/dev/null; exit 1")
        .current_dir(&path);

    let outcome = o8v_stacks::run_tool(
        cmd,
        "big-output",
        Duration::from_secs(30),
        test_ctx().interrupted,
    );
    match outcome {
        CheckOutcome::Failed {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            let cap = o8v_process::DEFAULT_MAX_OUTPUT;
            assert!(
                raw_stdout.len() <= cap + 64, // small slack for truncation
                "stdout should be capped near {cap}, got {}",
                raw_stdout.len()
            );
            assert!(
                raw_stderr.len() <= cap + 64,
                "stderr should be capped near {cap}, got {}",
                raw_stderr.len()
            );
        }
        other => panic!("expected Failed with truncated output, got {other:?}"),
    }
}

// ─── Check name ──────────────────────────────────────────────────────────

#[test]
fn check_name_matches_construction() {
    let check = o8v_stacks::ToolCheck::new("my-check", "true", &[]);
    assert_eq!(check.name(), "my-check");
}
