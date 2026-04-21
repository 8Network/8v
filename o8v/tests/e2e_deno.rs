// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! End-to-end tests for `8v` on Deno projects — check, fmt --check.
//!
//! Uses the `check-deno` fixture: a minimal deno.json + main.ts that type-checks clean.
//! The broken `deno-workspace` corpus fixture is intentionally avoided here.

use o8v_testkit::{fixture_path, TempProject};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

fn fixture(name: &str) -> TempProject {
    let path = fixture_path("o8v", name);
    TempProject::from_fixture(&path)
}

// ─── 8v check ───────────────────────────────────────────────────────────────

#[test]
fn deno_check_clean_exits_0() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check on check-deno");

    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "8v check on clean deno project should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn deno_check_stack_detected() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-deno");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "should detect at least one project");

    let stacks: Vec<&str> = results.iter().filter_map(|r| r["stack"].as_str()).collect();
    assert!(
        stacks.contains(&"deno"),
        "expected 'deno' in stacks, got: {stacks:?}"
    );
}

#[test]
fn deno_check_json_has_deno_check_entry() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["check", "--json", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v check --json on check-deno");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("invalid JSON: {e}\noutput: {stdout}"),
    };

    let results = parsed["results"].as_array().expect("results array");
    let deno_result = results
        .iter()
        .find(|r| r["stack"].as_str() == Some("deno"))
        .expect("deno result not found");

    let checks = deno_result["checks"].as_array().expect("checks array");
    let has_deno_check = checks
        .iter()
        .any(|c| c["name"].as_str() == Some("deno check"));
    assert!(
        has_deno_check,
        "expected 'deno check' entry in checks, got: {:?}",
        checks
            .iter()
            .map(|c| c["name"].as_str())
            .collect::<Vec<_>>()
    );
}

// ─── 8v fmt --check ─────────────────────────────────────────────────────────

#[test]
fn deno_fmt_check_runs() {
    let project = fixture("check-deno");

    let out = bin()
        .args(["fmt", "--check", project.path().to_str().unwrap()])
        .output()
        .expect("run 8v fmt --check on check-deno");

    // exit 0 (already formatted) or 1 (needs formatting) — must not crash
    let code = out.status.code().unwrap_or(99);
    assert!(
        code == 0 || code == 1,
        "8v fmt --check on deno must exit 0 or 1, not crash (got {code})\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
