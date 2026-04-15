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

/// FindReplace without --all replaces only the first occurrence.
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
        out.status.success(),
        "should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "qux bar\nfoo baz\n");
}

/// FindReplace with --all replaces every occurrence.
#[test]
fn find_replace_all() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, "foo bar\nfoo baz\n").unwrap();

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--find", "foo", "--replace", "qux", "--all"])
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
    assert_eq!(result, "line1\nline2\nline3");
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

/// CE6: Lone-\r (classic Mac) files are rejected for line-based operations.
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

/// CE19: Mixed LF+CRLF files are rejected for line-based operations.
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
        combined.contains("does not exist"),
        "error must mention 'does not exist'\ngot: {combined}"
    );
    assert!(
        combined.contains("8v write"),
        "error must include remediation hint with '8v write'\ngot: {combined}"
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
        result,
        b"line1\r\nreplaced\r\nline3\r\n",
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

// ─── HIGH-3 / HIGH-2: content line-ending validation ─────────────────────────

/// HIGH-3: Content with embedded lone \r is rejected.
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

/// HIGH-2: Content with mixed CRLF+LF endings is rejected.
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

/// HIGH-3 (CRLF): Content with pure CRLF endings is rejected — was silently
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

// ─── HIGH-6: mid-line \r in a \n-terminated file ─────────────────────────────

/// HIGH-6: A file containing "A\nB\rC\nD\n" (mid-line \r, not part of \r\n)
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

// ─── HIGH-9: CreateFile empty content ────────────────────────────────────────

/// HIGH-9: CreateFile with empty content is rejected with a clear error.
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
        result,
        b"line1\r\nnew_a\r\nnew_b\r\nline3\r\n",
        "CRLF endings must be preserved byte-exactly including injected lines"
    );
}

// ─── B1: Append uses the file's existing line ending as separator ────────────

/// Append to a CRLF file without a trailing terminator must use \r\n as the
/// separator, not a hardcoded \n (which would create mixed endings).
#[test]
fn append_to_crlf_file_without_trailing_newline_preserves_crlf() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    setup_project(&tmp);
    let file = tmp.path().join("src.txt");
    std::fs::write(&file, b"line1\r\nline2").unwrap(); // CRLF, no trailing

    let out = bin_in(tmp.path())
        .args(["write", "src.txt", "--append", "line3"])
        .output()
        .expect("run 8v write");

    assert!(
        out.status.success(),
        "append should succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = std::fs::read(&file).unwrap();
    assert_eq!(
        result, b"line1\r\nline2\r\nline3",
        "separator must match existing CRLF, not inject lone \\n"
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
    assert_eq!(result, b"a\nb\nc");
}

// ─── H1: --find --replace validates strings for \r ───────────────────────────

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
