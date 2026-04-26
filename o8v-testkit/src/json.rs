//! JSON output types for E2E deserialization.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JsonOutput {
    pub results: Vec<JsonResult>,
    pub summary: JsonSummary,
}

#[derive(Debug, Deserialize)]
pub struct JsonResult {
    pub project: String,
    pub stack: String,
    pub checks: Vec<JsonCheck>,
}

#[derive(Debug, Deserialize)]
pub struct JsonCheck {
    pub name: String,
    pub outcome: String,
    pub diagnostics: Vec<JsonDiagnostic>,
}

#[derive(Debug, Deserialize)]
pub struct JsonDiagnostic {
    pub location: JsonLocation,
    pub rule: Option<String>,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct JsonLocation {
    pub path: String,
    #[serde(rename = "type")]
    pub location_type: String,
}

#[derive(Debug, Deserialize)]
pub struct JsonSummary {
    pub success: bool,
}
