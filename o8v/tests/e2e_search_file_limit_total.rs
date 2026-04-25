// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E test: `8v search PATTERN . --files --limit 2` with 5 matching files must
//! report "5" (total matching files) in the summary line, not "4".
//!
//! BUG-2: When the N+1th file gets added to `result.files` then trimmed to empty
//! and popped, `total_files` is decremented even though the file did match.
//! Files processed after that via `count_file_matches` only add 3 more, giving
//! total_files=4 instead of 5.

use std::process::{Command, Stdio};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

fn init_temp_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = bin()
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v init --yes");
    assert!(out.status.success(), "8v init failed: {:?}", out);
    dir
}

/// `8v search NEEDLE . --files --limit 2` with 5 matching files must report "5"
/// as the total file count in the summary line.
#[test]
fn search_files_limit_total_is_correct() {
    let dir = init_temp_workspace();

    // Create 5 files each containing "NEEDLE_UNIQUE_TOKEN"
    for i in 1..=5 {
        std::fs::write(
            dir.path().join(format!("file{i}.txt")),
            format!("line with NEEDLE_UNIQUE_TOKEN in file{i}\n"),
        )
        .unwrap();
    }

    let out = bin()
        .args([
            "search",
            "NEEDLE_UNIQUE_TOKEN",
            ".",
            "--files",
            "--limit",
            "2",
        ])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "expected exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The summary must reference all 5 matching files, not 4.
    assert!(
        stdout.contains('5'),
        "expected '5' in stdout summary (total files = 5)\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Double-check the buggy value is not the only number present.
    // "Found 4 files" would be the pre-fix output.
    assert!(
        !stdout.contains("Found 4 files"),
        "stdout must not say 'Found 4 files' — that's the pre-fix bug\nstdout: {stdout}"
    );
}
