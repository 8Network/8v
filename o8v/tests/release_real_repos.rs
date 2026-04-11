//! E2E tests against 5 real public repositories.
//!
//! This is the Phase 1 release gate: proves that `8v check` works on real-world
//! code, not just synthetic fixtures. Each repo exercises a different stack.
//!
//! ## Repos under test
//!
//! | Repo                      | Stack      | URL                                       |
//! |---------------------------|------------|-------------------------------------------|
//! | BurntSushi/ripgrep        | rust       | https://github.com/BurntSushi/ripgrep     |
//! | psf/requests              | python     | https://github.com/psf/requests           |
//! | junegunn/fzf              | go         | https://github.com/junegunn/fzf           |
//! | microsoft/TypeScript      | typescript | https://github.com/microsoft/TypeScript   |
//! | dotnet/aspnetcore         | dotnet     | https://github.com/dotnet/aspnetcore      |
//!
//! ## What is asserted for each repo
//!
//! 1. Clone succeeds
//! 2. `8v check --json` exits with a valid code: 0 (pass), 1 (violations), 2 (nothing to check)
//!    Any other exit code means a crash or internal error — that is a bug.
//! 3. `--json` output is valid JSON
//! 4. `results` array is non-empty (at least one project was detected and checked)
//! 5. The expected stack appears in at least one result entry
//!
//! ## Running
//!
//! These tests are `#[ignore]`d by default because they require network access,
//! language toolchains, and several minutes to complete.
//!
//! Run all five:
//! ```sh
//! cargo test -p o8v --test real_repos -- --ignored
//! ```
//!
//! Run one at a time:
//! ```sh
//! cargo test -p o8v --test real_repos real_repo_ripgrep -- --ignored
//! ```

use std::path::Path;
use std::process::Command;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Shallow-clone `url` into `dest`. Returns false on failure.
fn git_clone(url: &str, dest: &Path) -> bool {
    match Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            "--quiet",
            url,
            dest.to_str().expect("valid UTF-8 path"),
        ])
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

/// Core assertion logic shared by all five tests.
///
/// Clones `url`, runs `8v check <dir> --json`, then asserts:
/// - Exit code in {0, 1, 2} — no crash
/// - stdout is valid JSON
/// - `results` array non-empty — at least one project detected
/// - `expected_stack` appears in `results[].stack`
fn check_real_repo(name: &str, url: &str, expected_stack: &str) {
    let tmpdir = tempfile::tempdir().expect("failed to create temporary directory");

    assert!(
        git_clone(url, tmpdir.path()),
        "{name}: git clone failed for {url}"
    );

    let out = bin()
        .args([
            "check",
            tmpdir.path().to_str().expect("valid path"),
            "--json",
        ])
        .output()
        .expect("failed to run 8v check");

    // ── Exit code ─────────────────────────────────────────────────────────────

    let exit_code = out.status.code().unwrap_or(-1);
    assert!(
        matches!(exit_code, 0..=2),
        "{name}: exit code {exit_code} is not a valid 8v exit code (0=pass, 1=violations, 2=nothing)\n\
         stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // ── JSON validity ─────────────────────────────────────────────────────────

    // stdout must be valid JSON — the `--json` flag guarantees this.
    // Note: do NOT use String::from_utf8_lossy + echo/println to validate JSON —
    // echo interprets \n escape sequences inside strings and corrupts the content.
    // Parse directly from the raw bytes.
    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "{name}: --json output is not valid JSON: {e}\n\
             stdout (first 500 bytes): {}",
            String::from_utf8_lossy(&out.stdout[..out.stdout.len().min(500)])
        ),
    };

    // ── Results non-empty ─────────────────────────────────────────────────────

    let results = json["results"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}: JSON missing 'results' array"));

    assert!(
        !results.is_empty(),
        "{name}: 'results' array is empty — no projects were detected or checked"
    );

    // ── Stack detection ───────────────────────────────────────────────────────

    let detected_stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();

    assert!(
        detected_stacks.contains(&expected_stack),
        "{name}: expected stack '{expected_stack}' not found\n\
         detected stacks: {detected_stacks:?}"
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// ripgrep — Rust project. Exercises: cargo check, clippy, cargo fmt.
#[test]
#[ignore = "requires network and cargo toolchain (~1 min)"]
fn real_repo_ripgrep() {
    check_real_repo("ripgrep", "https://github.com/BurntSushi/ripgrep", "rust");
}

/// requests — Python project. Exercises: ruff, mypy.
#[test]
#[ignore = "requires network and python toolchain (~1 min)"]
fn real_repo_requests() {
    check_real_repo("requests", "https://github.com/psf/requests", "python");
}

/// fzf — Go project. Exercises: go vet, staticcheck, gofmt.
#[test]
#[ignore = "requires network and go toolchain (~1 min)"]
fn real_repo_fzf() {
    check_real_repo("fzf", "https://github.com/junegunn/fzf", "go");
}

/// TypeScript compiler — TypeScript project.
///
/// Notable: tsconfig.json lives in src/, not at the repo root.
/// Detection uses `typescript` in devDependencies as the signal.
/// Exercises: tsc, eslint, prettier.
#[test]
#[ignore = "requires network and node toolchain (~3 min)"]
fn real_repo_typescript_compiler() {
    check_real_repo(
        "typescript",
        "https://github.com/microsoft/TypeScript",
        "typescript",
    );
}

/// aspnetcore — .NET project. Exercises: dotnet build, dotnet format.
#[test]
#[ignore = "requires network and dotnet toolchain (~4 min)"]
fn real_repo_aspnetcore() {
    check_real_repo(
        "aspnetcore",
        "https://github.com/dotnet/aspnetcore",
        "dotnet",
    );
}
