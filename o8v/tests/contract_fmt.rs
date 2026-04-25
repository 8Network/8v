// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Binary-boundary contract tests for `8v fmt`.
//!
//! Every test spawns `CARGO_BIN_EXE_8v` and asserts on exit code + output shape.
//!
//! # Behavior surface (discovered 2026-04-25)
//!
//! | Scenario                        | exit | stream | key phrase                        |
//! |---------------------------------|------|--------|-----------------------------------|
//! | Path is a file                  |   1  | stderr | "fmt requires a directory path"   |
//! | Nonexistent path                |   1  | stderr | "path not found"                  |
//! | Empty / no-stack dir            |   0  | stdout | "0 stacks formatted"              |
//! | .txt-only dir                   |   0  | stdout | "0 stacks formatted"              |
//! | Readonly file in Rust project   |   1  | stderr | "formatter exited with error"     |
//! | Fmt twice (idempotent)          |   0  | —      | mtime unchanged                   |
//! | Fmt then --check                |   0  | —      | check exits 0                     |
//! | Invalid flag                    |   2  | stderr | unexpected argument               |
//! | --json rust project             |   0  | stdout | parseable JSON with "stacks"      |
//! | --json no-stack dir             |   0  | stdout | reason = "no_stacks"              |
//! | Syntax-error Rust file          |   1  | stderr | "formatter exited with error"     |

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Create a minimal Rust project in a tempdir (no `8v init`).
fn rust_project() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"testpkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    fs::create_dir(dir.path().join("src")).expect("mkdir src");
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
    dir
}

// ─── Test 1: file path clear error ───────────────────────────────────────────

/// `8v fmt <file>` must exit non-zero with a message directing to use a directory.
///
/// Contract: exit=1, stderr contains "fmt requires a directory path".
#[test]
fn fmt_on_file_path_clear_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("test.rs");
    fs::write(&file, "fn main() {}\n").expect("write file");

    let out = bin()
        .args(["fmt", file.to_str().unwrap()])
        .output()
        .expect("run 8v fmt <file>");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "fmt on a file must exit non-zero\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("fmt requires a directory path"),
        "stderr must name the constraint\ngot: {stderr}"
    );
}

// ─── Test 2: nonexistent path ─────────────────────────────────────────────────

/// `8v fmt /tmp/no-such-path` must exit non-zero with the missing path in stderr.
#[test]
fn fmt_on_nonexistent_path() {
    let missing = "/tmp/does-not-exist-contract-fmt-zz9988abc";

    let out = bin()
        .args(["fmt", missing])
        .output()
        .expect("run 8v fmt nonexistent");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "fmt on nonexistent path must exit non-zero\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("path not found") || stderr.contains(missing),
        "stderr must identify the missing path\ngot: {stderr}"
    );
}

// ─── Test 3: empty dir ────────────────────────────────────────────────────────

/// `8v fmt <empty-dir>` must exit 0 — no stacks is not an error.
///
/// Contract: exit=0, output mentions "0 stacks formatted".
#[test]
fn fmt_on_empty_dir() {
    let dir = tempfile::tempdir().expect("tempdir");

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt empty-dir");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "fmt on empty dir must exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );
    // Human-readable output goes to stderr; stdout is empty for non-JSON.
    assert!(
        stderr.contains("0 stacks formatted"),
        "stderr must report zero stacks\nstderr: {stderr}"
    );
}

// ─── Test 4: dir with no supported stack ─────────────────────────────────────

/// `8v fmt <dir-with-only-.txt>` must exit 0 — no recognized stack is a no-op.
#[test]
fn fmt_on_dir_with_no_supported_stack() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("notes.txt"), "hello\n").expect("write notes.txt");

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt txt-only dir");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "fmt on unsupported-stack dir must exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );
    // Human-readable output goes to stderr; stdout is empty for non-JSON.
    assert!(
        stderr.contains("0 stacks formatted"),
        "stderr must report zero stacks\nstderr: {stderr}"
    );
}

// ─── Test 5: readonly file in Rust project ───────────────────────────────────

/// `8v fmt` on a project with a chmod-444 Rust file must exit non-zero.
///
/// Contract: exit=1, NOT silent success (the formatter fails, error is surfaced).
#[test]
fn fmt_on_readonly_file_in_project() {
    let dir = rust_project();
    let src = dir.path().join("src/main.rs");

    // Write something that rustfmt would want to change, then make it readonly.
    fs::write(&src, "fn main(  ) {}\n").expect("write unformatted main.rs");
    let perms = std::fs::Permissions::from_mode(0o444);
    fs::set_permissions(&src, perms).expect("chmod 444");

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt on readonly file");

    // Restore so tempdir cleanup can delete.
    let perms = std::fs::Permissions::from_mode(0o644);
    fs::set_permissions(&src, perms).expect("chmod 644 restore");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "fmt on readonly file must exit non-zero (not silent)\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("formatter exited with error"),
        "stderr must report the formatter error\nstderr: {stderr}"
    );
}

use std::os::unix::fs::PermissionsExt;

// ─── Test 6: fmt idempotent ───────────────────────────────────────────────────

/// Running `8v fmt` twice must not mutate files on the second run.
///
/// Contract: mtime of source file unchanged after second fmt invocation.
#[test]
fn fmt_idempotent() {
    let dir = rust_project();
    let src = dir.path().join("src/main.rs");

    // First fmt — normalizes the file.
    let out1 = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("first fmt");
    assert!(out1.status.success(), "first fmt must succeed");

    let mtime1 = fs::metadata(&src)
        .expect("stat after first fmt")
        .modified()
        .expect("mtime");

    // Second fmt — must be a no-op.
    let out2 = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("second fmt");
    assert!(out2.status.success(), "second fmt must succeed");

    let mtime2 = fs::metadata(&src)
        .expect("stat after second fmt")
        .modified()
        .expect("mtime");

    assert_eq!(
        mtime1, mtime2,
        "second fmt must not mutate the file (mtime changed)"
    );
}

// ─── Test 7: fmt then check passes ───────────────────────────────────────────

/// `8v fmt` followed by `8v fmt --check` must exit 0 on the same project.
///
/// Round-trip contract: fmt produces output that passes its own check.
#[test]
fn fmt_then_check_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"testpkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("Cargo.toml");
    fs::create_dir(dir.path().join("src")).expect("mkdir src");
    // Intentionally unformatted.
    fs::write(dir.path().join("src/main.rs"), "fn main(  ) {}\n").expect("write main.rs");

    let fmt_out = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("8v fmt");
    assert!(
        fmt_out.status.success(),
        "fmt must succeed\nstderr: {}",
        String::from_utf8_lossy(&fmt_out.stderr)
    );

    let check_out = bin()
        .args(["fmt", "--check", dir.path().to_str().unwrap()])
        .output()
        .expect("8v fmt --check");
    let stderr = String::from_utf8_lossy(&check_out.stderr);
    let stdout = String::from_utf8_lossy(&check_out.stdout);
    assert!(
        check_out.status.success(),
        "fmt --check must exit 0 after fmt\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 8: invalid flag ────────────────────────────────────────────────────

/// `8v fmt --no-such-flag .` must exit 2 (clap error) with a message about the bad flag.
#[test]
fn fmt_with_invalid_flag() {
    let out = bin()
        .args(["fmt", "--no-such-flag", "."])
        .output()
        .expect("run 8v fmt --no-such-flag");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "invalid flag must exit 2 (clap)\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("unexpected argument"),
        "stderr must identify the bad flag\nstderr: {stderr}"
    );
}

// ─── Test 9a: --json output shape (rust project) ─────────────────────────────

/// `8v fmt . --json` on a Rust project must emit parseable JSON with a "stacks" array.
#[test]
fn fmt_json_output_shape_rust() {
    let dir = rust_project();

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v fmt --json");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "fmt --json on rust project must exit 0\nstderr: {stderr}"
    );

    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON\ngot: {stdout}\nerr: {e}"),
    };

    assert!(
        v.get("stacks").and_then(|s| s.as_array()).is_some(),
        "JSON must have a 'stacks' array\ngot: {v}"
    );
}

// ─── Test 9b: --json on no-stack dir ─────────────────────────────────────────

/// `8v fmt <empty-dir> --json` must emit JSON with reason="no_stacks".
#[test]
fn fmt_json_output_shape_no_stacks() {
    let dir = tempfile::tempdir().expect("tempdir");

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap(), "--json"])
        .output()
        .expect("run 8v fmt --json no-stack");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "fmt --json on empty dir must exit 0\nstdout: {stdout}"
    );

    let v: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(e) => panic!("stdout must be valid JSON\ngot: {stdout}\nerr: {e}"),
    };

    assert_eq!(
        v["reason"].as_str(),
        Some("no_stacks"),
        "JSON must have reason=no_stacks\ngot: {v}"
    );
}

// ─── Test 10: syntax-error Rust file ─────────────────────────────────────────

/// `8v fmt` on a project containing a Rust file with a syntax error must exit non-zero.
///
/// Contract: rustfmt fails on parse errors; 8v surfaces that as exit=1.
#[test]
fn fmt_on_dir_with_syntax_error_rust_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"testpkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("Cargo.toml");
    fs::create_dir(dir.path().join("src")).expect("mkdir src");
    // Deliberately broken syntax.
    fs::write(dir.path().join("src/main.rs"), "fn broken( {\n").expect("write broken main.rs");

    let out = bin()
        .args(["fmt", dir.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt on syntax-error project");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "fmt on syntax-error file must exit non-zero\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("formatter exited with error"),
        "stderr must report formatter failure\nstderr: {stderr}"
    );
}
