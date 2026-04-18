// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Signal handling — application infrastructure.

use std::sync::atomic::{AtomicBool, Ordering};

/// Install a Ctrl+C / SIGTERM handler.
///
/// - First signal: sets `interrupted` flag, prints a message to stderr.
///   The check loop sees the flag and stops after the current check finishes.
/// - Second signal: force-exits with code 130 (SIGINT convention).
pub fn install(interrupted: &'static AtomicBool) {
    let result = ctrlc::set_handler(move || {
        if interrupted.swap(true, Ordering::Release) {
            let _ = write_stderr(b"\nforce exit\n");
            std::process::exit(130);
        }
        let _ = write_stderr(b"\ninterrupted, cleaning up...\n");
    });

    if let Err(e) = result {
        tracing::warn!(error = %e, "could not install signal handler");
    }
}

fn write_stderr(msg: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    stderr.write_all(msg)?;
    stderr.flush()
}
