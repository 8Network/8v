//! Tests for JS/TS `NodeToolCheck` — local tool resolution from `node_modules/.bin`.

use o8v_core::CheckOutcome;
use o8v_testkit::{assert_passed, run_check_path};

// ─── TypeScript: tool not installed locally ───────────────────────────────

#[test]
fn ts_no_local_tsc_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    // Detected as TypeScript (package.json + tsconfig.json)
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
    // No node_modules/.bin — tools not installed

    let report = run_check_path(dir.path());
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );

    // Required tools (tsc, eslint) should report Error.
    // Optional tools (prettier, biome, oxlint, shellcheck) should return Passed (tool not used or no shell files).
    for entry in report.results()[0].entries() {
        match entry.name() {
            "tsc" | "eslint" => match entry.outcome() {
                CheckOutcome::Error { cause, .. } => {
                    assert!(
                        cause.contains("not installed locally"),
                        "{}: should say not installed: {cause}",
                        entry.name()
                    );
                }
                other => panic!("{}: expected Error, got {other:?}", entry.name()),
            },
            "prettier" | "biome" | "oxlint" | "shellcheck" => {
                // Optional tools should return Passed when not installed or when n/a
                assert_passed(entry);
            }
            _ => panic!("unexpected tool: {}", entry.name()),
        }
    }
}

// ─── TypeScript: local tool runs ──────────────────────────────────────────

#[cfg(unix)]
#[test]
fn ts_local_tsc_runs() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    // Create fake node_modules/.bin tools (both required and optional) that exit 0
    let bin = dir.path().join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    for tool in ["tsc", "eslint", "prettier", "biome", "oxlint"] {
        let script = bin.join(tool);
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let report = run_check_path(dir.path());
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );

    // Both should pass
    for entry in report.results()[0].entries() {
        assert_passed(entry);
    }
}

// ─── JavaScript: same pattern ─────────────────────────────────────────────

#[test]
fn js_no_local_eslint_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    // Detected as JavaScript (package.json, no tsconfig)
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();

    let report = run_check_path(dir.path());
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    assert_eq!(
        report.results().len(),
        1,
        "should detect exactly one project"
    );

    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "eslint", "check should be eslint");
    match entry.outcome() {
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("not installed locally"),
                "should say not installed: {cause}"
            );
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn js_local_eslint_runs() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();

    let bin = dir.path().join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    // Create fake eslint, prettier, biome, oxlint that exit 0
    for tool in ["eslint", "prettier", "biome", "oxlint"] {
        let script = bin.join(tool);
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let report = run_check_path(dir.path());
    // Check that all JavaScript stack tools pass
    for entry in report.results()[0].entries() {
        assert_passed(entry);
    }
}

// ─── ESLint: no config file → Passed (downgrade from error) ───────────────

#[cfg(unix)]
#[test]
fn eslint_no_config_returns_passed() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();

    let bin = dir.path().join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    // eslint should fail with config error (which gets downgraded to Passed)
    let script = bin.join("eslint");
    std::fs::write(
        &script,
        "#!/bin/sh\necho \"Oops! Something went wrong!\" >&2\necho \"ESLint couldn't find an eslint.config.(js|mjs|cjs) file.\" >&2\nexit 2\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // prettier, biome and oxlint should also be created (just pass)
    for tool in ["prettier", "biome", "oxlint"] {
        let script = bin.join(tool);
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let report = run_check_path(dir.path());
    // eslint should be downgraded to Passed due to no-config error
    let eslint_entry = report.results()[0]
        .entries()
        .iter()
        .find(|e| e.name() == "eslint");
    assert!(eslint_entry.is_some(), "should have eslint entry");
    assert_passed(eslint_entry.unwrap());
}

// ─── Walk-up: tool found in parent directory ──────────────────────────────

#[cfg(unix)]
#[test]
fn walk_up_finds_tool_in_parent() {
    use std::os::unix::fs::PermissionsExt;

    let parent = tempfile::tempdir().unwrap();
    // Create eslint, prettier, biome, oxlint in parent's node_modules/.bin
    let bin = parent.path().join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();

    // eslint outputs JSON array
    let script = bin.join("eslint");
    std::fs::write(&script, "#!/bin/sh\necho '[]'\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // prettier outputs nothing (no changes needed)
    let script = bin.join("prettier");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // biome outputs JSON object with diagnostics array
    let script = bin.join("biome");
    std::fs::write(&script, "#!/bin/sh\necho '{\"diagnostics\":[]}'\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // oxlint outputs JSON object with diagnostics array
    let script = bin.join("oxlint");
    std::fs::write(&script, "#!/bin/sh\necho '{\"diagnostics\":[]}'\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Create subdirectory as project (simulating a monorepo sub-package)
    let subdir = parent.path().join("subpackage");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(
        subdir.join("package.json"),
        r#"{"name": "subpkg", "version": "1.0.0"}"#,
    )
    .unwrap();

    let report = run_check_path(&subdir);
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    // All tools should be found via walk-up and pass
    for entry in report.results()[0].entries() {
        assert_passed(entry);
    }
}

// ─── Walk-up: tool not found anywhere → Error ─────────────────────────────

#[test]
fn walk_up_not_found_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();
    // No node_modules anywhere — tool will not be found

    let report = run_check_path(dir.path());
    assert!(
        report.detection_errors().is_empty(),
        "should not have detection errors"
    );
    let entry = &report.results()[0].entries()[0];
    assert_eq!(entry.name(), "eslint", "check should be eslint");
    match entry.outcome() {
        CheckOutcome::Error { cause, .. } => {
            assert!(
                cause.contains("not installed locally"),
                "expected 'not installed locally' error, got: {cause}"
            );
        }
        other => panic!("expected Error, got {other:?}"),
    }
}
