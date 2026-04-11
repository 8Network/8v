//! Integration test for fmt functionality.
//!
//! Creates a temporary Rust project with unformatted code, runs fmt,
//! and verifies it detects the formatting needs correctly.

use o8v_core::{FmtConfig, FmtOutcome};
use o8v_project::ProjectRoot;
use o8v_stacks::fmt;
use std::fs;
use std::sync::atomic::AtomicBool;

// Static AtomicBool for testing
static NEVER_INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Create a test Rust project with unformatted code.
fn create_unformatted_rust_project(dir: &std::path::Path) -> std::io::Result<()> {
    // Create Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "test-fmt"
version = "0.1.0"
edition = "2021"
"#,
    )?;

    // Create src directory
    fs::create_dir(dir.join("src"))?;

    // Create unformatted main.rs
    fs::write(
        dir.join("src/main.rs"),
        r#"fn main(  ) {
    println!("hello"  )  ;
}
"#,
    )?;

    Ok(())
}

#[test]
#[ignore] // Requires cargo to be in PATH; test manually when cargo is available
fn fmt_detects_unformatted_rust_in_check_mode() {
    let tmpdir = tempfile::tempdir().unwrap();
    let tmppath = tmpdir.path();

    // Create unformatted Rust project
    create_unformatted_rust_project(tmppath).expect("failed to create test project");

    // Run fmt in check mode
    let root = ProjectRoot::new(tmppath).expect("invalid path");
    let config = FmtConfig {
        timeout: None,
        check_mode: true,
        interrupted: &NEVER_INTERRUPTED,
    };

    let report = fmt(&root, &config);

    // Should have found the project
    assert!(!report.entries.is_empty(), "expected to find Rust project");
    assert!(
        report.detection_errors.is_empty(),
        "expected no detection errors"
    );

    // Should report that files need formatting
    let entry = &report.entries[0];
    match &entry.outcome {
        FmtOutcome::Ok { .. } => panic!("expected Dirty but got Ok"),
        FmtOutcome::Dirty { .. } => {
            // Success: formatter detected unformatted code
        }
        FmtOutcome::Error { cause, .. } => {
            panic!("expected Dirty but got Error: {}", cause)
        }
        FmtOutcome::NotFound { program } => {
            panic!("formatter not found: {}", program)
        }
    }
}

#[test]
fn fmt_report_is_not_ok_when_entries_have_errors() {
    let tmpdir = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(tmpdir.path()).unwrap();

    let report = o8v_core::FmtReport {
        entries: vec![o8v_core::FmtEntry {
            stack: o8v_project::Stack::Rust,
            project_root: root,
            tool: "cargo fmt".to_string(),
            outcome: FmtOutcome::Error {
                cause: "test error".to_string(),
                stderr: "test stderr".to_string(),
            },
        }],
        detection_errors: vec![],
    };

    assert!(!report.is_ok());
}
