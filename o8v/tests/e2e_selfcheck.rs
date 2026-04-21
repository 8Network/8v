// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Self-dogfood tests — run 8v against its own codebase.
//!
//! No fixtures, no network. The 8v workspace itself is the test subject.
//! Covers: ls, search, read, check, build across all commands.

use std::process::Command;

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

// ─── ls tests ────────────────────────────────────────────────────────────────

#[test]
fn selfcheck_ls_json_finds_rust_project() {
    let ws = workspace();
    let out = bin()
        .args(["ls", ws.to_str().expect("valid path"), "--json"])
        .output()
        .expect("run 8v ls --json on 8v workspace");

    assert!(
        out.status.success(),
        "8v ls --json should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    let projects = match json["projects"].as_array() {
        Some(p) => p,
        None => panic!("JSON missing 'projects' array"),
    };

    assert!(
        !projects.is_empty(),
        "8v workspace should contain at least one project"
    );

    let has_rust = projects.iter().any(|p| p["stack"].as_str() == Some("rust"));
    assert!(has_rust, "expected at least one project with stack='rust'");
}

#[test]
fn selfcheck_ls_json_has_required_fields() {
    let ws = workspace();
    let out = bin()
        .args(["ls", ws.to_str().expect("valid path"), "--json"])
        .output()
        .expect("run 8v ls --json");

    assert!(out.status.success(), "should exit 0");

    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    assert!(json.get("projects").is_some(), "missing 'projects' field");
    assert!(
        json.get("total_projects").is_some(),
        "missing 'total_projects' field"
    );
    assert!(
        json.get("total_files").is_some(),
        "missing 'total_files' field"
    );
}

#[test]
fn selfcheck_ls_stack_filter_rust_exits_0() {
    let ws = workspace();
    let out = bin()
        .args(["ls", "--stack", "rust", ws.to_str().expect("valid path")])
        .output()
        .expect("run 8v ls --stack rust");

    assert!(
        out.status.success(),
        "8v ls --stack rust should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("rust"),
        "output should contain 'rust' when filtering by stack=rust\ngot: {stdout}"
    );
}

#[test]
fn selfcheck_ls_tree_does_not_crash() {
    let ws = workspace();
    let out = bin()
        .args(["ls", "--tree", ws.to_str().expect("valid path")])
        .output()
        .expect("run 8v ls --tree");

    assert!(
        out.status.success(),
        "8v ls --tree should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.is_empty(),
        "8v ls --tree output should be non-empty"
    );
}

// ─── search tests ────────────────────────────────────────────────────────────

#[test]
fn selfcheck_search_finds_fn_main() {
    let ws = workspace();
    let src_path = ws.join("o8v").join("src");
    let out = bin()
        .args([
            "search",
            "--json",
            "fn main",
            src_path.to_str().expect("valid path"),
        ])
        .output()
        .expect("run 8v search --json fn main");

    assert!(
        out.status.success(),
        "8v search should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    let total = json["total_matches"].as_u64().unwrap_or(0);
    assert!(
        total > 0,
        "expected at least one match for 'fn main' in o8v/src"
    );
}

#[test]
fn selfcheck_search_json_has_required_fields() {
    let ws = workspace();
    let src_path = ws.join("o8v").join("src");
    let out = bin()
        .args([
            "search",
            "--json",
            "fn main",
            src_path.to_str().expect("valid path"),
        ])
        .output()
        .expect("run 8v search --json");

    assert!(out.status.success(), "should exit 0");

    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    assert!(json.get("files").is_some(), "missing 'files' field");
    assert!(
        json.get("total_matches").is_some(),
        "missing 'total_matches' field"
    );
    assert!(
        json.get("files_searched").is_some(),
        "missing 'files_searched' field"
    );
}

#[test]
fn selfcheck_search_files_flag_produces_paths() {
    let ws = workspace();
    let src_path = ws.join("o8v").join("src");
    // --files searches file *names*, not contents. Use a pattern that matches
    // the filename "main.rs" (the pattern "main" matches the name component).
    let out = bin()
        .args([
            "search",
            "main",
            src_path.to_str().expect("valid path"),
            "--files",
        ])
        .output()
        .expect("run 8v search --files");

    assert!(
        out.status.success(),
        "8v search --files should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(".rs"),
        "output should contain .rs file paths\ngot: {stdout}"
    );
}

// ─── read tests ──────────────────────────────────────────────────────────────

#[test]
fn selfcheck_read_symbol_map_has_fn_main() {
    let ws = workspace();
    let main_rs = ws.join("o8v").join("src").join("main.rs");
    let out = bin()
        .args(["read", main_rs.to_str().expect("valid path")])
        .output()
        .expect("run 8v read main.rs");

    assert!(
        out.status.success(),
        "8v read should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fn main"),
        "symbol map should contain 'fn main'\ngot: {stdout}"
    );
}

#[test]
fn selfcheck_read_range_exits_0() {
    let ws = workspace();
    let main_rs = ws.join("o8v").join("src").join("main.rs");
    let range_arg = format!("{}:1-10", main_rs.to_str().expect("valid path"));
    let out = bin()
        .args(["read", &range_arg])
        .output()
        .expect("run 8v read main.rs:1-10");

    assert!(
        out.status.success(),
        "8v read range should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.is_empty(), "range output should be non-empty");
}

#[test]
fn selfcheck_read_json_is_valid() {
    let ws = workspace();
    let main_rs = ws.join("o8v").join("src").join("main.rs");
    let out = bin()
        .args(["read", "--json", main_rs.to_str().expect("valid path")])
        .output()
        .expect("run 8v read --json");

    assert!(
        out.status.success(),
        "8v read --json should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    match serde_json::from_slice::<serde_json::Value>(&out.stdout) {
        Ok(_) => {}
        Err(e) => panic!(
            "8v read --json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    }
}

// ─── check tests ─────────────────────────────────────────────────────────────

#[test]
fn selfcheck_check_exits_0() {
    let ws = workspace();
    let out = bin()
        .args(["check", "--json", ws.to_str().expect("valid path")])
        .output()
        .expect("run 8v check --json on 8v workspace");

    assert!(
        out.status.success(),
        "8v check on its own workspace should exit 0 (passes its own checks)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn selfcheck_check_json_has_rust_stack() {
    let ws = workspace();
    let out = bin()
        .args(["check", "--json", ws.to_str().expect("valid path")])
        .output()
        .expect("run 8v check --json");

    // check may exit 0 or non-zero; we only care about the JSON shape here.
    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    let results = match json["results"].as_array() {
        Some(r) => r,
        None => panic!("JSON missing 'results' array"),
    };

    assert!(
        !results.is_empty(),
        "'results' array should be non-empty for the 8v workspace"
    );

    let detected_stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();

    assert!(
        detected_stacks.contains(&"rust"),
        "expected stack='rust' in results\ndetected: {detected_stacks:?}"
    );
}

// ─── build tests ─────────────────────────────────────────────────────────────

#[test]
fn selfcheck_build_crate_succeeds() {
    let ws = workspace();
    let stacks_path = ws.join("o8v-stacks");
    let out = bin()
        .args(["build", "--json", stacks_path.to_str().expect("valid path")])
        .output()
        .expect("run 8v build --json on o8v-stacks");

    assert!(
        out.status.success(),
        "8v build --json on o8v-stacks should exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => panic!(
            "--json output is not valid JSON: {e}\nstdout: {}",
            String::from_utf8_lossy(&out.stdout)
        ),
    };

    assert_eq!(json["stack"], "rust", "build stack should be 'rust'");
    assert_eq!(json["exit_code"], 0, "exit_code should be 0");
}
