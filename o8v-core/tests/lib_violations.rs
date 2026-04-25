//! E2E violation tests — run real tools on fixtures with known violations.
//!
//! These tests assert on SPECIFIC diagnostics (rule, file, severity, message).
//! They are regression tests: a tool format change that breaks parsing shows up
//! as "expected diagnostic not found."
//!
//! Rust tests run unconditionally (cargo is always present).
//! Non-Rust tests use `#[ignore]` — CI runs them with `--include-ignored`.
//!
//! Existing `integration_real_tools.rs` tests are smoke tests ("does parsing work?").
//! These are regression tests ("does 8v find the right violations?").

use o8v_testkit::e2e_test;
use o8v_testkit::*;

// ─── JavaScript (#[ignore] — needs npm install in fixture) ────────────────

e2e_test!(
    #[ignore]
    javascript_violations_detected,
    "javascript-violations"
);

#[test]
#[ignore]
fn javascript_violations_eslint_diagnostics_are_structured() {
    let fixture = Fixture::e2e("javascript-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "eslint", "javascript");

    assert!(
        diags.len() >= 2,
        "expected at least 2 diagnostics, got {}",
        diags.len()
    );
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "eslint diagnostic must have a rule");
        assert!(d.span.is_some(), "eslint diagnostic must have a span");
        assert_eq!(d.tool, "eslint");
        assert_eq!(d.stack, "javascript");
    }
}

// ─── Deno (#[ignore] — needs deno installed) ──────────────────────────────

e2e_test!(
    #[ignore]
    deno_violations_detected,
    "deno-violations"
);

#[test]
#[ignore]
fn deno_violations_diagnostics_are_structured() {
    let fixture = Fixture::e2e("deno-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "deno check", "deno");

    assert!(!diags.is_empty(), "expected at least 1 diagnostic");
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "deno diagnostic must have a rule");
        assert_eq!(d.tool, "deno check");
        assert_eq!(d.stack, "deno");
    }
}

// ─── Dockerfile (#[ignore] — needs hadolint installed) ────────────────────

e2e_test!(
    #[ignore]
    dockerfile_violations_detected,
    "dockerfile-violations"
);

// ─── Python (#[ignore] — needs ruff installed) ────────────────────────────

e2e_test!(
    #[ignore]
    python_violations_detected,
    "python-violations"
);

#[test]
#[ignore]
fn python_violations_project_detected() {
    let fixture = Fixture::e2e("python-violations");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    assert_project_count(&report, 1);
    assert_eq!(report.results()[0].project_name(), "test-violations");
}

#[test]
#[ignore]
fn python_violations_ruff_diagnostics_are_structured() {
    let fixture = Fixture::e2e("python-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "ruff", "python");

    assert!(
        diags.len() >= 2,
        "expected at least 2 diagnostics, got {}",
        diags.len()
    );
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "ruff diagnostic must have a rule");
        assert!(d.span.is_some(), "ruff diagnostic must have a span");
        assert_eq!(d.tool, "ruff");
        assert_eq!(d.stack, "python");
    }
}

#[test]
#[ignore]
fn python_violations_diagnostics_are_sanitized() {
    let fixture = Fixture::e2e("python-violations");
    let report = run_check(&fixture);
    assert_sanitized(&report);
}

// ─── TypeScript (#[ignore] — needs npm install in fixture) ────────────────

e2e_test!(
    #[ignore]
    typescript_violations_detected,
    "typescript-violations"
);

#[test]
#[ignore]
fn typescript_violations_tsc_diagnostics_are_structured() {
    let fixture = Fixture::e2e("typescript-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "tsc", "typescript");

    assert!(!diags.is_empty(), "expected at least 1 diagnostic");
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "tsc diagnostic must have a rule");
        assert!(d.span.is_some(), "tsc diagnostic must have a span");
        assert_eq!(d.tool, "tsc");
        assert_eq!(d.stack, "typescript");
    }
}

// ─── .NET (#[ignore] — needs dotnet installed) ────────────────────────────

e2e_test!(
    #[ignore]
    dotnet_violations_detected,
    "dotnet-violations"
);

#[test]
#[ignore]
fn dotnet_violations_diagnostics_are_structured() {
    let fixture = Fixture::e2e("dotnet-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "dotnet build", "dotnet");

    assert!(!diags.is_empty(), "expected at least 1 diagnostic");
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "dotnet diagnostic must have a rule");
        assert_eq!(d.tool, "dotnet build");
        assert_eq!(d.stack, "dotnet");
    }
}

// ─── Ruby (#[ignore] — needs rubocop installed) ────────────────────────────

e2e_test!(
    #[ignore]
    ruby_violations_detected,
    "ruby-violations"
);

// ─── Go (#[ignore] — needs go installed) ──────────────────────────────────

e2e_test!(
    #[ignore]
    go_violations_detected,
    "go-violations"
);

#[test]
#[ignore]
fn go_violations_govet_diagnostics_are_structured() {
    let fixture = Fixture::e2e("go-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "go vet", "go");

    assert!(!diags.is_empty(), "expected at least 1 diagnostic");
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "go vet diagnostic must have a rule");
        assert_eq!(d.tool, "go vet");
        assert_eq!(d.stack, "go");
    }
}

// ─── Terraform (#[ignore] — needs tflint installed) ────────────────────────

e2e_test!(
    #[ignore]
    terraform_violations_detected,
    "terraform-violations"
);

// ─── Java (#[ignore] — needs Maven installed) ────────────────────────────

e2e_test!(
    #[ignore]
    java_violations_detected,
    "java-violations"
);

// ─── Kotlin (#[ignore] — needs ktlint installed) ─────────────────────────

e2e_test!(
    #[ignore]
    kotlin_violations_detected,
    "kotlin-violations"
);

// ─── Swift (#[ignore] — needs swiftlint installed) ────────────────────────

e2e_test!(
    #[ignore]
    swift_violations_detected,
    "swift-violations"
);

// ─── Helm (#[ignore] — needs helm installed) ──────────────────────────────

e2e_test!(
    #[ignore]
    helm_violations_detected,
    "helm-violations"
);

// ─── Kustomize (#[ignore] — needs kustomize installed) ────────────────────

e2e_test!(
    #[ignore]
    kustomize_violations_detected,
    "kustomize-violations"
);

// ─── Rust (always runs) ────────────────────────────────────────────────────

e2e_test!(rust_violations_detected, "rust-violations");

#[test]
fn rust_violations_all_checks_run() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let names = all_check_names(&report);

    assert!(
        names.contains(&"cargo check"),
        "missing cargo check: {names:?}"
    );
    assert!(names.contains(&"clippy"), "missing clippy: {names:?}");
    assert!(names.contains(&"cargo fmt"), "missing cargo fmt: {names:?}");
}

#[test]
fn rust_violations_project_detected() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);

    assert_no_detection_errors(&report);
    // rust-violations is a Cargo workspace root ("test-violations") with two member crates
    // ("app-crate", "lib-crate"). After the Bug A fix, all three are detected.
    assert_project_count(&report, 3);
    assert!(
        report
            .results()
            .iter()
            .any(|r| r.project_name() == "test-violations"),
        "workspace root 'test-violations' must be detected; got: {:?}",
        report
            .results()
            .iter()
            .map(|r| r.project_name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn rust_violations_clippy_diagnostics_are_structured() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "clippy", "rust");

    assert!(
        diags.len() >= 2,
        "expected at least 2 diagnostics, got {}",
        diags.len()
    );
    for d in &diags {
        assert!(!d.message.is_empty(), "diagnostic must have a message");
        assert!(d.rule.is_some(), "clippy diagnostic must have a rule");
        assert!(d.span.is_some(), "clippy diagnostic must have a span");
        assert_eq!(d.tool, "clippy");
        assert_eq!(d.stack, "rust");
    }
}

#[test]
fn rust_violations_diagnostics_have_suggestions() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    let diags = collect_diagnostics(&report, "clippy", "rust");

    let has_suggestion = diags.iter().any(|d| !d.suggestions.is_empty());
    assert!(
        has_suggestion,
        "at least one clippy diagnostic should have a suggestion"
    );
}

#[test]
fn rust_violations_diagnostics_are_sanitized() {
    let fixture = Fixture::e2e("rust-violations");
    let report = run_check(&fixture);
    assert_sanitized(&report);
}
