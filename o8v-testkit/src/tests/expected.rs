// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::*;

#[test]
fn expected_loads_from_fixture() {
    let fixture = Fixture::e2e("rust-violations");
    let expected = Expected::load(&fixture);
    assert_eq!(expected.stack, "rust", "stack should be rust");
    assert!(
        !expected.checks.is_empty(),
        "should have at least one [[check]]"
    );
    assert_eq!(
        expected.checks[0].tool, "clippy",
        "first check should be clippy"
    );
    assert!(
        !expected.checks[0].diagnostics.is_empty(),
        "clippy check should have diagnostics"
    );
}

#[test]
fn expected_diagnostic_severity_parsing() {
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "error".to_string(),
        message_contains: None,
    };
    assert_eq!(exp.severity(), Severity::Error);

    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "warning".to_string(),
        message_contains: None,
    };
    assert_eq!(exp.severity(), Severity::Warning);
}

#[test]
#[should_panic(expected = "unknown severity")]
fn expected_diagnostic_unknown_severity_panics() {
    let exp = ExpectedDiagnostic {
        rule: None,
        file: String::new(),
        severity: "critical".to_string(),
        message_contains: None,
    };
    let _ = exp.severity();
}
