// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

// ─── find_result tests ────────────────────────────────────────────

#[test]
fn find_result_returns_matching_stack() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    assert_eq!(result.stack(), Stack::Rust);
}

#[test]
#[should_panic(expected = "not found in report")]
fn find_result_panics_on_missing_stack() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let _ = find_result(&report, Stack::Python);
}

// ─── find_entry tests ─────────────────────────────────────────────

#[test]
fn find_entry_returns_matching_check() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let entry = find_entry(result, "clippy");
    assert_eq!(entry.name(), "clippy");
}

#[test]
#[should_panic(expected = "not found")]
fn find_entry_panics_on_missing_check() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let result = find_result(&report, Stack::Rust);
    let _ = find_entry(result, "nonexistent-check");
}

// ─── all_check_names tests ────────────────────────────────────────

#[test]
fn all_check_names_includes_all_results() {
    let proj = TempProject::empty();
    proj.write_file(
        "Cargo.toml",
        b"[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .expect("write Cargo.toml");
    proj.create_dir("src").expect("create src/");
    proj.write_file("src/main.rs", b"fn main() {}\n")
        .expect("write src/main.rs");
    let report = run_check_path(proj.path());
    let names = all_check_names(&report);
    // Rust has at least cargo check, clippy, cargo fmt
    assert!(names.contains(&"clippy"), "missing clippy: {names:?}");
    assert!(
        names.contains(&"cargo check"),
        "missing cargo check: {names:?}"
    );
}
