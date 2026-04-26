// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E test: `8v read <path>:<start>-<end> --json` uses canonical field names.
//!
//! Contract: the JSON output for a range read is:
//!   `{"Range":{"path":"...","start":<usize>,"end":<usize>,"total_lines":<usize>,"lines":[...]}}`
//!
//! Field names MUST be `"start"` and `"end"` (NOT `"start_line"`/`"end_line"`).
//! The variant key MUST be `"Range"` (not `"range"` or `"RangeRead"`).

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

/// `8v read file.txt:2-4 --json` must produce a `Range` variant with `start`
/// and `end` fields — not `start_line`/`end_line`.
#[test]
fn range_json_uses_start_and_end_field_names() {
    let dir = init_temp_workspace();
    // Five-line file so :2-4 is a valid range.
    std::fs::write(
        dir.path().join("sample.txt"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    let out = bin()
        .args(["read", "sample.txt:2-4", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v read sample.txt:2-4 --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "expected exit 0\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\nstdout: {stdout}"),
    };

    // Top-level key must be "Range".
    let range = v.get("Range").unwrap_or_else(|| {
        panic!("expected top-level key 'Range' in JSON output\nstdout: {stdout}")
    });

    // Must have "start" field (not "start_line").
    assert!(
        range.get("start").is_some(),
        "missing 'start' field in Range object\nstdout: {stdout}"
    );
    assert!(
        range.get("start_line").is_none(),
        "unexpected 'start_line' field — canonical name is 'start'\nstdout: {stdout}"
    );

    // Must have "end" field (not "end_line").
    assert!(
        range.get("end").is_some(),
        "missing 'end' field in Range object\nstdout: {stdout}"
    );
    assert!(
        range.get("end_line").is_none(),
        "unexpected 'end_line' field — canonical name is 'end'\nstdout: {stdout}"
    );

    // Values must match the requested range.
    assert_eq!(
        range["start"].as_u64(),
        Some(2),
        "Range.start must equal 2\nstdout: {stdout}"
    );
    assert_eq!(
        range["end"].as_u64(),
        Some(4),
        "Range.end must equal 4\nstdout: {stdout}"
    );

    // Must also have "path", "total_lines", and "lines".
    assert!(
        range.get("path").is_some(),
        "missing 'path' field in Range object\nstdout: {stdout}"
    );
    assert!(
        range.get("total_lines").is_some(),
        "missing 'total_lines' field in Range object\nstdout: {stdout}"
    );
    assert!(
        range.get("lines").is_some(),
        "missing 'lines' field in Range object\nstdout: {stdout}"
    );
}
