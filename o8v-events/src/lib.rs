// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! # o8v-events
//!
//! Event sourcing infrastructure for 8v. Persists check run data for trend tracking.
//!
//! Provides serialization and deserialization of the `series.json` format:
//! - `run_id`: unique identifier for the check run
//! - `timestamp`: unix milliseconds when the check executed
//! - `diagnostics`: HashMap of diagnostic_id → SeriesEntry (per-diagnostic metadata)

use std::collections::HashMap;

/// Metadata for a single diagnostic tracked across runs.
/// Captures file, rule, severity, and message for trend analysis.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SeriesEntry {
    /// File path where the diagnostic was found.
    pub file: String,
    /// Rule or check that produced this diagnostic.
    pub rule: String,
    /// Severity level (e.g. "Error", "Warning").
    pub severity: String,
    /// Human-readable message.
    pub message: String,
    /// Line number where the diagnostic was found.
    #[serde(default)]
    pub line: u32,
    /// Tool or linter that produced this diagnostic.
    #[serde(default)]
    pub tool: String,
    /// Stack trace or call context, if available.
    #[serde(default)]
    pub stack: String,
    /// Project name this diagnostic belongs to.
    #[serde(default)]
    pub project: String,
    /// Unix milliseconds when this diagnostic was first seen.
    pub first_seen: u64,
    /// How many runs this diagnostic has appeared in.
    pub run_count: u32,
}

/// Complete series.json structure.
/// Represents a single check run's diagnostics and metadata.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SeriesJson {
    /// Unique identifier for this check run.
    pub run_id: String,
    /// Unix milliseconds when the check executed.
    pub timestamp: u64,
    /// Run ID of the baseline (set by 8v init). None if no baseline.
    #[serde(default)]
    pub baseline_run_id: Option<String>,
    /// 8v version that produced this run.
    #[serde(default)]
    pub version: String,
    /// Git SHA at time of check, if inside a repo.
    #[serde(default)]
    pub git_sha: Option<String>,
    /// Per-diagnostic metadata: diagnostic_id → SeriesEntry.
    pub diagnostics: HashMap<String, SeriesEntry>,
}

impl SeriesJson {
    /// Create an empty series with the given run_id and timestamp.
    #[must_use]
    pub fn new(run_id: String, timestamp: u64) -> Self {
        Self {
            run_id,
            timestamp,
            baseline_run_id: None,
            version: String::new(),
            git_sha: None,
            diagnostics: HashMap::new(),
        }
    }
}

impl Default for SeriesJson {
    fn default() -> Self {
        Self::new(String::new(), 0)
    }
}

/// Normalize a diagnostic message for stable identity computation.
/// Strips volatile content (line numbers, column numbers, paths) so the
/// same logical error produces the same hash across line shifts.
///
/// Digit runs that are surrounded by non-alphanumeric characters are replaced
/// with `N`. Digits that are adjacent to ASCII letters (e.g. `u8`, `i32`,
/// `utf8`, `x86`) are preserved — they are part of identifiers or type names.
///
/// # Examples
///
/// ```
/// use o8v_events::normalize_message;
/// assert_eq!(normalize_message("error at (3,1): cannot find name"), "error at (N,N): cannot find name");
/// assert_eq!(normalize_message("line 42, column 5"), "line N, column N");
/// assert_eq!(normalize_message("unused variable `x`"), "unused variable `x`");
/// assert_eq!(normalize_message("expected u8 found i32"), "expected u8 found i32");
/// ```
pub fn normalize_message(msg: &str) -> String {
    let mut result = String::with_capacity(msg.len());
    let chars: Vec<char> = msg.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            // Check if preceded by an ASCII letter (part of identifier like `u8`, `i32`, `x86`)
            let prev_is_alpha = i > 0 && chars[i - 1].is_ascii_alphabetic();
            if prev_is_alpha {
                // Keep the entire digit run — all digits are part of the identifier
                while i < chars.len() && chars[i].is_ascii_digit() {
                    result.push(chars[i]);
                    i += 1;
                }
            } else {
                // Consume the entire digit run
                let run_start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                // If the digit run is followed by an ASCII letter it is a numeric prefix
                // to an identifier (uncommon but possible — e.g. `3d`). Keep it as-is.
                let next_is_alpha = i < chars.len() && chars[i].is_ascii_alphabetic();
                if next_is_alpha {
                    for &ch in &chars[run_start..i] {
                        result.push(ch);
                    }
                } else {
                    result.push('N');
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Deserialize series.json from bytes.
///
/// Returns a `SeriesJson` if the input is valid JSON.
/// On parse error, returns the error from serde_json.
///
/// # Errors
///
/// Returns `serde_json::Error` if the JSON is malformed.
pub fn parse_series(bytes: &[u8]) -> Result<SeriesJson, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// Serialize series.json to bytes.
///
/// # Errors
///
/// Returns `serde_json::Error` if serialization fails (unlikely with well-formed data).
pub fn serialize_series(series: &SeriesJson) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_message_parens_coords() {
        assert_eq!(
            normalize_message("error at (3,1): cannot find name"),
            "error at (N,N): cannot find name"
        );
    }

    #[test]
    fn test_normalize_message_ts_coords() {
        // TS2304: "2" follows alpha "S", so the entire digit run "2304" is preserved.
        // Result: TS2304.
        assert_eq!(
            normalize_message("src/index.ts(7,5): error TS2304"),
            "src/index.ts(N,N): error TS2304"
        );
    }

    #[test]
    fn test_normalize_message_no_digits() {
        assert_eq!(
            normalize_message("unused variable `x`"),
            "unused variable `x`"
        );
    }

    #[test]
    fn test_normalize_message_line_column() {
        assert_eq!(normalize_message("line 42, column 5"), "line N, column N");
    }

    #[test]
    fn test_normalize_message_preserves_type_identifiers() {
        // u8: "8" follows alpha "u" → entire run kept. Result: u8.
        // i32: "3" follows alpha "i" → entire run "32" kept. Result: i32.
        assert_eq!(
            normalize_message("expected u8 found i32"),
            "expected u8 found i32"
        );
    }

    #[test]
    fn test_normalize_message_type_names_in_sentence() {
        assert_eq!(
            normalize_message("expected i32 found u8"),
            "expected i32 found u8"
        );
    }

    #[test]
    fn test_series_entry_roundtrip() {
        let entry = SeriesEntry {
            file: "src/main.rs".to_string(),
            rule: "unused-variable".to_string(),
            severity: "Warning".to_string(),
            message: "unused variable x".to_string(),
            line: 42,
            tool: "clippy".to_string(),
            stack: "main::foo".to_string(),
            project: "o8v-cli".to_string(),
            first_seen: 1712425200000,
            run_count: 5,
        };

        let json = serde_json::to_string(&entry).expect("serialize failed");
        let deserialized: SeriesEntry = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(deserialized.file, entry.file);
        assert_eq!(deserialized.rule, entry.rule);
        assert_eq!(deserialized.severity, entry.severity);
        assert_eq!(deserialized.message, entry.message);
        assert_eq!(deserialized.line, entry.line);
        assert_eq!(deserialized.tool, entry.tool);
        assert_eq!(deserialized.stack, entry.stack);
        assert_eq!(deserialized.project, entry.project);
        assert_eq!(deserialized.first_seen, entry.first_seen);
        assert_eq!(deserialized.run_count, entry.run_count);
    }

    #[test]
    fn test_series_json_default() {
        let series = SeriesJson::default();
        assert!(series.diagnostics.is_empty());
        assert_eq!(series.timestamp, 0);
    }

    #[test]
    fn test_parse_series_bytes() {
        let json_bytes = br#"{
  "run_id": "run-001",
  "timestamp": 1712425200000,
  "baseline_run_id": "run-000",
  "version": "0.1.0",
  "git_sha": "abc123def456",
  "diagnostics": {
    "abc123": {
      "file": "src/main.rs",
      "rule": "unused-var",
      "severity": "Warning",
      "message": "unused variable",
      "line": 10,
      "tool": "clippy",
      "stack": "",
      "project": "o8v-cli",
      "first_seen": 1712425200000,
      "run_count": 1
    }
  }
}"#;
        let series = parse_series(json_bytes).expect("parse failed");
        assert_eq!(series.run_id, "run-001");
        assert_eq!(series.diagnostics.len(), 1);
        assert_eq!(series.diagnostics["abc123"].run_count, 1);
    }

    #[test]
    fn test_serialize_series() {
        let mut diags = HashMap::new();
        diags.insert(
            "id1".to_string(),
            SeriesEntry {
                file: "test.rs".to_string(),
                rule: "test-rule".to_string(),
                severity: "Error".to_string(),
                message: "test message".to_string(),
                line: 7,
                tool: "rustc".to_string(),
                stack: String::new(),
                project: "o8v-events".to_string(),
                first_seen: 1712425200000,
                run_count: 1,
            },
        );

        let series = SeriesJson {
            run_id: "run-001".to_string(),
            timestamp: 1712425200000,
            baseline_run_id: Some("run-000".to_string()),
            version: "0.1.0".to_string(),
            git_sha: Some("deadbeef".to_string()),
            diagnostics: diags,
        };

        let bytes = serialize_series(&series).expect("serialize failed");
        let json_str = String::from_utf8(bytes).expect("utf8 failed");
        assert!(json_str.contains("run-001"));
        assert!(json_str.contains("test.rs"));
    }
}
