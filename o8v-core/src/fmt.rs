// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Fmt value types — FmtOutcome, FmtEntry, FmtReport, FmtConfig.
//!
//! These types live here because render depends on them. The fmt()
//! orchestration function lives in o8v-stacks.

use o8v_project::{ProjectRoot, Stack};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Outcome of running a single formatter.
#[derive(Debug, Clone)]
pub enum FmtOutcome {
    /// Formatter succeeded (files formatted or already formatted).
    Ok { duration: Duration },
    /// In check mode: files need formatting.
    Dirty { duration: Duration },
    /// Formatter failed or was interrupted.
    Error { cause: String, stderr: String },
    /// Formatter binary not found.
    NotFound { program: String },
}

impl FmtOutcome {
    /// True if the outcome is Ok (files formatted or already clean).
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, FmtOutcome::Ok { .. })
    }
}

impl std::fmt::Display for FmtOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok { duration } => write!(f, "ok ({:?})", duration),
            Self::Dirty { duration } => write!(f, "dirty ({:?})", duration),
            Self::Error { cause, .. } => write!(f, "error: {}", cause),
            Self::NotFound { program } => write!(f, "not found: {}", program),
        }
    }
}

/// Entry in a FmtReport — result of running a formatter on one project.
#[derive(Debug, Clone)]
pub struct FmtEntry {
    /// The stack that was formatted.
    pub stack: Stack,
    /// Project root where the formatter was run.
    pub project_root: ProjectRoot,
    /// Formatter tool name (e.g., "cargo fmt", "prettier").
    pub tool: String,
    /// Outcome of running the formatter.
    pub outcome: FmtOutcome,
}

/// Report of all formatting operations.
#[derive(Debug)]
pub struct FmtReport {
    /// One entry per formatted project.
    pub entries: Vec<FmtEntry>,
    /// Errors encountered during project detection.
    pub detection_errors: Vec<o8v_project::DetectError>,
}

impl FmtReport {
    /// True if all formatters succeeded (or --check: no changes needed) and no detection errors.
    /// Empty report (no projects, no errors) is NOT ok — nothing was checked.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.entries.is_empty()
            && self.detection_errors.is_empty()
            && self.entries.iter().all(|e| e.outcome.is_ok())
    }
}

/// Configuration for formatting operations.
pub struct FmtConfig {
    /// Timeout per formatter. None uses the default (5 minutes).
    pub timeout: Option<Duration>,
    /// If true, check mode only — don't write files, report if dirty.
    pub check_mode: bool,
    /// Shared interruption flag — set by signal handler on Ctrl+C.
    pub interrupted: &'static AtomicBool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_outcome_is_ok_for_ok() {
        let outcome = FmtOutcome::Ok {
            duration: Duration::from_secs(1),
        };
        assert!(outcome.is_ok());
    }

    #[test]
    fn fmt_outcome_not_ok_for_dirty() {
        let outcome = FmtOutcome::Dirty {
            duration: Duration::from_secs(1),
        };
        assert!(!outcome.is_ok());
    }

    #[test]
    fn fmt_outcome_not_ok_for_error() {
        let outcome = FmtOutcome::Error {
            cause: "test error".to_string(),
            stderr: String::new(),
        };
        assert!(!outcome.is_ok());
    }

    #[test]
    fn fmt_outcome_not_ok_for_not_found() {
        let outcome = FmtOutcome::NotFound {
            program: "missing".to_string(),
        };
        assert!(!outcome.is_ok());
    }

    #[test]
    fn fmt_report_is_ok_when_empty() {
        let report = FmtReport {
            entries: vec![],
            detection_errors: vec![],
        };
        assert!(!report.is_ok()); // Empty report is not ok
    }

    #[test]
    fn fmt_report_is_ok_when_all_ok() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Rust,
                project_root: root,
                tool: "cargo".to_string(),
                outcome: FmtOutcome::Ok {
                    duration: Duration::from_secs(1),
                },
            }],
            detection_errors: vec![],
        };
        assert!(report.is_ok());
    }

    #[test]
    fn fmt_report_not_ok_with_detection_errors() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Rust,
                project_root: root,
                tool: "cargo".to_string(),
                outcome: FmtOutcome::Ok {
                    duration: Duration::from_secs(1),
                },
            }],
            detection_errors: vec![o8v_project::DetectError::ManifestInvalid {
                path: std::path::PathBuf::from("/fake"),
                cause: "test".into(),
            }],
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn fmt_report_not_ok_with_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Rust,
                project_root: root,
                tool: "cargo".to_string(),
                outcome: FmtOutcome::Dirty {
                    duration: Duration::from_secs(1),
                },
            }],
            detection_errors: vec![],
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn fmt_report_not_ok_with_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Rust,
                project_root: root,
                tool: "cargo".to_string(),
                outcome: FmtOutcome::Error {
                    cause: "test error".to_string(),
                    stderr: String::new(),
                },
            }],
            detection_errors: vec![],
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn fmt_entry_construction_ok() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();
        let entry = FmtEntry {
            stack: Stack::Rust,
            project_root: root,
            tool: "cargo fmt".to_string(),
            outcome: FmtOutcome::Ok {
                duration: Duration::from_secs(1),
            },
        };
        assert_eq!(entry.stack, Stack::Rust);
        assert_eq!(entry.tool, "cargo fmt");
        assert!(entry.outcome.is_ok());
    }

    #[test]
    fn fmt_entry_construction_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();
        let entry = FmtEntry {
            stack: Stack::Python,
            project_root: root,
            tool: "ruff".to_string(),
            outcome: FmtOutcome::Dirty {
                duration: Duration::from_millis(500),
            },
        };
        assert_eq!(entry.stack, Stack::Python);
        assert!(!entry.outcome.is_ok());
    }

    #[test]
    fn fmt_entry_construction_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();
        let entry = FmtEntry {
            stack: Stack::Go,
            project_root: root,
            tool: "gofmt".to_string(),
            outcome: FmtOutcome::Error {
                cause: "test error".to_string(),
                stderr: "stderr msg".to_string(),
            },
        };
        assert_eq!(entry.stack, Stack::Go);
        assert!(!entry.outcome.is_ok());
    }

    #[test]
    fn fmt_entry_construction_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();
        let entry = FmtEntry {
            stack: Stack::TypeScript,
            project_root: root,
            tool: "prettier".to_string(),
            outcome: FmtOutcome::NotFound {
                program: "prettier".to_string(),
            },
        };
        assert_eq!(entry.stack, Stack::TypeScript);
        assert!(!entry.outcome.is_ok());
    }

    #[test]
    fn fmt_report_with_all_ok_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![
                FmtEntry {
                    stack: Stack::Rust,
                    project_root: root.clone(),
                    tool: "cargo".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(100),
                    },
                },
                FmtEntry {
                    stack: Stack::Python,
                    project_root: root,
                    tool: "ruff".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(50),
                    },
                },
            ],
            detection_errors: vec![],
        };
        assert!(report.is_ok());
    }

    #[test]
    fn fmt_report_with_one_error_mixed() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![
                FmtEntry {
                    stack: Stack::Rust,
                    project_root: root.clone(),
                    tool: "cargo".to_string(),
                    outcome: FmtOutcome::Ok {
                        duration: Duration::from_millis(100),
                    },
                },
                FmtEntry {
                    stack: Stack::Python,
                    project_root: root,
                    tool: "ruff".to_string(),
                    outcome: FmtOutcome::Error {
                        cause: "formatter failed".to_string(),
                        stderr: String::new(),
                    },
                },
            ],
            detection_errors: vec![],
        };
        assert!(!report.is_ok()); // One error makes report not ok
    }

    #[test]
    fn fmt_report_with_dirty_outcome_is_not_ok() {
        let dir = tempfile::tempdir().unwrap();
        let root = ProjectRoot::new(dir.path()).unwrap();

        let report = FmtReport {
            entries: vec![FmtEntry {
                stack: Stack::Go,
                project_root: root,
                tool: "gofmt".to_string(),
                outcome: FmtOutcome::Dirty {
                    duration: Duration::from_millis(200),
                },
            }],
            detection_errors: vec![],
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn fmt_outcome_display_ok() {
        let outcome = FmtOutcome::Ok {
            duration: Duration::from_secs(1),
        };
        let display_str = outcome.to_string();
        assert!(display_str.contains("ok"));
        assert!(display_str.contains("1s"));
    }

    #[test]
    fn fmt_outcome_display_dirty() {
        let outcome = FmtOutcome::Dirty {
            duration: Duration::from_millis(500),
        };
        let display_str = outcome.to_string();
        assert!(display_str.contains("dirty"));
        assert!(display_str.contains("500ms"));
    }

    #[test]
    fn fmt_outcome_display_error() {
        let outcome = FmtOutcome::Error {
            cause: "test failed".to_string(),
            stderr: String::new(),
        };
        let display_str = outcome.to_string();
        assert!(display_str.contains("error"));
        assert!(display_str.contains("test failed"));
    }

    #[test]
    fn fmt_outcome_display_not_found() {
        let outcome = FmtOutcome::NotFound {
            program: "missing_tool".to_string(),
        };
        let display_str = outcome.to_string();
        assert!(display_str.contains("not found"));
        assert!(display_str.contains("missing_tool"));
    }
}
