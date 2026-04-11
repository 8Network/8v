//! Process group kill — platform-specific cleanup.

/// Kill the entire process group (Unix) or just the child (other).
///
/// Called ALWAYS after the poll loop — even on normal exit — to kill
/// surviving grandchildren and close their pipe ends (bug #184).
pub(crate) fn kill_process_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // SAFETY: child.id() returns the PID of a process we spawned (always > 0).
        // Negating it targets the process group. cast_signed() produces positive i32
        // (max PID is well below i32::MAX). Negating positive i32 cannot overflow.
        let ret =
            unsafe { libc::kill(-libc::pid_t::from(child.id().cast_signed()), libc::SIGKILL) };
        // ESRCH = no such process — expected when child already exited.
        // EPERM = shouldn't happen for our own children — warn.
        if ret != 0 {
            let errno = std::io::Error::last_os_error();
            if errno.raw_os_error() != Some(libc::ESRCH) {
                tracing::warn!(error = %errno, "killpg returned unexpected error");
            }
        }

        // Belt-and-suspenders fallback: send SIGKILL to the specific PID.
        // If child called setsid(), killpg may have missed it. child.kill()
        // sends SIGKILL directly to the PID regardless of process group.
        let _ = child.kill();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

/// Classify an exit status into an `ExitOutcome`.
pub(crate) fn classify_exit(status: std::process::ExitStatus) -> crate::ExitOutcome {
    use crate::ExitOutcome;

    if status.success() {
        return ExitOutcome::Success;
    }

    if let Some(code) = status.code() {
        return ExitOutcome::Failed { code };
    }

    // No exit code → killed by signal (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let signal = status.signal().unwrap_or(0);
        ExitOutcome::Signal { signal }
    }

    #[cfg(not(unix))]
    ExitOutcome::Failed { code: -1 }
}
