// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary-boundary contract tests: `8v <command>` rejects out-of-project paths.
//!
//! Each test spawns the actual `8v` binary and asserts:
//!   1. Non-zero exit code
//!   2. stderr contains `"escapes project directory"`
//!
//! Commands that intentionally bypass workspace containment (search, ls, fmt)
//! are marked `#[ignore]` with a FIXME.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::PathBuf;
use std::process::Command;

// ─── Setup ───────────────────────────────────────────────────────────────────

struct TestProject {
    /// The temp dir that owns the project. Kept alive for the test duration.
    _dir: tempfile::TempDir,
    /// Canonical path to the project root (resolves macOS /tmp → /private/tmp).
    root: PathBuf,
    /// A file inside the project to use as a valid target.
    _inside_file: PathBuf,
}

impl TestProject {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        // Canonicalize so macOS /tmp → /private/tmp doesn't break containment checks.
        let root = dir
            .path()
            .canonicalize()
            .expect("canonicalize project root");

        // Run `8v init --yes` so the workspace is recognized.
        let status = Command::new(env!("CARGO_BIN_EXE_8v"))
            .args(["init", "--yes"])
            .current_dir(&root)
            .status()
            .expect("8v init --yes");
        assert!(status.success(), "8v init --yes failed");

        // Create a file inside the project.
        let inside_file = root.join("inside.txt");
        fs::write(&inside_file, "hello\n").expect("write inside.txt");

        TestProject {
            _dir: dir,
            root,
            _inside_file: inside_file,
        }
    }

    /// Return an absolute path to a file that is outside this project root.
    fn outside_file(&self) -> PathBuf {
        // /tmp/outside_<random>.txt lives outside our project temp dir.
        let name = format!(
            "/tmp/outside_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        );
        fs::write(&name, "outside\n").expect("write outside file");
        PathBuf::from(name)
    }

    /// Return a path to an existing file that lives OUTSIDE the project root
    /// (in the parent of the temp dir). Useful for traversal tests that need
    /// a file that actually exists so canonicalize() succeeds and the
    /// containment check fires rather than a "not found" error.
    fn sibling_file(&self) -> PathBuf {
        // parent() of canonical root is guaranteed to be outside the project.
        let parent = self.root.parent().expect("root has parent");
        let name = format!(
            "sibling_escape_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        );
        let path = parent.join(&name);
        fs::write(&path, "sibling\n").expect("write sibling file");
        path
    }

    /// Run `8v` inside the project root and return (exit_code, combined_output).
    /// Combined output = stderr + stdout so that both text-mode errors (stderr)
    /// and --json errors (stdout JSON envelope) are captured in one string.
    fn run(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_8v"))
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("spawn 8v");
        let code = out.status.code().unwrap_or(-1);
        let mut combined = String::from_utf8_lossy(&out.stderr).into_owned();
        combined.push_str(&String::from_utf8_lossy(&out.stdout));
        (code, combined)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn assert_containment_violation(code: i32, stderr: &str) {
    assert_ne!(code, 0, "expected non-zero exit code; stderr:\n{stderr}");
    assert!(
        stderr.contains("escapes project directory"),
        "expected 'escapes project directory' in stderr; got:\n{stderr}"
    );
}

// ─── read: absolute path ─────────────────────────────────────────────────────

#[test]
fn read_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let (code, stderr) = proj.run(&["read", outside.to_str().unwrap()]);
    assert_containment_violation(code, &stderr);
}

// ─── read: traversal ─────────────────────────────────────────────────────────

#[test]
fn read_rejects_traversal_path() {
    let proj = TestProject::new();
    // Use a sibling file that actually exists outside the project root.
    // A non-existent traversal target (e.g. ../../../etc/passwd) causes
    // canonicalize() to fail inside guarded_read, so "not found" wins over
    // the containment check. The sibling file is guaranteed to exist.
    let sibling = proj.sibling_file();
    // Build a relative traversal path from the project root to the sibling.
    // sibling is at <parent>/<name>; from root (= <parent>/<project>) that is ../sibling.
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, stderr) = proj.run(&["read", &traversal]);
    assert_containment_violation(code, &stderr);
}

// ─── read --json: absolute path ──────────────────────────────────────────────

#[test]
fn read_json_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let (code, combined) = proj.run(&["read", "--json", outside.to_str().unwrap()]);
    assert_containment_violation(code, &combined);
}

// ─── read --json: traversal ───────────────────────────────────────────────────

#[test]
fn read_json_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, combined) = proj.run(&["read", "--json", &traversal]);
    assert_containment_violation(code, &combined);
}

// ─── write: absolute path ────────────────────────────────────────────────────

#[test]
fn write_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    // Attempt to replace line 1 of the outside file.
    let target = format!("{}:1", outside.to_str().unwrap());
    let (code, stderr) = proj.run(&["write", &target, "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write: traversal ────────────────────────────────────────────────────────

#[test]
fn write_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}:1");
    let (code, stderr) = proj.run(&["write", &traversal, "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── search: outside path ────────────────────────────────────────────────────
//
// FIXME phase-2c: search does not enforce containment at binary layer.
// `do_search` intentionally creates a new ContainmentRoot anchored at the
// supplied path so files inside it can be read. An outside path is NOT rejected.
//
// POLICY: search/ls accept any explicit path arg (read-only). Containment is not enforced; this is intentional.
#[test]
#[ignore]
fn search_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside_dir = tempfile::tempdir().expect("outside dir");
    let outside_path = outside_dir.path().canonicalize().expect("canonicalize");
    let (code, stderr) = proj.run(&["search", "hello", outside_path.to_str().unwrap()]);
    assert_containment_violation(code, &stderr);
}

// ─── ls: outside path ────────────────────────────────────────────────────────
//
// FIXME phase-2c: ls does not enforce containment at binary layer.
// `do_ls` creates ContainmentRoot anchored at the supplied path, not the workspace root.
//
// POLICY: search/ls accept any explicit path arg (read-only). Containment is not enforced; this is intentional.
#[test]
#[ignore]
fn ls_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside_dir = tempfile::tempdir().expect("outside dir");
    let outside_path = outside_dir.path().canonicalize().expect("canonicalize");
    let (code, stderr) = proj.run(&["ls", outside_path.to_str().unwrap()]);
    assert_containment_violation(code, &stderr);
}

// ─── fmt: outside path ───────────────────────────────────────────────────────

#[test]
fn fmt_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside_dir = tempfile::tempdir().expect("outside dir");
    let outside_path = outside_dir.path().canonicalize().expect("canonicalize");
    let (code, stderr) = proj.run(&["fmt", outside_path.to_str().unwrap()]);
    assert_containment_violation(code, &stderr);
}

#[test]
fn fmt_rejects_traversal_escape() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, stderr) = proj.run(&["fmt", &traversal]);
    assert_containment_violation(code, &stderr);
}

// ─── symlink escape: read ─────────────────────────────────────────────────────

#[test]
fn read_rejects_symlink_escaping_root() {
    let proj = TestProject::new();

    // Create a file outside the project.
    let outside = proj.outside_file();

    // Create a symlink inside the project pointing to the outside file.
    let symlink_path = proj.root.join("escape_link.txt");
    unix_fs::symlink(&outside, &symlink_path).expect("create symlink");

    let (code, stderr) = proj.run(&["read", "escape_link.txt"]);
    assert_containment_violation(code, &stderr);
}

// ─── symlink escape: write ────────────────────────────────────────────────────

#[test]
fn write_rejects_symlink_escaping_root() {
    let proj = TestProject::new();

    // Create a file outside the project.
    let outside = proj.outside_file();

    // Create a symlink inside the project pointing to the outside file.
    let symlink_path = proj.root.join("escape_write_link.txt");
    unix_fs::symlink(&outside, &symlink_path).expect("create symlink");

    let (code, stderr) = proj.run(&["write", "escape_write_link.txt:1", "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --insert: absolute path ───────────────────────────────────────────

#[test]
fn write_insert_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let target = format!("{}:1", outside.to_str().unwrap());
    let (code, stderr) = proj.run(&["write", &target, "--insert", "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --insert: traversal ───────────────────────────────────────────────

#[test]
fn write_insert_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}:1");
    let (code, stderr) = proj.run(&["write", &traversal, "--insert", "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --delete: absolute path ───────────────────────────────────────────

#[test]
fn write_delete_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let target = format!("{}:1", outside.to_str().unwrap());
    let (code, stderr) = proj.run(&["write", &target, "--delete"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --delete: traversal ───────────────────────────────────────────────

#[test]
fn write_delete_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}:1");
    let (code, stderr) = proj.run(&["write", &traversal, "--delete"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --append: absolute path ───────────────────────────────────────────

#[test]
fn write_append_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let (code, stderr) = proj.run(&["write", outside.to_str().unwrap(), "--append", "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --append: traversal ───────────────────────────────────────────────

#[test]
fn write_append_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, stderr) = proj.run(&["write", &traversal, "--append", "injected"]);
    assert_containment_violation(code, &stderr);
}

// ─── write --find/--replace: absolute path ───────────────────────────────────

#[test]
fn write_find_replace_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside = proj.outside_file();
    let (code, stderr) = proj.run(&[
        "write",
        outside.to_str().unwrap(),
        "--find",
        "outside",
        "--replace",
        "injected",
    ]);
    assert_containment_violation(code, &stderr);
}

// ─── write --find/--replace: traversal ───────────────────────────────────────

#[test]
fn write_find_replace_rejects_traversal_path() {
    let proj = TestProject::new();
    let sibling = proj.sibling_file();
    let sibling_name = sibling.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, stderr) = proj.run(&[
        "write",
        &traversal,
        "--find",
        "sibling",
        "--replace",
        "injected",
    ]);
    assert_containment_violation(code, &stderr);
}

// ─── init: outside path ──────────────────────────────────────────────────────
//
// FIXME phase-4b-decision: `8v init <path>` anchors its ContainmentRoot to the
// supplied path argument, not to the current project. Passing an outside dir
// succeeds by design — init bootstraps a new workspace at any given location.
// These tests lock current (permissive) behavior and surface it as a decision
// point: should init refuse to run when invoked from inside an existing project
// with an outside-project target?
#[test]
#[ignore]
fn init_rejects_absolute_outside_path() {
    let proj = TestProject::new();
    let outside_dir = tempfile::tempdir().expect("outside dir");
    let outside_path = outside_dir.path().canonicalize().expect("canonicalize");
    let (code, stderr) = proj.run(&["init", "--yes", outside_path.to_str().unwrap()]);
    assert_containment_violation(code, &stderr);
}

// FIXME phase-4b-decision: same as above — traversal to a sibling dir is
// accepted because init re-anchors containment to its path arg.
#[test]
#[ignore]
fn init_rejects_traversal_path() {
    let proj = TestProject::new();
    // Create a sibling directory outside the project root.
    let parent = proj.root.parent().expect("root has parent");
    let sibling_dir = parent.join(format!(
        "sibling_init_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    fs::create_dir(&sibling_dir).expect("create sibling dir");
    let sibling_name = sibling_dir.file_name().unwrap().to_str().unwrap();
    let traversal = format!("../{sibling_name}");
    let (code, stderr) = proj.run(&["init", "--yes", &traversal]);
    assert_containment_violation(code, &stderr);
}
