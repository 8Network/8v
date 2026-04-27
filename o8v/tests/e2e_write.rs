// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Integration tests for `8v write` — full pipeline against real files.
//!
//! Each test runs the compiled binary with `.current_dir(tmp.path())` where
//! `tmp` contains `Cargo.toml`. This satisfies `WorkspaceRoot` resolution in
//! `build_context`, which requires `resolve_workspace(CWD)` to succeed.

use std::process::Command;

fn bin_in(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_8v"));
    cmd.current_dir(dir);
    cmd
}

/// Create a minimal project root so `WorkspaceRoot` resolves from CWD.
fn setup_project(tmp: &tempfile::TempDir) {
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
}

// ─── ReplaceLine ─────────────────────────────────────────────────────────────

/// Baseline: single-line replace of line 2 in a 4-line file.
#[test]
fn replace_line_single_line() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "replaced"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nreplaced\nline3\nline4\n");
}

/// Regression for Bug 1: ReplaceLine with multi-line content must expand into
/// multiple lines, not embed a raw newline in a single entry.
#[test]
fn replace_line_multiline_content() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "new_a\nnew_b\nnew_c"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nnew_a\nnew_b\nnew_c\nline3\nline4\n");
}

// ─── ReplaceRange ────────────────────────────────────────────────────────────

/// Regression for Bug 1: ReplaceRange with multi-line content must expand into
/// multiple lines.
#[test]
fn replace_range_multiline_content() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2-3", "new_a\nnew_b\nnew_c"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nnew_a\nnew_b\nnew_c\nline4\n");
}

/// ReplaceRange with single-line content (baseline, not a regression).
#[test]
fn replace_range_single_line_content() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2-3", "replaced"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nreplaced\nline4\n");
}

// ─── DeleteLines ─────────────────────────────────────────────────────────────

/// Delete lines 2–3 from a 4-line file.
#[test]
fn delete_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2-3", "--delete"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nline4\n");
}

// ─── InsertBefore ────────────────────────────────────────────────────────────

/// Regression for Bug 1: InsertBefore with multi-line content must insert each
/// line individually, not embed raw newlines.
#[test]
fn insert_before_multiline() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "--insert", "ins_a\nins_b"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\nins_a\nins_b\nline2\nline3\n");
}

// ─── FindReplace ─────────────────────────────────────────────────────────────

/// FindReplace without --all on N>1 matches must fail (silent partial-replace footgun).
#[test]
fn find_replace_first_occurrence() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo bar\nfoo baz\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "foo", "--replace", "qux"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "expected non-zero exit when N>1 matches without --all\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--all"),
        "error must mention --all\nstderr: {stderr}"
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "foo bar\nfoo baz\n", "file must not be modified");
}

/// FindReplace with --all replaces every occurrence.
#[test]
fn find_replace_all() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo bar\nfoo baz\n").unwrap();

    let out = bin_in(tmp.path())
        .args([
            "write",
            "src.txt",
            "--find",
            "foo",
            "--replace",
            "qux",
            "--all",
        ])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "qux bar\nqux baz\n");
}

// ─── AppendToFile ────────────────────────────────────────────────────────────

/// Append content to an existing file.
#[test]
fn append_to_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--append", "line3"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    // BUG-1 fix: append always ensures trailing newline.
    assert_eq!(result, "line1\nline2\nline3\n");
}

// ─── CreateFile ──────────────────────────────────────────────────────────────

/// CreateFile writes a new file when it does not yet exist.
#[test]
fn create_new_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("new.txt");
    assert!(!file.exists(), "file must not exist before test");

    let out = bin_in(tmp.path())
        .args(["write", "new.txt", "hello world"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "hello world");
}

/// CreateFile when file already exists returns an error with the improved
/// message that guides the agent to the correct alternatives.
#[test]
fn create_file_already_exists_error() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("existing.txt");
    std::fs::write(&file, "original content\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "existing.txt", "new content"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero when file exists"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("already exists"),
        "error must mention 'already exists'\ngot: {combined}"
    );
    assert!(
        combined.contains("--force"),
        "error must mention '--force'\ngot: {combined}"
    );
    assert!(
        combined.contains("<start>-<end>"),
        "error must mention range alternative ':start-end'\ngot: {combined}"
    );
    assert!(
        combined.contains("--find") || combined.contains("find"),
        "error must mention find/replace alternative\ngot: {combined}"
    );
    // Verify original file is untouched.
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "original content\n");
}

/// CreateFile with --force overwrites an existing file.
#[test]
fn create_file_force_overwrite() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("existing.txt");
    std::fs::write(&file, "original content\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "existing.txt", "new content", "--force"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0 with --force\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert!(
        result.contains("new content"),
        "file must contain new content after --force"
    );
    assert!(
        !result.contains("original content"),
        "original content must be replaced"
    );
}

// ─── Regression tests ────────────────────────────────────────────────────────

/// CE3: Empty content is rejected for ReplaceLine.
#[test]
fn empty_content_rejected_on_replace_line() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", ""])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for empty content"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("cannot be empty"),
        "error must mention 'cannot be empty'\ngot: {combined}"
    );
}

/// CE3: Empty content is rejected for ReplaceRange.
#[test]
fn empty_content_rejected_on_replace_range() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2-3", ""])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for empty content"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("cannot be empty"),
        "error must mention 'cannot be empty'\ngot: {combined}"
    );
}

/// CE17: Empty content is rejected for InsertBefore.
#[test]
fn empty_content_rejected_on_insert() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "--insert", ""])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for empty insert content"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("cannot be empty"),
        "error must mention 'cannot be empty'\ngot: {combined}"
    );
}

/// Lone-\r (classic Mac) files are rejected for line-based operations.
#[test]
fn lone_cr_file_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    // Write a file with only \r line endings (classic Mac).
    std::fs::write(&file, b"line1\rline2\rline3\r").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for lone-CR file"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("classic Mac line endings"),
        "error must mention 'classic Mac line endings'\ngot: {combined}"
    );
}

/// Mixed LF+CRLF files are rejected for line-based operations.
#[test]
fn mixed_line_endings_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    // Write a file with mixed LF and CRLF endings.
    std::fs::write(&file, b"line1\nline2\r\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for mixed line endings"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("mixed line endings"),
        "error must mention 'mixed line endings'\ngot: {combined}"
    );
}

/// CE14: --append on a nonexistent file gives a helpful error.
#[test]
fn append_nonexistent_file_has_helpful_error() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);

    let out = bin_in(tmp.path())
        .args(["write", "nonexistent.txt", "--append", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero when appending to nonexistent file"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("not found"),
        "error must mention 'not found'\ngot: {combined}"
    );
    assert!(
        combined.contains("not found"),
        "error must indicate file not found\ngot: {combined}"
    );
}

/// parse_path_line: inverted range (end < start) is rejected.
#[test]
fn invalid_range_end_before_start_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:5-2", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for inverted range"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("start must be <= end"),
        "error must mention 'start must be <= end'\ngot: {combined}"
    );
}

/// CRLF file: line replacement preserves CRLF endings byte-exactly.
#[test]
fn crlf_file_byte_exact_preservation() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    // Write a file with CRLF endings only.
    std::fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "replaced"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0 for CRLF file\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nreplaced\r\nline3\r\n",
        "CRLF endings must be preserved byte-exactly"
    );
}

/// FindReplace: no match is a hard error (non-zero exit, stderr tells agent
/// how to recover). Silent no-match was causing agents to believe the write
/// succeeded and move on without the edit being applied.
#[test]
fn find_replace_no_match_is_error() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo bar\nbaz qux\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "nothere", "--replace", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "no-match find-replace must fail (non-zero exit)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("no matches found"),
        "error must state no matches found\ngot: {combined}"
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "foo bar\nbaz qux\n", "file must be unchanged");
}

// ─── BREAKING-1 trailing-blank-line regression tests ─────────────────────────

/// BREAKING-1: Replace line 2 with "\n" (a single blank line) — must produce
/// one blank line, not delete the line.
#[test]
fn replace_with_blank_line_preserved() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "\n"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\n\nline3\n");
}

/// BREAKING-1: Replace line 2 with "\n\n" — must produce two blank lines.
#[test]
fn replace_with_multi_blank_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "\n\n"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\n\n\nline3\n");
}

/// BREAKING-1: Replace line 2 with "a\nb\n" — trailing newline is terminator,
/// not a third blank line. Produces exactly two lines: "a" and "b".
#[test]
fn replace_multiline_content_with_trailing_newline() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "a\nb\n"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\na\nb\nline3\n");
}

/// BREAKING-1: Replace line 2 with "a\n\nb" — blank middle line must be preserved.
#[test]
fn replace_multiline_content_with_blank_middle() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "a\n\nb"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "line1\na\n\nb\nline3\n");
}

// ─── content line-ending validation ────────────────────────────────────────

/// Content with embedded lone \r is rejected.
#[test]
fn content_with_embedded_lone_cr_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "A\rB"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for content with lone \\r"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("\\n line endings only") || combined.contains("do not include"),
        "error must mention line-ending restriction\ngot: {combined}"
    );
}

/// Content with mixed CRLF+LF endings is rejected.
#[test]
fn content_with_mixed_endings_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "A\r\nB\nC"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for content with mixed CRLF+LF"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("\\n line endings only") || combined.contains("do not include"),
        "error must mention line-ending restriction\ngot: {combined}"
    );
}

/// Content with pure CRLF endings is rejected — was silently
/// allowed by the old validator, causing \r to be written into LF files.
#[test]
fn content_with_crlf_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "A\r\nB"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for content with CRLF"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("\\n line endings only") || combined.contains("do not include"),
        "error must mention line-ending restriction\ngot: {combined}"
    );
    // File must not be modified
    let contents = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        contents, "line1\nline2\nline3\n",
        "file must be unchanged after rejected write"
    );
}

// ─── mid-line \r in a \n-terminated file ──────────────────────────────────

/// A file containing "A\nB\rC\nD\n" (mid-line \r, not part of \r\n)
/// must be rejected — not silently corrupted.
#[test]
fn mid_line_cr_file_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"A\nB\rC\nD\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "x"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for file with mid-line \\r"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("carriage return"),
        "error must mention 'carriage return'\ngot: {combined}"
    );
}

// ─── CreateFile empty content ──────────────────────────────────────────────

/// CreateFile with empty content is rejected with a clear error.
#[test]
fn create_file_empty_content_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("new.txt");
    assert!(!file.exists(), "file must not exist before test");

    let out = bin_in(tmp.path())
        .args(["write", "new.txt", ""])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "should exit non-zero for empty CreateFile content"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("cannot be empty"),
        "error must mention 'cannot be empty'\ngot: {combined}"
    );
    assert!(!file.exists(), "file must not be created on error");
}

// ─── CRLF + blank lines byte-exact round-trip ────────────────────────────────

/// CRLF file with blank lines: replacement must preserve CRLF byte-exactly
/// including any blank lines introduced by content_to_lines.
#[test]
fn crlf_file_byte_exact_preservation_with_blank_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    // File: "line1\r\nline2\r\nline3\r\n"
    std::fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

    // Replace line 2 with two lines "new_a\nnew_b" (LF content, CRLF file)
    let out = bin_in(tmp.path())
        .args(["write", "src.txt:2", "new_a\nnew_b"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nnew_a\r\nnew_b\r\nline3\r\n",
        "CRLF endings must be preserved byte-exactly including injected lines"
    );
}

/// Append to an LF file without trailing terminator uses \n separator.
#[test]
fn append_to_lf_file_without_trailing_newline_uses_lf() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"a\nb").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--append", "c"])
        .output()
        .expect("run 8v write");

    assert!(out.status.success(), "append should succeed");
    let result = std::fs::read(&file).unwrap();
    assert_eq!(result, b"a\nb\nc\n");
}

// ─── --find --replace validates strings for \r ──────────────────────────────

/// Find/replace must reject a replacement containing \r — would contaminate
/// an LF file with stray carriage returns.
#[test]
fn find_replace_rejects_replacement_with_cr() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"hello world\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "hello", "--replace", "hi\r"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "replacement with \\r must be rejected"
    );
    // File must be unchanged
    assert_eq!(std::fs::read(&file).unwrap(), b"hello world\n");
}

/// Find/replace must also reject a find pattern containing \r.
#[test]
fn find_replace_rejects_find_pattern_with_cr() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"hello world\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "hello\r", "--replace", "hi"])
        .output()
        .expect("run 8v write");

    assert!(!out.status.success(), "find with \\r must be rejected");
    assert_eq!(std::fs::read(&file).unwrap(), b"hello world\n");
}

// ─── --append "" is rejected (no side effects on empty content) ──────────────

#[test]
fn append_with_empty_content_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"hello").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--append", ""])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "empty append must be rejected, got stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"hello",
        "file must be untouched on rejected append"
    );
}

// ─── F1 regression: absolute path must not leak in rendered output ────────────

/// Regression for F1: when an absolute path is passed (as MCP does after
/// resolve_mcp_paths), the rendered output header must show the relative path,
/// not the absolute path.
#[test]
fn write_absolute_path_renders_relative_in_header() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    // Simulate what MCP does: pass the absolute path directly to the binary.
    let abs_path = file.to_str().unwrap();
    let out = bin_in(tmp.path())
        .args(["write", &format!("{abs_path}:2"), "replaced"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The absolute path prefix of the temp dir must NOT appear in any header.
    let abs_prefix = tmp.path().to_str().unwrap();
    assert!(
        !stdout.contains(abs_prefix),
        "absolute path leaked in write output header:\n{stdout}"
    );
    // The relative filename must appear instead.
    assert!(
        stdout.contains("src.txt"),
        "relative filename missing from write output header:\n{stdout}"
    );
}

// ─── FindReplace multi-occurrence guard ──────────────────────────────────────

/// N=3 occurrences without --all must fail with a non-zero exit and mention --all.
#[test]
fn find_replace_multi_match_without_all_fails() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo\nfoo\nfoo\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "foo", "--replace", "bar"])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "expected non-zero exit when N>1 matches without --all\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("--all"),
        "error output must mention --all\ncombined: {combined}"
    );
    // File must be unchanged.
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "foo\nfoo\nfoo\n", "file must not be modified");
}

/// N=3 occurrences with --all must replace all, exit 0.
#[test]
fn find_replace_multi_match_with_all_replaces_all() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo\nfoo\nfoo\n").unwrap();

    let out = bin_in(tmp.path())
        .args([
            "write",
            "src.txt",
            "--find",
            "foo",
            "--replace",
            "bar",
            "--all",
        ])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "expected exit 0 with --all\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        content, "bar\nbar\nbar\n",
        "all occurrences must be replaced"
    );
}

/// N=1 occurrence without --all must still succeed (backward compat).
#[test]
fn find_replace_single_match_without_all_succeeds() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "hello\nworld\n").unwrap();

    let out = bin_in(tmp.path())
        .args([
            "write",
            "src.txt",
            "--find",
            "hello",
            "--replace",
            "goodbye",
        ])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "expected exit 0 for single match\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "goodbye\nworld\n", "single match must be replaced");
}

// ─── Concurrent append atomicity ─────────────────────────────────────────────

/// CE-APPEND-CONCURRENT: 50 concurrent `8v write --append` invocations on the
/// same file must all succeed and produce exactly 50 lines.
///
/// The race: write.rs reads the file to check for a trailing newline, then
/// appends. Two processes can both read before either writes, causing both
/// to prepend a separator — resulting in a merged line (double separator) or
/// a missing line. This test catches that regression.
#[test]
fn append_concurrent_50_all_lines_written() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("concurrent.txt");
    // Seed file with NO trailing newline — forces every concurrent append to go
    // through the separator path (needs_separator=true). This is where the TOCTOU
    // race lives: read → decide separator → append are three separate operations.
    // Two processes can both read before either writes, causing duplicate separators
    // and a merged/lost line.
    std::fs::write(&file, "seed").unwrap();

    let bin = env!("CARGO_BIN_EXE_8v");
    let dir = tmp.path().to_path_buf();
    let file_name = "concurrent.txt";
    let n = 50usize;

    // Barrier ensures all threads call `Command::output` simultaneously.
    let barrier = Arc::new(Barrier::new(n));
    let handles: Vec<_> = (0..n)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let bin = bin.to_string();
            let dir = dir.clone();
            thread::spawn(move || {
                barrier.wait();
                std::process::Command::new(&bin)
                    .current_dir(&dir)
                    .args(["write", file_name, "--append", &format!("line{i}")])
                    .output()
                    .expect("spawn 8v write")
                    .status
                    .success()
            })
        })
        .collect();

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let failures = results.iter().filter(|&&ok| !ok).count();
    assert_eq!(
        failures, 0,
        "{failures} / {n} concurrent appends exited non-zero"
    );

    let content = std::fs::read_to_string(&file).unwrap();
    // 1 seed + 50 appended = 51 lines. If the race fires, two processes prepend
    // the same separator and one line is merged into another, leaving < 51 lines.
    let line_count = content.lines().count();
    assert_eq!(
        line_count,
        n + 1,
        "expected {} lines (1 seed + {n} appends), got {line_count};\nfile:\n{content}",
        n + 1
    );
}

// ─── Diff output correctness ──────────────────────────────────────────────────

/// BUG A regression: every line of the new content must appear with `  + ` prefix.
/// Previously only the first line was prefixed; subsequent lines appeared as plain text.
#[test]
fn write_replace_range_diff_prefixes_all_new_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("f.txt");
    std::fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "f.txt:1-2", "X\nY\nZ"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("  + X"),
        "stdout must contain '  + X'; got:\n{stdout}"
    );
    assert!(
        stdout.contains("  + Y"),
        "stdout must contain '  + Y'; got:\n{stdout}"
    );
    assert!(
        stdout.contains("  + Z"),
        "stdout must contain '  + Z'; got:\n{stdout}"
    );
    // Bare Y or Z without the prefix must not appear as a standalone line.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed == "Y" || trimmed == "Z" {
            panic!("new content line appeared without '  + ' prefix; got line: {line:?}\nfull stdout:\n{stdout}");
        }
    }
}

// BUG-1 regression: --append must ensure the resulting file ends with a newline.
// If the appended content itself does not end with \n, one must be added.
#[test]
fn append_ensures_trailing_newline() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("target.txt");
    // Existing file ends with newline.
    std::fs::write(&file, "line1\n").unwrap();

    // Append content that does NOT end with \n.
    let out = bin_in(tmp.path())
        .args(["write", "target.txt", "--append", "no-newline"])
        .output()
        .expect("run 8v write --append");

    assert!(
        out.status.success(),
        "--append must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read_to_string(&file).unwrap();
    assert!(
        result.ends_with('\n'),
        "--append must ensure trailing newline; file content: {result:?}"
    );
    assert_eq!(
        result, "line1\nno-newline\n",
        "--append must produce expected content with trailing newline"
    );
}

// --- Issue #3: CRLF line-ending preservation gaps --------------------------
//
// Tests 1-3 verify the fixes for issue #3.
// Tests 4-5 are positive baselines (Insert and Delete already worked correctly).

/// Issue #3 Test 1 -- Append regression (f330d45):
/// Seeding a pure CRLF file then appending must produce CRLF endings,
/// not a trailing bare \n. Today --append hardcodes \n regardless of
/// the file line ending, so the file ends with `appended\n` breaking CRLF purity.
#[test]
fn append_to_crlf_file_preserves_crlf_endings() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf.txt");
    std::fs::write(&file, b"line1\r\nline2\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf.txt", "--append", "appended"])
        .output()
        .expect("run 8v write --append");

    assert!(
        out.status.success(),
        "--append on CRLF file must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    // The appended line must end with \r\n, not bare \n.
    assert_eq!(
        result, b"line1\r\nline2\r\nappended\r\n",
        "append to CRLF file must preserve CRLF; got bytes: {:?}",
        result
    );
}

/// Issue #3 Test 2 -- Append corrupts CRLF file for the next 8v op:
/// After appending to a CRLF file the file has mixed endings.
/// A subsequent 8v write on the file must succeed -- today it fails
/// because validate_line_endings rejects the now-mixed file.
#[test]
fn append_to_crlf_file_subsequent_write_succeeds() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf2.txt");
    std::fs::write(&file, b"line1\r\nline2\r\n").unwrap();

    // First op: append.
    let out1 = bin_in(tmp.path())
        .args(["write", "crlf2.txt", "--append", "appended"])
        .output()
        .expect("run 8v write --append");
    assert!(
        out1.status.success(),
        "first append must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    // Second op: replace line 1 -- must NOT be rejected due to mixed endings
    // introduced by the first append.
    let out2 = bin_in(tmp.path())
        .args(["write", "crlf2.txt:1", "replaced"])
        .output()
        .expect("run second 8v write");

    assert!(
        out2.status.success(),
        "second write after append must exit 0; file likely has mixed endings now\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out2.stdout),
        String::from_utf8_lossy(&out2.stderr)
    );
}

/// Issue #3 Test 3 -- FindReplace with \n in replacement on a CRLF file:
/// The replacement string "a\nb" must be written as "a\r\nb" (CRLF-normalised)
/// on a CRLF file. Today the raw \n is injected, creating mixed endings.
#[test]
fn find_replace_newline_in_replacement_on_crlf_file_preserves_crlf() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf3.txt");
    std::fs::write(&file, b"foo\r\nbar\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf3.txt", "--find", "foo", "--replace", "a\nb"])
        .output()
        .expect("run 8v write --find --replace");

    assert!(
        out.status.success(),
        "find-replace on CRLF file must exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    // After replacing "foo" with "a\nb", the file must remain pure CRLF.
    assert_eq!(
        result, b"a\r\nb\r\nbar\r\n",
        "find-replace must produce pure CRLF; got bytes: {:?}",
        result
    );
}

/// Issue #3 Test 4 -- Insert + CRLF positive baseline:
/// `8v write file:2 --insert "X"` on a CRLF file should already preserve
/// CRLF endings (Insert uses detect_line_ending). This documents that
/// Insert already works -- the gap is test coverage, not behaviour.
#[test]
fn insert_into_crlf_file_preserves_crlf_endings() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf4.txt");
    std::fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf4.txt:2", "--insert", "X"])
        .output()
        .expect("run 8v write --insert");

    assert!(
        out.status.success(),
        "insert into CRLF file must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nX\r\nline2\r\nline3\r\n",
        "insert into CRLF file must preserve CRLF endings byte-exactly; got: {:?}",
        result
    );
}

/// Issue #3 Test 5 -- Delete + CRLF positive baseline:
/// `8v write file:2-2 --delete` on a CRLF file should already preserve
/// CRLF endings (Delete uses detect_line_ending). Documents Delete already
/// works -- gap is test coverage, not behaviour.
#[test]
fn delete_from_crlf_file_preserves_crlf_endings() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf5.txt");
    std::fs::write(&file, b"line1\r\nline2\r\nline3\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf5.txt:2-2", "--delete"])
        .output()
        .expect("run 8v write --delete");

    assert!(
        out.status.success(),
        "delete from CRLF file must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nline3\r\n",
        "delete from CRLF file must preserve CRLF endings byte-exactly; got: {:?}",
        result
    );
}

// ─── PR #4 review reproducers ─────────────────────────────────────────────────
//
// Each test below pins a concrete finding from the three PR #4 review rounds.
// Tests marked #[ignore] exercise currently-broken behaviour; they will be
// un-ignored when the underlying bug is fixed.

#[allow(dead_code)]
fn assert_pure_crlf(bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            assert!(
                i > 0 && bytes[i - 1] == b'\r',
                "bare \\n at byte {} in {:?}",
                i,
                bytes
            );
        }
        if b == b'\r' {
            assert!(
                i + 1 < bytes.len() && bytes[i + 1] == b'\n',
                "lone \\r at byte {} in {:?}",
                i,
                bytes
            );
        }
    }
}

/// PR#4-R1: CRLF file without trailing newline -- append must produce pure CRLF.
/// Today `is_crlf` reads only the last 2 bytes; when the file ends without
/// `\r\n` those 2 bytes are the last chars of the final word, so detection
/// falls back to LF and the appended line gets a bare `\n` instead of `\r\n`.
#[test]
#[ignore = "bug: is_crlf detection fails when file has no trailing newline"]
fn append_to_crlf_file_without_trailing_newline_preserves_crlf() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf_no_trail.txt");
    std::fs::write(&file, b"line1\r\nline2\r\nno_trailing").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf_no_trail.txt", "--append", "appended"])
        .output()
        .expect("run 8v write --append");

    assert!(
        out.status.success(),
        "--append on CRLF file without trailing newline must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nline2\r\nno_trailing\r\nappended\r\n",
        "append to CRLF-without-trailing-newline must produce pure CRLF; got bytes: {:?}",
        result
    );
}

/// PR#4-R2: After appending to a CRLF file without trailing newline the file
/// ends up with mixed endings (due to bug above). A subsequent `8v write`
/// on that file must still succeed -- today it is rejected by validate_line_endings.
#[test]
#[ignore = "bug: post-append file has mixed endings, subsequent write is rejected"]
fn append_then_subsequent_write_succeeds_on_crlf_file_without_trailing_newline() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf_no_trail2.txt");
    std::fs::write(&file, b"line1\r\nline2\r\nno_trailing").unwrap();

    // First op: append.
    let out1 = bin_in(tmp.path())
        .args(["write", "crlf_no_trail2.txt", "--append", "appended"])
        .output()
        .expect("run 8v write --append");
    assert!(
        out1.status.success(),
        "first append must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    // Second op: replace line 1 -- must not be rejected due to mixed endings
    // left behind by the first (buggy) append.
    let out2 = bin_in(tmp.path())
        .args(["write", "crlf_no_trail2.txt:1", "replaced"])
        .output()
        .expect("run second 8v write");

    assert!(
        out2.status.success(),
        "second write after append must exit 0; file likely has mixed endings\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out2.stdout),
        String::from_utf8_lossy(&out2.stderr)
    );
}

/// PR#4-R3: Appending to an already-mixed file must be REJECTED.
/// Today `--append` skips validate_line_endings on the existing file content,
/// so it silently appends to a mixed file.
#[test]
#[ignore = "bug: append does not validate existing file for mixed endings"]
fn append_on_pre_existing_mixed_ending_file_is_rejected() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("mixed.txt");
    // Deliberately mixed: first line CRLF, second LF, third CRLF.
    std::fs::write(&file, b"line1\r\nline2\nline3\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "mixed.txt", "--append", "x"])
        .output()
        .expect("run 8v write --append");

    assert!(
        !out.status.success(),
        "append on mixed-ending file must fail\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.to_lowercase().contains("mixed"),
        "error must mention mixed line endings; got: {combined}"
    );
    // File must be untouched.
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"line1\r\nline2\nline3\r\n",
        "file must be unchanged on rejected append"
    );
}

/// PR#4-R4: Pin current behaviour for append with empty content on a CRLF
/// file that lacks a trailing newline. Empty content is already rejected
/// globally before any line-ending detection; this ensures that stays true.
#[test]
fn append_empty_content_on_crlf_file_without_trailing_newline() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf_no_trail3.txt");
    std::fs::write(&file, b"line1\r\nline2").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "crlf_no_trail3.txt", "--append", ""])
        .output()
        .expect("run 8v write --append");

    // Empty content is rejected before any line-ending processing.
    assert!(
        !out.status.success(),
        "empty append must be rejected\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // File must be untouched.
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"line1\r\nline2",
        "file must be unchanged on rejected empty append"
    );
}

/// PR#4-R5: Boundary case -- 2-byte CRLF-only file (b"\r\n").
/// `is_crlf` checks whether the last 2 bytes are b"\r\n"; this is that
/// exact boundary -- the entire file is one CRLF sequence.
#[test]
fn append_to_two_byte_crlf_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("two_byte_crlf.txt");
    std::fs::write(&file, b"\r\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "two_byte_crlf.txt", "--append", "x"])
        .output()
        .expect("run 8v write --append");

    assert!(
        out.status.success(),
        "--append on 2-byte CRLF file must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"\r\nx\r\n",
        "append to 2-byte CRLF file must produce pure CRLF; got: {:?}",
        result
    );
}

/// PR#4-R6: Boundary case -- single lone-CR byte (b"\r").
/// This is an invalid file; pin that the tool exits cleanly (no crash/abort).
#[test]
fn append_to_one_byte_lone_cr_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("lone_cr.txt");
    std::fs::write(&file, b"\r").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "lone_cr.txt", "--append", "x"])
        .output()
        .expect("run 8v write --append");

    // A lone-CR file is already invalid. The tool may reject it or fall back
    // to LF. Either outcome is acceptable. What must NOT happen is a panic or
    // unclean exit (signal). Pin that the process terminates normally.
    assert!(
        out.status.code().is_some(),
        "process must exit cleanly (no signal/abort); status: {:?}",
        out.status
    );
}

/// PR#4-R7: Empty file -- 0 bytes. No content means no line-ending detection
/// possible; must default to LF and produce b"x\n".
#[test]
fn append_to_empty_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("empty_append.txt");
    std::fs::write(&file, b"").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "empty_append.txt", "--append", "x"])
        .output()
        .expect("run 8v write --append");

    assert!(
        out.status.success(),
        "--append on empty file must exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"x\n",
        "append to empty file must default to LF; got: {:?}",
        result
    );
}

/// PR#4-R8: find-replace with a newline in the find pattern on a CRLF file
/// when there is NO match. The operation must fail; the error must reference
/// the user's original pattern, NOT the internally-normalised CRLF form.
#[test]
fn find_replace_with_newline_in_find_zero_matches_on_crlf_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf_find.txt");
    std::fs::write(&file, b"foo\r\nbar\r\n").unwrap();

    // Pattern "baz\nqux" (LF) cannot match anything in this file.
    let out = bin_in(tmp.path())
        .args([
            "write",
            "crlf_find.txt",
            "--find",
            "baz\nqux",
            "--replace",
            "x",
        ])
        .output()
        .expect("run 8v write");

    assert!(
        !out.status.success(),
        "no-match find must fail\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // The error must NOT expose the internally-normalised "baz\r\nqux" form.
    assert!(
        !combined.contains("baz\r\nqux"),
        "error must not expose internal CRLF-normalised pattern; got: {combined}"
    );
    // File must be untouched.
    assert_eq!(std::fs::read(&file).unwrap(), b"foo\r\nbar\r\n");
}

/// PR#4-R9: find-replace with a newline in the find pattern on a CRLF file
/// when the pattern DOES match after CRLF-normalisation ("foo\nbar" -> "foo\r\nbar").
/// If normalisation is implemented the result must be pure CRLF; if not yet
/// implemented the operation must fail with "no matches found" (not crash).
#[test]
fn find_replace_with_newline_in_find_matches_on_crlf_file() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("crlf_find2.txt");
    std::fs::write(&file, b"foo\r\nbar\r\n").unwrap();

    // "foo\nbar" should match "foo\r\nbar" after normalisation.
    let out = bin_in(tmp.path())
        .args([
            "write",
            "crlf_find2.txt",
            "--find",
            "foo\nbar",
            "--replace",
            "x",
        ])
        .output()
        .expect("run 8v write");

    let result = std::fs::read(&file).unwrap();

    if out.status.success() {
        // Normalisation implemented: result must be pure CRLF.
        assert_eq!(
            result, b"x\r\n",
            "find-replace across CRLF boundary must produce pure CRLF; got: {:?}",
            result
        );
    } else {
        // Normalisation not yet implemented: must fail with no-match error.
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            combined.contains("no matches found"),
            "failure must state 'no matches found'; got: {combined}"
        );
        assert_eq!(
            result, b"foo\r\nbar\r\n",
            "file must be unchanged on no-match"
        );
    }
}
