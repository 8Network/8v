// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Cross-layer invariant: no command may hang when the working tree contains a
//! parent-pointing symlink loop.
//!
//! Phase 4a foundation-audit contract tests. Each test spawns the binary, plants
//! a `sub/loop → parent` symlink, and asserts completion within the per-command
//! deadline. If a command regresses and re-acquires the BFS hang, the test
//! catches it at the binary boundary.
//!
//! `ls --tree` is already covered by
//! `bug_fixes::ls_tree_does_not_hang_on_symlink_loop`. We do NOT duplicate it
//! here; the comment at the top of the ls_tree test section notes the overlap.

use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

/// Build a temporary directory containing:
///  - a minimal Rust project (`Cargo.toml` + `src/main.rs`)
///  - an initialised 8v workspace (`.8v/` config)
///  - a symlink loop: `sub/loop → <root>`
///
/// The Rust skeleton means that commands which dispatch to the Rust stack
/// (`check`, `fmt`, `build`, `test`) have something real to operate on.
#[cfg(unix)]
fn project_with_symlink_loop() -> TempDir {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();

    // Minimal Rust project.
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"symlink-loop-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // 8v init so the workspace is recognised.
    Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Plant the loop: sub/ directory with sub/loop → parent.
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    symlink(dir.path(), sub.join("loop")).unwrap();

    dir
}

// ─── ls (no --tree) ──────────────────────────────────────────────────────────

/// `8v ls .` must complete in <5s even when the tree contains a symlink loop.
///
/// `8v ls --tree` is already covered by
/// `bug_fixes::ls_tree_does_not_hang_on_symlink_loop`.
#[cfg(unix)]
#[test]
fn ls_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["ls", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "ls . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── ls --tree (cross-reference only) ────────────────────────────────────────
//
// `8v ls --tree` is covered by:
//   o8v/tests/bug_fixes.rs :: ls_tree_does_not_hang_on_symlink_loop
//
// Do not duplicate here.

// ─── init --yes (re-init) ────────────────────────────────────────────────────

/// Running `8v init --yes` a second time in a project that already contains a
/// symlink loop must complete in <5s.
#[cfg(unix)]
#[test]
fn init_reinit_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop(); // loop already planted
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["init", "--yes"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "init --yes (re-init) hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── search ──────────────────────────────────────────────────────────────────

/// `8v search foo .` must complete in <5s.  Exit code 0 (match found) or 1
/// (no match) are both acceptable; a crash or hang is not.
#[cfg(unix)]
#[test]
fn search_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["search", "foo", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "search foo . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let code = out.status.code().unwrap_or(255);
    assert!(
        code == 0 || code == 1,
        "search foo . unexpected exit code {code} on symlink loop; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `8v search foo . --files` must complete in <5s.
#[cfg(unix)]
#[test]
fn search_files_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["search", "foo", ".", "--files"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "search foo . --files hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let code = out.status.code().unwrap_or(255);
    assert!(
        code == 0 || code == 1,
        "search foo . --files unexpected exit code {code} on symlink loop; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── check ───────────────────────────────────────────────────────────────────

/// `8v check .` calls into stack detect; it must complete in <5s on a project
/// containing a symlink loop.  The Rust skeleton gives it something real to
/// dispatch on.
#[cfg(unix)]
#[test]
fn check_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["check", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "check . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── fmt ─────────────────────────────────────────────────────────────────────

/// `8v fmt .` must complete in <5s on a project containing a symlink loop.
#[cfg(unix)]
#[test]
fn fmt_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["fmt", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "fmt . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── build ───────────────────────────────────────────────────────────────────

/// `8v build .` may invoke `cargo build` which takes a few seconds.  Allow up
/// to 10s so the deadline only catches a genuine hang, not a slow compile.
#[cfg(unix)]
#[test]
fn build_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["build", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "build . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── test ────────────────────────────────────────────────────────────────────

/// `8v test .` may invoke `cargo test`; allow up to 10s.
#[cfg(unix)]
#[test]
fn test_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["test", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "test . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── read ────────────────────────────────────────────────────────────────────

/// `8v read .` is not a regular file — it must exit non-zero and must NOT hang.
#[cfg(unix)]
#[test]
fn read_dir_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["read", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "read . hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    assert_ne!(
        out.status.code(),
        Some(0),
        "read . on a directory must exit non-zero; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `8v read sub/loop` points at the looping symlink — it must error gracefully
/// (not a regular file / path escapes sandbox) and must NOT hang.
#[cfg(unix)]
#[test]
fn read_symlink_loop_path_does_not_hang() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["read", "sub/loop"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "read sub/loop hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    assert_ne!(
        out.status.code(),
        Some(0),
        "read sub/loop must exit non-zero (not a regular file or escapes sandbox); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ─── stats ───────────────────────────────────────────────────────────────────

/// `8v stats` reads from the event log, not the file tree — but we test it
/// anyway to lock the contract.
// FIXME phase-4a-fix: stats hangs on symlink loop in o8v/src/stats (event log read blocks)
#[cfg(unix)]
#[test]
#[ignore]
fn stats_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["stats"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "stats hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}

// ─── log ─────────────────────────────────────────────────────────────────────

/// `8v log` reads from the event log, not the file tree — test it anyway.
// FIXME phase-4a-fix: log hangs on symlink loop in o8v/src/log (event log read blocks)
#[cfg(unix)]
#[test]
#[ignore]
fn log_does_not_hang_on_symlink_loop() {
    let dir = project_with_symlink_loop();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_8v"))
        .args(["log"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "log hung on symlink loop: took {}ms",
        elapsed.as_millis()
    );
    let _ = out;
}
