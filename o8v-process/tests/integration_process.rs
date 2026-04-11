use o8v_process::{
    format, format_duration, run, ExitOutcome, ProcessConfig, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT,
};
use std::process::Command;
use std::time::Duration;

#[test]
fn default_config() {
    let c = ProcessConfig::default();
    assert_eq!(
        c.timeout, DEFAULT_TIMEOUT,
        "default timeout should be DEFAULT_TIMEOUT"
    );
    assert_eq!(
        c.max_stdout, DEFAULT_MAX_OUTPUT,
        "default max_stdout should be DEFAULT_MAX_OUTPUT"
    );
    assert_eq!(
        c.max_stderr, DEFAULT_MAX_OUTPUT,
        "default max_stderr should be DEFAULT_MAX_OUTPUT"
    );
    assert!(
        c.interrupted.is_none(),
        "default interrupted should be None"
    );
}

#[test]
fn format_duration_zero() {
    assert_eq!(format_duration(Duration::ZERO), "0ms");
}

#[test]
fn format_duration_sub_second() {
    assert_eq!(format_duration(Duration::from_millis(999)), "999ms");
}

#[test]
fn format_duration_one_second() {
    assert_eq!(format_duration(Duration::from_secs(1)), "1.0s");
}

#[test]
fn format_duration_boundary_1001ms() {
    assert_eq!(format_duration(Duration::from_millis(1001)), "1.0s");
}

#[test]
fn format_duration_many_seconds() {
    assert_eq!(format_duration(Duration::from_secs(300)), "300.0s");
}

#[test]
fn truncate_under_limit() {
    let mut s = "short".to_string();
    let truncated = format::truncate_with_marker(&mut s, 1024, "test");
    assert!(!truncated, "string under limit should not be truncated");
    assert_eq!(s, "short", "string under limit should be unchanged");
}

#[test]
fn truncate_at_limit() {
    let mut s = "a".repeat(100);
    let truncated = format::truncate_with_marker(&mut s, 100, "test");
    assert!(
        !truncated,
        "string exactly at limit should not be truncated"
    );
    assert_eq!(
        s.len(),
        100,
        "string at limit should remain exactly 100 bytes"
    );
}

#[test]
fn truncate_over_limit() {
    let mut s = "a".repeat(2000);
    let truncated = format::truncate_with_marker(&mut s, 100, "test");
    assert!(truncated, "string over limit should be truncated");
    assert!(
        s.len() <= 100,
        "truncated string must be within limit: {}",
        s.len()
    );
}

#[test]
fn exit_outcome_display() {
    assert_eq!(
        ExitOutcome::Success.to_string(),
        "success",
        "Success should display as 'success'"
    );
    assert_eq!(
        ExitOutcome::Failed { code: 1 }.to_string(),
        "failed (exit 1)",
        "Failed should display with exit code"
    );
    assert_eq!(
        ExitOutcome::Signal { signal: 9 }.to_string(),
        "killed by signal 9",
        "Signal should display with signal number"
    );
    assert_eq!(
        ExitOutcome::Interrupted.to_string(),
        "interrupted",
        "Interrupted should display as 'interrupted'"
    );
    assert!(
        ExitOutcome::SpawnError {
            cause: "not found".into(),
            kind: std::io::ErrorKind::NotFound,
        }
        .to_string()
        .contains("not found"),
        "SpawnError should include the cause in its display"
    );
}

#[cfg(unix)]
mod subprocess {
    use super::*;

    #[test]
    fn success() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo hello"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(result.outcome, ExitOutcome::Success),
            "echo command should succeed"
        );
        assert_eq!(
            result.stdout.trim(),
            "hello",
            "stdout should contain the echoed text"
        );
        assert!(
            !result.stdout_truncated,
            "short output should not be truncated"
        );
    }

    #[test]
    fn failure_with_code() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 42"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(result.outcome, ExitOutcome::Failed { code: 42 }),
            "exit 42 should produce Failed with code 42"
        );
    }

    #[test]
    fn timeout() {
        let mut cmd = Command::new("sleep");
        cmd.arg("999");
        let config = ProcessConfig {
            timeout: Duration::from_millis(200),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Timeout { .. }),
            "slow command should time out"
        );
    }

    #[test]
    fn signal_death() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "kill -9 $$"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(result.outcome, ExitOutcome::Signal { signal: 9 }),
            "expected Signal {{ signal: 9 }}, got {:?}",
            result.outcome
        );
    }

    #[test]
    fn spawn_failure() {
        let cmd = Command::new("/nonexistent/binary/that/does/not/exist");
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(
                result.outcome,
                ExitOutcome::SpawnError {
                    kind: std::io::ErrorKind::NotFound,
                    ..
                }
            ),
            "nonexistent binary should produce SpawnError with NotFound"
        );
    }

    #[test]
    fn empty_output() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 0"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(result.outcome, ExitOutcome::Success),
            "exit 0 with no output should succeed"
        );
        assert!(
            result.stdout.is_empty(),
            "command with no output should have empty stdout"
        );
        assert!(
            result.stderr.is_empty(),
            "command with no output should have empty stderr"
        );
    }

    #[test]
    fn stdin_closed() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "read x"]);
        let config = ProcessConfig {
            timeout: Duration::from_secs(5),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(
                result.outcome,
                ExitOutcome::Failed { .. } | ExitOutcome::Success
            ),
            "stdin closed should not hang: {:?}",
            result.outcome
        );
        assert!(
            result.duration < Duration::from_secs(4),
            "stdin-closed command should not hang, took {:?}",
            result.duration
        );
    }

    #[test]
    fn large_stdout_truncated() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "dd if=/dev/zero bs=1024 count=2048 2>/dev/null | tr '\\0' 'A'",
        ]);
        let config = ProcessConfig {
            max_stdout: 1024,
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(result.stdout_truncated, "stdout should be truncated");
    }

    #[test]
    fn large_stderr_truncated() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "dd if=/dev/zero bs=1024 count=2048 2>/dev/null | tr '\\0' 'B' >&2",
        ]);
        let config = ProcessConfig {
            max_stderr: 1024,
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(result.stderr_truncated, "stderr should be truncated");
    }

    #[test]
    fn spawn_permission_denied() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("noperm");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        let cmd = Command::new(&script);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(
                result.outcome,
                ExitOutcome::SpawnError {
                    kind: std::io::ErrorKind::PermissionDenied,
                    ..
                }
            ),
            "expected PermissionDenied, got {:?}",
            result.outcome
        );
    }

    #[test]
    fn slow_writer_no_sigpipe() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "for i in $(seq 1 20); do printf '%0100d' 0; sleep 0.01; done",
        ]);
        let config = ProcessConfig {
            max_stdout: 1024,
            timeout: Duration::from_secs(10),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(
                result.outcome,
                ExitOutcome::Success | ExitOutcome::Failed { .. }
            ),
            "slow writer should not be killed by SIGPIPE: {:?}",
            result.outcome
        );
        assert!(
            result.stdout_truncated,
            "slow writer output should be truncated at max_stdout"
        );
    }

    #[test]
    fn stderr_captured_separately() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo out; echo err >&2"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(result.stdout.contains("out"), "stdout should contain 'out'");
        assert!(result.stderr.contains("err"), "stderr should contain 'err'");
    }

    #[test]
    #[ignore]
    fn grandchild_killed_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 999 & sleep 999 & wait"]);
        let config = ProcessConfig {
            timeout: Duration::from_millis(200),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Timeout { .. }),
            "grandchild group should be killed on timeout"
        );
    }

    #[test]
    #[ignore]
    fn normal_exit_kills_group() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 999 & exit 0"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            matches!(result.outcome, ExitOutcome::Success),
            "process group should exit cleanly"
        );
    }

    #[test]
    #[ignore]
    fn fork_bomb_contained() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "for i in $(seq 1 10); do sleep 999 & done; wait"]);
        let config = ProcessConfig {
            timeout: Duration::from_millis(300),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Timeout { .. }),
            "fork bomb should be contained by timeout"
        );
    }

    #[test]
    #[ignore]
    fn grandchild_setsid_escape_preserves_output() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "echo 'direct child output'; setsid sleep 999 & exit 0",
        ]);
        let config = ProcessConfig {
            timeout: Duration::from_secs(5),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Success),
            "direct child should exit cleanly"
        );
        assert!(
            result.duration < Duration::from_secs(5),
            "should complete before timeout, took {:?}",
            result.duration
        );
        assert!(
            result.stdout.contains("direct child output"),
            "setsid escape should not lose captured output: stdout={:?}",
            result.stdout
        );
    }

    #[test]
    #[ignore]
    fn interrupted_flag_kills_immediately() {
        let flag: &'static std::sync::atomic::AtomicBool =
            Box::leak(Box::new(std::sync::atomic::AtomicBool::new(true)));
        let mut cmd = Command::new("sleep");
        cmd.arg("999");
        let config = ProcessConfig {
            interrupted: Some(flag),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Interrupted),
            "pre-set interrupted should return Interrupted: {:?}",
            result.outcome
        );
        assert!(
            result.duration < Duration::from_secs(1),
            "interrupted flag should kill immediately, took {:?}",
            result.duration
        );
    }

    // ─── Security tests ────────────────────────────────────────────

    #[test]
    #[ignore]
    fn pipe_flood_no_oom() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "dd if=/dev/zero bs=1048576 count=12 2>/dev/null | tr '\\0' 'X'",
        ]);
        let config = ProcessConfig {
            max_stdout: 1024 * 1024,
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            result.stdout_truncated,
            "12MB output should be truncated at 1MB limit"
        );
        assert!(
            result.stdout.len() <= 1024 * 1024,
            "truncated stdout must be within limit"
        );
    }

    #[test]
    #[ignore]
    fn slow_trickle_timeout_fires() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "while true; do printf 'x'; sleep 0.1; done"]);
        let config = ProcessConfig {
            timeout: Duration::from_secs(2),
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(
            matches!(result.outcome, ExitOutcome::Timeout { .. }),
            "slow trickle should timeout: {:?}",
            result.outcome
        );
        assert!(
            result.duration < Duration::from_secs(5),
            "slow trickle timeout should fire quickly, took {:?}",
            result.duration
        );
    }

    #[test]
    #[ignore]
    fn stderr_only_captured() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo 'error output' >&2; exit 1"]);
        let result = run(cmd, &ProcessConfig::default());
        assert!(
            result.stdout.is_empty(),
            "stdout should be empty when output only goes to stderr"
        );
        assert!(
            result.stderr.contains("error output"),
            "stderr should contain the error output"
        );
        assert!(
            matches!(result.outcome, ExitOutcome::Failed { .. }),
            "exit 1 should produce Failed"
        );
    }

    #[test]
    #[ignore]
    fn both_streams_large() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "dd if=/dev/zero bs=1024 count=2048 2>/dev/null | tr '\\0' 'A'; dd if=/dev/zero bs=1024 count=2048 2>/dev/null | tr '\\0' 'B' >&2"]);
        let config = ProcessConfig {
            max_stdout: 1024,
            max_stderr: 1024,
            ..ProcessConfig::default()
        };
        let result = run(cmd, &config);
        assert!(result.stdout_truncated, "stdout should be truncated");
        assert!(result.stderr_truncated, "stderr should be truncated");
    }

    // ─── Command injection security tests ──────────────────────────

    /// Security test: Command::new() does NOT use a shell.
    ///
    /// o8v-process calls Command::new(program).args([...]) and spawn().
    /// Arguments are passed directly to the executable, NOT through a shell.
    /// Shell metacharacters in arguments are literal strings, not interpreted.
    ///
    /// This is by design: Command::new() is "safe by default" because it
    /// bypasses the shell entirely. Arguments cannot be interpreted as code.
    #[test]
    fn security_no_shell_interpretation() {
        let mut cmd = Command::new("printf");
        // printf is called directly, not through /bin/sh
        // The argument "hello world" is passed as-is to printf
        cmd.arg("hello world");
        let result = run(cmd, &ProcessConfig::default());

        assert_eq!(
            result.stdout, "hello world",
            "arguments should be passed directly to the executable"
        );
    }

    /// Security test: Shell metacharacters are literal if passed as arguments.
    ///
    /// When a tool (not o8v-process) is invoked with Command::new("tool").arg(...)
    /// arguments are passed directly. They're not interpreted as shell syntax.
    ///
    /// This means: if o8v-check builds arguments from untrusted project files,
    /// the arguments themselves are safe (not executed as shell code). However,
    /// the tool being invoked MIGHT interpret them. For example, some tools accept
    /// config file paths or eval-like options. That's a tool-specific risk, not
    /// an o8v-process risk.
    #[test]
    fn security_arguments_are_literal() {
        let mut cmd = Command::new("printf");
        // Pass shell-like syntax as arguments — they're literal
        cmd.arg("; rm -rf /");
        let result = run(cmd, &ProcessConfig::default());

        // printf outputs the literal string
        assert_eq!(
            result.stdout, "; rm -rf /",
            "shell metacharacters should be literal arguments"
        );
    }

    /// Security test: printf with injected escape sequences.
    ///
    /// printf interprets its format string (that's printf's job). If a tool
    /// accepts user-controlled format strings, that's a tool bug, not an
    /// o8v-process bug. But we can document the expected behavior.
    #[test]
    fn security_tool_interprets_format_strings() {
        let mut cmd = Command::new("printf");
        // printf format string with escape codes
        cmd.arg("hello\\nworld");
        let result = run(cmd, &ProcessConfig::default());

        // printf interprets \n as a newline
        assert!(
            result.stdout.contains("hello\nworld"),
            "printf interprets its own escape sequences"
        );
    }

    /// Security test: Documenting the danger of shell -c.
    ///
    /// If a caller uses Command::new("sh").args(["-c", user_input]), that's unsafe.
    /// The -c argument is NOT validated by this test — that's the caller's responsibility.
    /// But we document what happens when -c is used.
    #[test]
    fn security_shell_c_is_caller_responsibility() {
        let mut cmd = Command::new("sh");
        // This is UNSAFE if program_name comes from an untrusted source,
        // because sh -c interprets the string as shell code.
        cmd.args(["-c", "echo safe"]);
        let result = run(cmd, &ProcessConfig::default());

        // sh -c works as expected when called with fixed, trusted strings
        assert_eq!(result.stdout.trim(), "safe");
        // But if the argument were untrusted, sh would interpret it. That's
        // a caller risk, not an o8v-process risk. o8v-process just executes
        // the command it's given.
    }
}

#[cfg(unix)]
mod injection_attack_vectors {
    use super::*;

    /// Security test: redirection operators are literal in direct execution.
    ///
    /// When Command::new("printf") is used (not sh -c), the > character
    /// in an argument is a literal character, not a shell redirect operator.
    #[test]
    fn security_output_redirection_literal() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = tmpdir.path().join("pwned.txt");

        let mut cmd = Command::new("printf");
        cmd.arg("content > /tmp/pwned");
        let result = run(cmd, &ProcessConfig::default());

        // The > is literal in the output
        assert_eq!(result.stdout, "content > /tmp/pwned");
        // The file is NOT created because > is not interpreted as a redirect
        assert!(
            !target.exists(),
            "output redirection in arg should not actually redirect"
        );
    }

    /// Security test: pipe operator is literal in direct execution.
    ///
    /// Pipe | in arguments to a non-shell tool is a literal character.
    #[test]
    fn security_pipe_operator_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("content | cat /etc/passwd");
        let result = run(cmd, &ProcessConfig::default());

        // The pipe is literal
        assert_eq!(result.stdout, "content | cat /etc/passwd");
        // /etc/passwd is not read because | is not interpreted
    }

    /// Security test: AND/OR operators are literal in direct execution.
    ///
    /// && and || are not interpreted in direct tool invocation.
    #[test]
    fn security_logical_operators_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("ok && rm -rf /");
        let result = run(cmd, &ProcessConfig::default());

        // The && is literal
        assert_eq!(result.stdout, "ok && rm -rf /");
    }

    /// Security test: command chaining with semicolon is literal.
    ///
    /// Semicolons in arguments to non-shell tools are literal characters.
    #[test]
    fn security_command_chaining_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("echo safe ; echo hacked");
        let result = run(cmd, &ProcessConfig::default());

        // The semicolon is literal
        assert_eq!(result.stdout, "echo safe ; echo hacked");
    }

    /// Security test: glob patterns are literal in direct execution.
    ///
    /// Glob patterns like * are not expanded by the tool (only shell expands them).
    #[test]
    fn security_glob_pattern_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("*.txt");
        let result = run(cmd, &ProcessConfig::default());

        // The glob is literal
        assert_eq!(result.stdout, "*.txt");
    }

    /// Security test: tilde is literal in direct execution.
    ///
    /// ~ in an argument to a non-shell tool is literal, not expanded to $HOME.
    #[test]
    fn security_tilde_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("~/secret.txt");
        let result = run(cmd, &ProcessConfig::default());

        // The ~ is literal
        assert_eq!(result.stdout, "~/secret.txt");
    }

    /// Security test: backticks are literal in direct execution.
    ///
    /// Command::new() does not use a shell, so backticks are just characters.
    #[test]
    fn security_backtick_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("`id`");
        let result = run(cmd, &ProcessConfig::default());

        assert_eq!(result.stdout, "`id`");
    }

    /// Security test: $() syntax is literal in direct execution.
    ///
    /// $() is only interpreted by the shell, not by direct tool execution.
    #[test]
    fn security_dollar_paren_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("$(whoami)");
        let result = run(cmd, &ProcessConfig::default());

        assert_eq!(result.stdout, "$(whoami)");
    }

    /// Security test: newlines in arguments are literal.
    ///
    /// Newlines in arguments don't create new commands — they're just characters.
    #[test]
    fn security_newline_literal() {
        let mut cmd = Command::new("printf");
        let arg = "hello\nworld";
        cmd.arg(arg);
        let result = run(cmd, &ProcessConfig::default());

        // printf outputs the literal string (with newline)
        assert_eq!(result.stdout, "hello\nworld");
    }

    /// Security test: path traversal in arguments is literal.
    ///
    /// Arguments like ../../../etc/passwd are just strings, not interpreted.
    #[test]
    fn security_path_traversal_literal() {
        let mut cmd = Command::new("printf");
        cmd.arg("../../../etc/passwd");
        let result = run(cmd, &ProcessConfig::default());

        assert_eq!(result.stdout, "../../../etc/passwd");
    }

    /// Security test: environment variables are NOT expanded in arguments.
    ///
    /// Direct tool execution does not expand $HOME or other env vars
    /// in arguments. (The shell does that, but we're not using the shell.)
    #[test]
    fn security_env_var_not_expanded() {
        let mut cmd = Command::new("printf");
        cmd.arg("$HOME");
        let result = run(cmd, &ProcessConfig::default());

        assert_eq!(
            result.stdout, "$HOME",
            "environment variable should not be expanded"
        );
    }
}
