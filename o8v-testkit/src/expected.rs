//! EXPECTED.toml parsing — load test expectations from fixtures.

use crate::Fixture;
use o8v_core::Severity;
use serde::Deserialize;

/// Expected checks loaded from EXPECTED.toml (multi-tool format).
#[derive(Debug, Deserialize)]
pub struct Expected {
    pub stack: String,
    #[serde(rename = "check")]
    pub checks: Vec<ExpectedCheck>,
}

/// One expected check (tool) from EXPECTED.toml.
#[derive(Debug, Deserialize)]
pub struct ExpectedCheck {
    pub tool: String,
    #[serde(default, rename = "diagnostic")]
    pub diagnostics: Vec<ExpectedDiagnostic>,
}

impl Expected {
    /// Load from a fixture's EXPECTED.toml file.
    #[must_use]
    pub fn load(fixture: &Fixture) -> Self {
        let path = fixture.path().join("EXPECTED.toml");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => panic!("EXPECTED.toml not found at {}: {e}", path.display()),
        };
        match toml::from_str(&content) {
            Ok(expected) => expected,
            Err(e) => panic!("invalid EXPECTED.toml at {}: {e}", path.display()),
        }
    }
}

/// One expected diagnostic from EXPECTED.toml.
#[derive(Debug, Deserialize)]
pub struct ExpectedDiagnostic {
    pub rule: Option<String>,
    pub file: String,
    pub severity: String,
    pub message_contains: Option<String>,
}

impl ExpectedDiagnostic {
    /// Parse the severity string into the enum.
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self.severity.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "info" => Severity::Info,
            "hint" => Severity::Hint,
            other => panic!("unknown severity in EXPECTED.toml: '{other}'"),
        }
    }
}
