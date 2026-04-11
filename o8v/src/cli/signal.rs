// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Signal handling — Ctrl+C / SIGTERM handler installation.

use std::sync::atomic::{AtomicBool, Ordering};

/// Install a Ctrl+C / SIGTERM handler.
///
/// - First signal: sets `interrupted` flag, prints a message to stderr.
///   The check loop sees the flag and stops after the current check finishes.
/// - Second signal: force-exits with code 130 (SIGINT convention).
pub(crate) fn install_signal_handler(interrupted: &'static AtomicBool) {
    let result = ctrlc::set_handler(move || {
        // Use raw write(2) — eprintln! can deadlock if the signal arrives
        // while the main thread holds a heap lock or I/O mutex.
        if interrupted.swap(true, Ordering::Release) {
            // Second signal — force exit. process::exit bypasses Drop,
            // intentional: user hit Ctrl+C twice, they want out NOW.
            let _ = write_stderr(b"\nforce exit\n");
            std::process::exit(130);
        }
        let _ = write_stderr(b"\ninterrupted, cleaning up...\n");
    });

    if let Err(e) = result {
        tracing::warn!(error = %e, "could not install signal handler");
    }
}

/// Write to stderr from the signal handler thread.
/// NOT async-signal-safe (acquires stderr lock), but ctrlc runs handlers
/// in a dedicated thread, not a signal context, so no deadlock.
fn write_stderr(msg: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    stderr.write_all(msg)?;
    stderr.flush()
}
