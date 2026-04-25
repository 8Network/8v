// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! E2E contract tests for `8v search`:
//! - `--limit 0` rejected at parse with exit 2 (clap error).
//! - Single-file search output includes the path label on every match.

use std::io::Write;
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
    assert!(
        out.status.success(),
        "8v init must succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

#[test]
fn search_limit_zero_rejected_at_parse() {
    let dir = init_temp_workspace();
    let out = bin()
        .args(["search", "foo", "--limit", "0"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");
    assert_eq!(
        out.status.code(),
        Some(2),
        "--limit 0 must exit 2 (clap parse error)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--limit") || stderr.contains("limit"),
        "stderr must mention --limit; got: {stderr}"
    );
}

#[test]
fn search_single_file_emits_path_prefix() {
    let dir = init_temp_workspace();
    let target = dir.path().join("needle.txt");
    let mut f = std::fs::File::create(&target).expect("create needle");
    writeln!(f, "hello world").unwrap();
    writeln!(f, "hello claude").unwrap();
    drop(f);

    let out = bin()
        .args(["search", "hello", "needle.txt"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search");
    assert!(
        out.status.success(),
        "single-file search must succeed\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("Found"))
    {
        assert!(
            line.starts_with("needle.txt:"),
            "single-file search result must prefix with file name; got: {line:?}\nfull stdout: {stdout}"
        );
    }
}

#[test]
fn search_absolute_path_outside_workspace_finds_matches() {
    let workspace = init_temp_workspace();
    // Create a SEPARATE directory outside the workspace root.
    let outside = tempfile::tempdir().expect("outside tempdir");
    let needle = outside.path().join("target.rs");
    std::fs::write(
        &needle,
        "pub fn fetch_unique_xyzzy() {}
",
    )
    .expect("write needle");

    let out = bin()
        .args([
            "search",
            "fetch_unique_xyzzy",
            outside.path().to_str().unwrap(),
        ])
        .current_dir(workspace.path())
        .output()
        .expect("run 8v search");
    assert!(
        out.status.success(),
        "search with absolute path outside workspace must succeed
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Compact mode shows <file>:<line>; -C shows match text. Verify a match was found.
    assert!(
        stdout.contains("target.rs") && stdout.contains("Found"),
        "must find match in absolute path outside workspace
stdout: {stdout}
stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// SEARCH-1: `--files` must list paths of files whose CONTENT matches the pattern,
/// not files whose NAME matches the pattern.
#[test]
fn search_files_flag_lists_content_matching_paths() {
    let dir = init_temp_workspace();

    let matching1 = dir.path().join("lib.rs");
    std::fs::write(&matching1, "pub fn main() {}\n").expect("write lib.rs");

    let matching2 = dir.path().join("helper.rs");
    std::fs::write(&matching2, "// helper\nfn main() {\n    todo!()\n}\n")
        .expect("write helper.rs");

    let non_matching = dir.path().join("no_match.rs");
    std::fs::write(&non_matching, "pub fn helper() {}\n").expect("write no_match.rs");

    let out = bin()
        .args(["search", "fn main", ".", "--files"])
        .current_dir(dir.path())
        .output()
        .expect("run 8v search --files");

    assert!(
        out.status.success(),
        "--files search must exit 0 when matches exist\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("lib.rs"),
        "--files output must contain lib.rs; got:\n{stdout}"
    );
    assert!(
        stdout.contains("helper.rs"),
        "--files output must contain helper.rs; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("no_match.rs"),
        "--files output must NOT contain no_match.rs; got:\n{stdout}"
    );
}
