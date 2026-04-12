//! Check trait and result types.

use crate::diagnostic::{sanitize, Diagnostic, ParseStatus};
use o8v_fs::ContainmentRoot;
use o8v_project::{ProjectRoot, Stack};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Configuration for a check run.
pub struct CheckConfig {
    /// Per-check timeout. `None` uses the stack default (5 minutes).
    pub timeout: Option<Duration>,
    /// Shared interruption flag — set by signal handler on Ctrl+C.
    pub interrupted: &'static AtomicBool,
}

/// Events emitted during check execution for streaming progress.
pub enum CheckEvent<'a> {
    /// Detection error found. Emitted before any checks.
    DetectionError { error: &'a o8v_project::DetectError },
    /// Project detected, checks about to start.
    ProjectStart {
        name: &'a str,
        stack: Stack,
        path: &'a ProjectRoot,
    },
    /// Individual check starting.
    CheckStart { name: &'a str },
    /// Individual check completed — full result available.
    CheckDone { entry: &'a CheckEntry },
}

/// Execution context passed to every check at run time.
///
/// Contains state that is not known at check construction time —
/// effective timeout (which depends on the user's `--timeout` cap and
/// the stack default) and the shared interruption flag.
///
/// Checks use `ctx.timeout` for their tool invocations. The interrupted
/// flag is checked between checks in the runner loop; individual checks
/// may also poll it for cooperative cancellation.
pub struct CheckContext {
    /// Effective timeout for this check's tool invocation.
    pub timeout: Duration,
    /// Shared flag — set by signal handler when the user hits Ctrl-C.
    /// `'static` because signal handlers require it, and the flag is
    /// leaked from an Arc in main.rs to live for the entire process.
    pub interrupted: &'static AtomicBool,
}

/// Outcome of running a single check.
#[derive(Debug)]
#[non_exhaustive]
pub enum CheckOutcome {
    /// Tool exited successfully and no diagnostics were found.
    /// The `diagnostics` vec is always empty — `enrich()` promotes any
    /// exit-0 result with diagnostics to `Failed`.
    Passed {
        diagnostics: Vec<Diagnostic>,
        raw_stdout: String,
        raw_stderr: String,
        parse_status: ParseStatus,
        stdout_truncated: bool,
        stderr_truncated: bool,
    },
    /// Tool exited with failure, or diagnostics found on exit 0.
    Failed {
        code: Option<i32>,
        diagnostics: Vec<Diagnostic>,
        raw_stdout: String,
        raw_stderr: String,
        parse_status: ParseStatus,
        stdout_truncated: bool,
        stderr_truncated: bool,
    },
    /// Something went wrong — tool or 8v itself.
    Error {
        kind: ErrorKind,
        cause: String,
        raw_stdout: String,
        raw_stderr: String,
    },
}

impl CheckOutcome {
    /// Construct a Passed outcome with the given output.
    /// Diagnostics are always empty — enrich() promotes to Failed if diagnostics are found.
    #[must_use]
    pub fn passed(
        raw_stdout: String,
        raw_stderr: String,
        parse_status: ParseStatus,
        stdout_truncated: bool,
        stderr_truncated: bool,
    ) -> Self {
        Self::Passed {
            diagnostics: Vec::new(),
            raw_stdout,
            raw_stderr,
            parse_status,
            stdout_truncated,
            stderr_truncated,
        }
    }

    /// Construct a Failed outcome with diagnostics and output.
    #[must_use]
    pub fn failed(
        code: Option<i32>,
        diagnostics: Vec<Diagnostic>,
        raw_stdout: String,
        raw_stderr: String,
        parse_status: ParseStatus,
        stdout_truncated: bool,
        stderr_truncated: bool,
    ) -> Self {
        Self::Failed {
            code,
            diagnostics,
            raw_stdout,
            raw_stderr,
            parse_status,
            stdout_truncated,
            stderr_truncated,
        }
    }

    /// Construct an Error outcome with no captured output.
    #[must_use]
    pub fn error(kind: ErrorKind, cause: String) -> Self {
        Self::Error {
            kind,
            cause: sanitize(&cause),
            raw_stdout: String::new(),
            raw_stderr: String::new(),
        }
    }

    /// Construct an Error outcome with captured output.
    #[must_use]
    pub fn error_with_output(
        kind: ErrorKind,
        cause: String,
        raw_stdout: String,
        raw_stderr: String,
    ) -> Self {
        Self::Error {
            kind,
            cause: sanitize(&cause),
            raw_stdout,
            raw_stderr,
        }
    }
}

/// What class of error occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Tool could not run — spawn failure, timeout, signal death.
    Runtime,
    /// Tool ran successfully but 8v could not verify the result.
    /// The parser failed on non-empty output from an exit-0 tool.
    Verification,
}

/// A single check that can be run against a project directory.
///
/// Implemented by external tool checks (clippy, ruff, tsc) and
/// by 8v's own rules. The runner
/// treats both the same.
pub trait Check {
    /// Human-readable name of this check (e.g. "clippy", "no-silent-fallback").
    fn name(&self) -> &'static str;

    /// Run the check against a validated project directory. Returns the outcome.
    ///
    /// `project_dir` is the security primitive — all fs operations are contained
    /// within it. `ctx` carries execution-time state.
    fn run(&self, project_dir: &ContainmentRoot, ctx: &CheckContext) -> CheckOutcome;
}

/// One check's name, outcome, and duration.
#[derive(Debug)]
pub struct CheckEntry {
    pub name: String,
    pub outcome: CheckOutcome,
    pub duration: std::time::Duration,
}

impl CheckEntry {
    /// Check name (e.g. "clippy", "tsc").
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What happened.
    #[must_use]
    pub const fn outcome(&self) -> &CheckOutcome {
        &self.outcome
    }

    /// How long the check took.
    #[must_use]
    pub const fn duration(&self) -> std::time::Duration {
        self.duration
    }
}

/// Result of checking one project.
#[derive(Debug)]
pub struct CheckResult {
    pub project_name: String,
    pub project_path: ProjectRoot,
    pub stack: o8v_project::Stack,
    pub entries: Vec<CheckEntry>,
}

impl CheckResult {
    /// Project name.
    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    /// Project path.
    #[must_use]
    pub const fn project_path(&self) -> &ProjectRoot {
        &self.project_path
    }

    /// Stack that was detected.
    #[must_use]
    pub const fn stack(&self) -> o8v_project::Stack {
        self.stack
    }

    /// All check entries — name, outcome, and duration.
    #[must_use]
    pub fn entries(&self) -> &[CheckEntry] {
        &self.entries
    }

    /// True if at least one check ran and all checks passed.
    /// Returns false for zero checks — nothing checked is not "ok".
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.entries.is_empty()
            && self
                .entries
                .iter()
                .all(|e| matches!(e.outcome, CheckOutcome::Passed { .. }))
    }

    /// Total duration of all checks.
    #[must_use]
    pub fn total_duration(&self) -> std::time::Duration {
        self.entries.iter().map(|e| e.duration).sum()
    }
}

/// Summary of changes compared to previous run's diagnostics.
#[derive(Debug, Clone)]
pub struct DeltaSummary {
    /// Diagnostics not seen in the previous run.
    pub new: usize,
    /// Diagnostics in the previous run but not in the current run.
    pub fixed: usize,
    /// Diagnostics present in both runs.
    pub unchanged: usize,
}

/// Result of checking a directory — may contain multiple projects.
#[derive(Debug)]
pub struct CheckReport {
    pub results: Vec<CheckResult>,
    pub detection_errors: Vec<o8v_project::DetectError>,
    /// Delta compared to previous run. None if no previous data available.
    pub delta: Option<DeltaSummary>,
    /// Render configuration — set by the command from CLI args before returning.
    pub render_config: crate::render::RenderConfig,
}

impl CheckReport {
    /// Per-project results.
    #[must_use]
    pub fn results(&self) -> &[CheckResult] {
        &self.results
    }

    /// Errors from project detection itself — structured, not stringified.
    #[must_use]
    pub fn detection_errors(&self) -> &[o8v_project::DetectError] {
        &self.detection_errors
    }

    /// True if at least one project was checked, no detection errors, and all passed.
    /// Empty report (no projects, no errors) is NOT ok — nothing was checked.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.results.is_empty()
            && self.detection_errors.is_empty()
            && self.results.iter().all(CheckResult::is_ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(outcomes: Vec<CheckOutcome>) -> CheckResult {
        let dir = tempfile::tempdir().unwrap();
        let path = ProjectRoot::new(dir.path()).unwrap();
        CheckResult {
            project_name: "test".to_string(),
            project_path: path,
            stack: o8v_project::Stack::Rust,
            entries: outcomes
                .into_iter()
                .enumerate()
                .map(|(i, o)| CheckEntry {
                    name: format!("check-{i}"),
                    outcome: o,
                    duration: std::time::Duration::ZERO,
                })
                .collect(),
        }
    }

    fn passed() -> CheckOutcome {
        CheckOutcome::passed(
            String::new(),
            String::new(),
            ParseStatus::Parsed,
            false,
            false,
        )
    }

    fn failed(output: &str) -> CheckOutcome {
        CheckOutcome::failed(
            None,
            vec![],
            output.to_string(),
            String::new(),
            ParseStatus::Unparsed,
            false,
            false,
        )
    }

    fn error(cause: &str) -> CheckOutcome {
        CheckOutcome::error(ErrorKind::Runtime, cause.to_string())
    }

    #[test]
    fn result_is_ok_when_all_passed() {
        let r = make_result(vec![passed(), passed()]);
        assert!(r.is_ok());
    }

    #[test]
    fn result_not_ok_when_any_failed() {
        let r = make_result(vec![passed(), failed("err")]);
        assert!(!r.is_ok());
    }

    #[test]
    fn result_not_ok_when_any_error() {
        let r = make_result(vec![passed(), error("boom")]);
        assert!(!r.is_ok());
    }

    #[test]
    fn result_accessors() {
        let r = make_result(vec![passed()]);
        assert_eq!(r.project_name(), "test");
        assert_eq!(r.stack(), o8v_project::Stack::Rust);
        assert_eq!(r.entries().len(), 1);
        assert!(!r.project_path().to_string().is_empty());
    }

    #[test]
    fn report_is_ok_when_clean() {
        let report = CheckReport {
            results: vec![make_result(vec![passed()])],
            detection_errors: vec![],
            delta: None,
            render_config: crate::render::RenderConfig::default(),
        };
        assert!(report.is_ok());
    }

    #[test]
    fn report_not_ok_with_detection_errors() {
        let report = CheckReport {
            results: vec![make_result(vec![passed()])],
            detection_errors: vec![o8v_project::DetectError::ManifestInvalid {
                path: std::path::PathBuf::from("/fake"),
                cause: "test".into(),
            }],
            delta: None,
            render_config: crate::render::RenderConfig::default(),
        };
        assert!(!report.is_ok());
    }

    #[test]
    fn report_not_ok_with_failed_check() {
        let report = CheckReport {
            results: vec![make_result(vec![failed("err")])],
            detection_errors: vec![],
            delta: None,
            render_config: crate::render::RenderConfig::default(),
        };
        assert!(!report.is_ok());
    }
}
