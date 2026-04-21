// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

// ─── assert_passed tests ──────────────────────────────────────────

#[test]
fn assert_passed_succeeds_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    // At least one check should pass on clean code
    let entry = find_entry(result, "cargo check");
    assert_passed(entry);
}

#[test]
#[should_panic(expected = "expected Passed but Failed")]
fn assert_passed_panics_on_failed() {
    // Create a project with violations to get a Failed outcome
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_passed(entry); // clippy should fail on violations
}

// ─── assert_failed tests ──────────────────────────────────────────

#[test]
fn assert_failed_succeeds_on_failed() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_failed(entry);
}

#[test]
#[should_panic(expected = "expected Failed but Passed")]
fn assert_failed_panics_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "cargo check");
    assert_failed(entry);
}

// ─── assert_error tests ───────────────────────────────────────────

#[test]
fn assert_error_succeeds_on_matching_cause() {
    // TypeScript project without node_modules — tools will Error
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_error(entry, "not installed");
}

#[test]
#[should_panic(expected = "does not contain")]
fn assert_error_panics_on_wrong_cause() {
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_error(entry, "timed out"); // wrong cause
}

#[test]
#[should_panic(expected = "expected Error but Passed")]
fn assert_error_panics_on_passed() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "cargo check");
    assert_error(entry, "anything");
}

// ─── assert_parse_status tests ────────────────────────────────────

#[test]
fn assert_parse_status_succeeds_on_match() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_parse_status(entry, ParseStatus::Parsed);
}

#[test]
#[should_panic(expected = "parse_status")]
fn assert_parse_status_panics_on_mismatch() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_parse_status(entry, ParseStatus::Unparsed); // wrong
}

#[test]
#[should_panic(expected = "is Error")]
fn assert_parse_status_panics_on_error() {
    let proj = TempProject::empty();
    proj.write_file("package.json", br#"{"name": "t", "version": "1.0.0"}"#)
        .expect("write package.json");
    proj.write_file("tsconfig.json", b"{}")
        .expect("write tsconfig.json");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::TypeScript);
    let entry = find_entry(result, "tsc");
    assert_parse_status(entry, ParseStatus::Parsed);
}
