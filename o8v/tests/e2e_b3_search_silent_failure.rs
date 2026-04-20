// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Failing-first E2E tests for slice B3 — `8v search` silent-failure (BR-03, BR-06).
//!
//! These tests MUST FAIL before the implementation changes land.
//! The four bugs under test:
//! - BR-03: chmod-000 files silently skipped, stderr empty, indistinguishable from no-match.
//! - BR-06: binary (NUL-byte) files silently skipped without incrementing `files_skipped`.
//!
//! CE-2 discriminant:
//!   exit 0                        = ≥1 match, no I/O errors
//!   exit 1 + stderr empty         = 0 matches, no I/O errors (clean no-match)
//!   exit 1 + stderr non-empty     = partial I/O failure (0 or ≥1 matches)

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

/// Returns true when the current process is running as root (uid 0).
/// Uses `id -u` to avoid a dependency on the `users` crate.
fn running_as_root() -> bool {
    match Command::new("id").arg("-u").output() {
        Ok(o) => match String::from_utf8(o.stdout) {
            Ok(s) => s.trim() == "0",
            Err(_) => false,
        },
        Err(_) => false,
    }
}

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
    assert!(
        out.status.success(),
        "8v init must succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

// ─── BR-03: permission-denied stderr ─────────────────────────────────────────

/// BR-03: A chmod-000 file must produce a non-empty stderr warning.
///
/// Before fix: exit 1, stderr empty — indistinguishable from a clean no-match.
/// After fix:  exit 1 (or 0 if other files match), stderr contains
///             "error: search: permission_denied: <filename>".
#[test]
fn search_emits_stderr_warning_on_permission_denied() {
    // macOS restricts chmod 000 behavior when run as root, but CI runs as non-root.
    // Skip the test on root to avoid false-positive pass.
    if running_as_root() {
        eprintln!("skip: running as root, chmod 000 has no effect");
        return;
    }

    let dir = init_temp_workspace();

    // Create a readable file so the walk visits at least one file.
    let readable = dir.path().join("readable.txt");
    let mut f = std::fs::File::create(&readable).expect("create readable");
    writeln!(f, "no match here").unwrap();
    drop(f);

    // Create a file that will be permission-denied.
    let denied = dir.path().join("denied.txt");
    let mut f = std::fs::File::create(&denied).expect("create denied");
    writeln!(f, "pattern").unwrap();
    drop(f);
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).expect("chmod 000");

    let out = bin()
        .args(["search", "pattern"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");

    // Restore so tempdir cleanup can delete the file.
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644))
        .expect("restore permissions");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr must be non-empty when a file is permission-denied; got empty\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        stderr.contains("permission_denied") || stderr.contains("denied.txt"),
        "stderr must mention permission_denied or the denied file; got: {stderr}"
    );
}

/// CE-2 discriminant regression: exit 1 with zero matches must keep stderr EMPTY.
///
/// This test guards the CE-2 contract: if nothing went wrong, don't emit anything
/// to stderr. A future code path writing to stderr on a clean no-match would break
/// the agent's ability to distinguish partial failure from genuine no-match.
#[test]
fn search_stderr_empty_on_clean_no_match() {
    let dir = init_temp_workspace();

    let haystack = dir.path().join("haystack.txt");
    let mut f = std::fs::File::create(&haystack).expect("create haystack");
    writeln!(f, "apple banana cherry").unwrap();
    drop(f);

    let out = bin()
        .args(["search", "xyzzy_pattern_that_does_not_exist"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");

    // Must exit non-zero (no match).
    assert_ne!(
        out.status.code(),
        Some(0),
        "clean no-match must exit non-zero\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.is_empty(),
        "clean no-match must produce empty stderr (CE-2 discriminant); got: {stderr}"
    );
}

// ─── BR-06: binary files tracked in files_skipped_by_reason ─────────────────

/// BR-06: A binary (NUL-byte) file must be reflected in the JSON
/// `files_skipped_by_reason` map under key "binary".
///
/// Before fix: binary files are dropped silently; `files_skipped` = 0,
///             `files_skipped_by_reason` absent from JSON.
/// After fix:  `files_skipped_by_reason["binary"] >= 1` in JSON output.
#[test]
fn search_files_skipped_by_reason_populated_for_binary() {
    let dir = init_temp_workspace();

    // Write a binary file containing NUL bytes.
    let binary = dir.path().join("binary.bin");
    std::fs::write(&binary, b"hello\x00world\npattern\x00match\n").expect("write binary file");

    // Write a plain text file with the same pattern so search has a reason to run.
    let plain = dir.path().join("plain.txt");
    let mut f = std::fs::File::create(&plain).expect("create plain");
    writeln!(f, "no match here").unwrap();
    drop(f);

    let out = bin()
        .args(["search", "pattern", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON; error: {e}\nstdout: {stdout}"),
    };

    let by_reason = json.get("files_skipped_by_reason").unwrap_or_else(|| {
        panic!("JSON must contain 'files_skipped_by_reason' key; got: {stdout}")
    });

    let binary_count = by_reason
        .get("binary")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    assert!(
        binary_count >= 1,
        "files_skipped_by_reason[\"binary\"] must be >= 1; got: {by_reason}"
    );
}

/// BR-03 JSON path: permission-denied files must appear in `files_skipped_by_reason`
/// under key "permission_denied".
#[test]
fn search_files_skipped_by_reason_populated_for_permission_denied() {
    if running_as_root() {
        eprintln!("skip: running as root, chmod 000 has no effect");
        return;
    }

    let dir = init_temp_workspace();

    // Create a readable file so the walk has something to visit.
    let readable = dir.path().join("readable.txt");
    let mut f = std::fs::File::create(&readable).expect("create readable");
    writeln!(f, "no match here").unwrap();
    drop(f);

    // Create a permission-denied file.
    let denied = dir.path().join("denied2.txt");
    let mut f = std::fs::File::create(&denied).expect("create denied2");
    writeln!(f, "pattern").unwrap();
    drop(f);
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).expect("chmod 000");

    let out = bin()
        .args(["search", "pattern", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search --json");

    // Restore before assertions so cleanup works.
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644))
        .expect("restore permissions");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON; error: {e}\nstdout: {stdout}"),
    };

    let by_reason = json.get("files_skipped_by_reason").unwrap_or_else(|| {
        panic!("JSON must contain 'files_skipped_by_reason' key; got: {stdout}")
    });

    let pd_count = by_reason
        .get("permission_denied")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    assert!(
        pd_count >= 1,
        "files_skipped_by_reason[\"permission_denied\"] must be >= 1; got: {by_reason}"
    );
}
