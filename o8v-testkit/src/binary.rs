// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary execution — build and run the `8v` CLI binary in tests.

use crate::fixture::Fixture;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct CargoBuildTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct CargoBuildMessage {
    reason: String,
    executable: Option<String>,
    target: Option<CargoBuildTarget>,
}

/// Return the path to the compiled `8v` binary.
///
/// Runs `cargo build -p o8v --message-format=json` to get the
/// actual binary path from Cargo, respecting custom target directories
/// and platform-specific naming.
///
/// # Panics
/// Panics if the binary cannot be built or found.
#[must_use]
pub fn bin_path() -> PathBuf {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    let output = std::process::Command::new("cargo")
        .args(["build", "-p", "o8v", "--message-format=json"])
        .current_dir(&workspace)
        .stderr(std::process::Stdio::inherit())
        .output()
        .expect("failed to run cargo build");
    assert!(output.status.success(), "cargo build -p o8v failed");

    // Parse cargo's JSON messages to find the binary path.
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Ok(msg) = serde_json::from_str::<CargoBuildMessage>(line) {
            if msg.reason == "compiler-artifact" {
                if let Some(target) = &msg.target {
                    if target.name == "8v" && target.kind.contains(&"bin".to_string()) {
                        if let Some(exe) = msg.executable {
                            return PathBuf::from(exe);
                        }
                    }
                }
            }
        }
    }

    // Fallback to conventional path
    let debug = workspace.join("target/debug/8v");
    assert!(debug.exists(), "8v binary not found: {}", debug.display());
    debug
}

/// Run `8v check` on a fixture directory. Returns the process output.
///
/// # Panics
/// Panics if the binary cannot be executed.
#[must_use]
pub fn run_bin(fixture: &Fixture, extra_args: &[&str]) -> std::process::Output {
    let bin = bin_path();
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("check")
        .args(extra_args)
        .arg(fixture.path())
        .env("NO_COLOR", "1");

    // Extend PATH with common tool locations so tools like staticcheck
    // (installed via `go install`) are found by the subprocess.
    if let Ok(current_path) = std::env::var("PATH") {
        if let Some(home) = std::env::var_os("HOME") {
            let go_bin = std::path::Path::new(&home).join("go/bin");
            let new_path = format!("{}:{current_path}", go_bin.display());
            cmd.env("PATH", new_path);
        }
    }

    cmd.output().expect("failed to run 8v binary")
}
