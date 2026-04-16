// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Benchmark result persistence.
//!
//! Results are stored as NDJSON — one Observation per line.
//! Persistence is automatic: the pipeline calls `store.append()`,
//! not the test author.
//!
//! Uses `o8v_fs` for all file operations — same pattern as StorageSubscriber.

use std::path::{Path, PathBuf};
use o8v_fs::ContainmentRoot;
use super::types::{ExperimentResult, Observation};

const RESULTS_FILE: &str = "results.ndjson";

/// Manages benchmark result persistence.
///
/// Backed by a ContainmentRoot — all reads and writes go through o8v_fs.
/// Follows the same pattern as StorageSubscriber.
pub struct BenchmarkStore {
    containment: ContainmentRoot,
}

impl BenchmarkStore {
    /// Create a store at the given directory path.
    /// Creates the directory if it doesn't exist.
    pub fn at(dir: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let dir = dir.as_ref();
        // Bootstrap: create with raw fs — this IS the root we're establishing.
        std::fs::create_dir_all(dir)?;
        let canonical = std::fs::canonicalize(dir)?;
        let containment = ContainmentRoot::new(&canonical)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self { containment })
    }

    /// Create a store at `~/.8v/benchmark-results/`.
    pub fn open() -> Result<Self, std::io::Error> {
        let home = std::env::var("HOME")
            .map_err(|_| std::io::Error::other("HOME not set"))?;
        Self::at(PathBuf::from(home).join(".8v").join("benchmark-results"))
    }

    /// Append a Observation to the results file.
    /// File: `results.ndjson`. One line per record.
    ///
    /// Uses safe_append. If the file doesn't exist yet, falls back to
    /// safe_write to create it. Any other error is propagated immediately.
    pub fn append(&self, record: &Observation) -> Result<(), std::io::Error> {
        let path = self.results_path();
        let line = serde_json::to_string(record)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut data = line.into_bytes();
        data.push(b'\n');

        match o8v_fs::safe_append(&path, &self.containment, &data) {
            Ok(()) => Ok(()),
            Err(o8v_fs::FsError::NotFound { .. }) => {
                o8v_fs::safe_write(&path, &self.containment, &data)
                    .map_err(|e| std::io::Error::other(e.to_string()))
            }
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }

    /// Append an ExperimentResult to the experiments file.
    /// File: `experiments.ndjson`. One line per experiment.
    ///
    /// Same append semantics as `append()`.
    pub fn append_experiment(&self, result: &ExperimentResult) -> Result<(), std::io::Error> {
        let path = self.containment.as_path().join("experiments.ndjson");
        let line = serde_json::to_string(result)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut data = line.into_bytes();
        data.push(b'\n');

        match o8v_fs::safe_append(&path, &self.containment, &data) {
            Ok(()) => Ok(()),
            Err(o8v_fs::FsError::NotFound { .. }) => {
                o8v_fs::safe_write(&path, &self.containment, &data)
                    .map_err(|e| std::io::Error::other(e.to_string()))
            }
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }

    /// Read all stored Observations.
    ///
    /// Returns an empty vec if the file does not exist yet (valid state before
    /// first append). Any other error — permission denied, I/O error, etc. —
    /// is propagated immediately. Corrupt lines return an error with the line
    /// number to aid debugging.
    pub fn read_all(&self) -> Result<Vec<Observation>, std::io::Error> {
        let path = self.results_path();
        let content = match o8v_fs::safe_read(&path, &self.containment, &o8v_fs::FsConfig::default()) {
            Ok(c) => c.content().to_string(),
            Err(o8v_fs::FsError::NotFound { .. }) => return Ok(Vec::new()),
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };

        let mut records = Vec::new();
        for (i, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let record: Observation = serde_json::from_str(line)
                .map_err(|e| std::io::Error::other(format!("line {}: {e}", i + 1)))?;
            records.push(record);
        }
        Ok(records)
    }

    /// Write a structured report JSON for an experiment.
    /// File: `<experiment_name>/report.json`.
    pub fn write_report(&self, experiment_name: &str, report: &super::report::ReportJson) -> Result<PathBuf, std::io::Error> {
        let dir = self.containment.as_path().join(experiment_name);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("report.json");
        let json = serde_json::to_string_pretty(report)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        // Use ContainmentRoot rooted at the sub-dir for safe_write.
        let sub_containment = ContainmentRoot::new(&dir)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        o8v_fs::safe_write(&path, &sub_containment, json.as_bytes())
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(path)
    }

    /// The containment root for all fs operations.
    pub fn containment(&self) -> &ContainmentRoot {
        &self.containment
    }

    /// Path to the results NDJSON file.
    fn results_path(&self) -> PathBuf {
        self.containment.as_path().join(RESULTS_FILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::types::{TurnRecord, Verification};

    fn sample_record() -> Observation {
        Observation {
            scenario: "test-scenario".to_string(),
            task_name: "test-task".to_string(),
            timestamp_ms: 1000,
            git_commit: "abc123".to_string(),
            version: "0.1.0".to_string(),
            total_tokens: 5000,
            cost_usd: Some(0.05),
            exit_code: 0,
            tool_names: vec!["8v".to_string()],
            turns: vec![TurnRecord {
                role: "text".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }],
            init_message_bytes: 1024,
            response_text: "done".to_string(),
            event_count: 2,
            event_output_bytes: 400,
            event_command_bytes: 20,
            event_total_duration_ms: 50,
            verification: Verification {
                tests_pass: Some(true),
                check_pass: None,
                build_pass: None,
            },
            feedback: None,
            model: None,
            session_id: None,
            stop_reason: None,
            is_error: false,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            turn_count: 0,
            tool_calls_detail: vec![],
            agent_name: None,
            agent_version: None,
            mcp_protocol_version: None,
            agent_capabilities: vec![],
        }
    }

    #[test]
    fn append_creates_file_and_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();
        store.append(&sample_record()).unwrap();

        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].scenario, "test-scenario");
        assert_eq!(records[0].total_tokens, 5000);
    }

    #[test]
    fn append_multiple_records() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();
        store.append(&sample_record()).unwrap();
        store.append(&sample_record()).unwrap();

        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn read_all_empty_store() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();

        let records = store.read_all().unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn read_all_empty_file_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();
        // Write an empty file (valid state: file exists but has no records yet).
        let path = store.results_path();
        o8v_fs::safe_write(&path, store.containment(), b"").unwrap();

        let records = store.read_all().unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn read_all_corrupt_data_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();
        // Write a valid record followed by a corrupt line.
        let path = store.results_path();
        let mut content = serde_json::to_string(&sample_record()).unwrap();
        content.push('\n');
        content.push_str("not valid json\n");
        o8v_fs::safe_write(&path, store.containment(), content.as_bytes()).unwrap();

        let result = store.read_all();
        assert!(result.is_err(), "corrupt line must produce an error, not silently drop");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("line 2"), "error must identify the offending line: {msg}");
    }

    #[test]
    fn write_report_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BenchmarkStore::at(tmp.path()).unwrap();

        let report = super::super::report::ReportJson {
            schema_version: 1,
            experiment: "test-exp".into(),
            commit: "abc".into(),
            version_8v: "0.1.0".into(),
            started_ms: 1000,
            finished_ms: 2000,
            agent_name: Some("claude-code".into()),
            agent_version: Some("1.0.0".into()),
            model_id: None,
            mcp_protocol_version: None,
            task: super::super::report::TaskInfo {
                name: "test".into(),
                fixture: "test-fixture".into(),
                prompt_sha: "abc123".into(),
            },
            conditions: vec![],
            deltas_vs_control: vec![],
            confidence: super::super::report::Confidence {
                n_per_condition: 3,
                publishable: false,
                reason: "test".into(),
            },
            runs: vec![],
        };

        let path = store.write_report("test-exp", &report).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["experiment"], "test-exp");
        assert_eq!(parsed["agent_name"], "claude-code");
    }
}
