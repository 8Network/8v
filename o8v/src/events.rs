//! Event sourcing infrastructure — persist check results for trend analysis.
//!
//! `EventWriter` captures check events and persists them to:
//! - `~/.8v/events/`: NDJSON event logs (created on open; written per-diagnostic)
//! - `~/.8v/series.json`: aggregated series state (written on finalize)
//!
//! All path knowledge lives in `StorageDir` (`o8v-workspace`).
//!
//! Design:
//! - No error propagation: check never fails because of events.
//! - Atomic writes: `.tmp` → safe_rename pattern ensures consistency.
//! - File content caching: avoid repeated reads for span extraction.
//! - SHA256 diagnostic IDs: deterministic deduplication across runs.
//! - Only Location::File paths read for span content — Absolute is outside the project.
//! - All filesystem operations go through o8v-fs (containment + symlink protection).
//! - message/rule are DisplayStr (pre-sanitized at source) — no re-sanitization here.

use std::collections::HashMap;
use std::io::Write;
use tracing::warn;

// ─── EventWriter ─────────────────────────────────────────────────────────────

/// Event writer — captures check events and persists series state.
///
/// Two modes:
/// - Active: `.8v/` was found; diagnostics are accumulated and persisted.
/// - No-op: `.8v/` not found or open failed; all methods are silent no-ops.
pub struct EventWriter {
    inner: Option<ActiveWriter>,
}

struct ActiveWriter {
    storage: o8v_workspace::StorageDir,
    /// Containment boundary for source file reads (project root)
    project_root: o8v_fs::ContainmentRoot,
    run_id: String,
    timestamp: u64,
    /// Open NDJSON event log — None if events/ doesn't exist
    event_file: Option<std::fs::File>,
    file_cache: HashMap<String, String>,
    diagnostics: HashMap<String, o8v_events::SeriesEntry>,
}

impl EventWriter {
    /// Open a new event writer.
    ///
    /// Opens `StorageDir` at `~/.8v/` for event and series storage.
    /// Returns Err if `~/.8v/` cannot be opened.
    /// Caller falls back to `no_op()`.
    pub fn open(project_root: &o8v_fs::ContainmentRoot) -> Result<Self, std::io::Error> {
        let storage = o8v_workspace::StorageDir::open()?;

        let run_id = crate::util::new_uuid();
        let timestamp = crate::util::unix_ms();

        // Open event log. Failure is acceptable — series.json still gets written without it.
        let event_file = try_open_event_log(&storage.event_log(&run_id), storage.containment());

        Ok(Self {
            inner: Some(ActiveWriter {
                storage,
                project_root: project_root.clone(),
                run_id,
                timestamp,
                event_file,
                file_cache: HashMap::new(),
                diagnostics: HashMap::new(),
            }),
        })
    }

    /// Return a no-op writer — all operations are silent no-ops.
    pub fn no_op() -> Self {
        Self { inner: None }
    }

    /// Record a diagnostic.
    pub fn on_event(
        &mut self,
        diagnostic: &o8v_core::Diagnostic,
        tool: &str,
        stack: &str,
        project: &str,
    ) {
        let Some(ref mut inner) = self.inner else {
            return;
        };
        inner.on_diagnostic(diagnostic, tool, stack, project);
    }

    /// Write series.json and close event log. Best-effort: errors are silent.
    ///
    /// Returns the updated `SeriesJson` on success, or `None` if the writer is
    /// no-op or if writing fails.
    pub fn finalize(&mut self, _report: &o8v_core::CheckReport) -> Option<o8v_events::SeriesJson> {
        let Some(inner) = self.inner.take() else {
            return None;
        };
        inner.finalize_inner()
    }
}

// ─── ActiveWriter ─────────────────────────────────────────────────────────────

impl ActiveWriter {
    const DIAGNOSTIC_ID_LEN: usize = 16;

    fn on_diagnostic(
        &mut self,
        diagnostic: &o8v_core::Diagnostic,
        tool: &str,
        stack: &str,
        project: &str,
    ) {
        let diag_id = self.compute_diagnostic_id(diagnostic);

        let line = diagnostic.span.as_ref().map(|s| s.line).unwrap_or(0);

        let entry = o8v_events::SeriesEntry {
            file: loc_str(&diagnostic.location),
            rule: diagnostic.rule.as_deref().unwrap_or("").to_string(),
            severity: format!("{:?}", diagnostic.severity),
            message: diagnostic.message.to_string(),
            line,
            tool: tool.to_string(),
            stack: stack.to_string(),
            project: project.to_string(),
            first_seen: self.timestamp,
            run_count: 0, // Set during finalize merge
        };

        self.diagnostics.insert(diag_id.clone(), entry);
        if let Err(e) = self.write_event_log(diagnostic, &diag_id) {
            tracing::debug!(error = ?e, "events: could not write to event log");
        }
    }

    fn finalize_inner(mut self) -> Option<o8v_events::SeriesJson> {
        drop(self.event_file.take());

        let series_path = self.storage.series_json();
        let tmp_path = self.storage.series_tmp();
        let config = o8v_fs::FsConfig::default();

        let mut series: o8v_events::SeriesJson =
            match o8v_fs::safe_read(&series_path, self.storage.containment(), &config) {
                Ok(file) => {
                    let bytes = file.content().as_bytes();
                    match o8v_events::parse_series(bytes) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(error = ?e, "events: series.json corrupted, starting fresh");
                            o8v_events::SeriesJson::default()
                        }
                    }
                }
                // First run — series.json doesn't exist yet. Not an error.
                Err(o8v_fs::FsError::NotFound { .. }) => o8v_events::SeriesJson::default(),
                Err(e) => {
                    warn!(error = ?e, "events: failed to read series.json");
                    o8v_events::SeriesJson::default()
                }
            };

        for (diag_id, mut entry) in self.diagnostics {
            if let Some(existing) = series.diagnostics.get(&diag_id) {
                entry.first_seen = existing.first_seen;
                entry.run_count = existing.run_count + 1;
            } else {
                entry.run_count = 1;
            }
            series.diagnostics.insert(diag_id, entry);
        }

        series.run_id = self.run_id;
        series.timestamp = self.timestamp;
        series.version = env!("CARGO_PKG_VERSION").to_string();
        series.git_sha = None;
        // baseline_run_id is preserved from the existing series (set by 8v init);
        // do not overwrite it here.

        let write_result = (|| -> Result<(), String> {
            let containment = self.storage.containment();
            let bytes =
                o8v_events::serialize_series(&series).map_err(|e| format!("serialize: {e}"))?;
            o8v_fs::safe_write(&tmp_path, containment, &bytes)
                .map_err(|e| format!("write tmp: {e}"))?;
            o8v_fs::safe_rename(&tmp_path, &series_path, containment)
                .map_err(|e| format!("rename: {e}"))?;
            Ok(())
        })();
        if let Err(e) = write_result {
            warn!("events: failed to persist series.json: {e}");
            return None;
        }

        // Rotate event logs: keep at most 500 .ndjson files (oldest deleted first).
        rotate_event_logs(&self.storage.events_dir());

        Some(series)
    }

    fn compute_diagnostic_id(&mut self, diagnostic: &o8v_core::Diagnostic) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(loc_str(&diagnostic.location).as_bytes());
        if let Some(rule) = &diagnostic.rule {
            hasher.update(rule.as_bytes());
        }
        hasher.update(o8v_events::normalize_message(diagnostic.message.trim()).as_bytes());
        if let Some(content) = self.extract_span_content(diagnostic) {
            hasher.update(content.as_bytes());
        }

        let hex = format!("{:x}", hasher.finalize());
        hex[..Self::DIAGNOSTIC_ID_LEN.min(hex.len())].to_string()
    }

    fn extract_span_content(&mut self, diagnostic: &o8v_core::Diagnostic) -> Option<String> {
        let rel_path = match &diagnostic.location {
            o8v_core::Location::File(p) => p.clone(),
            _ => return None,
        };

        let abs_path = self.project_root.as_path().join(&rel_path);

        if let Some(content) = self.file_cache.get(&rel_path) {
            return extract_lines(content, &diagnostic.span);
        }

        let config = o8v_fs::FsConfig::default();
        match o8v_fs::safe_read(&abs_path, &self.project_root, &config) {
            Ok(file) => {
                let content = file.content().to_string();
                let result = extract_lines(&content, &diagnostic.span);
                self.file_cache.insert(rel_path, content);
                result
            }
            Err(e) => {
                tracing::debug!(error = ?e, "events: could not extract span content");
                None
            }
        }
    }

    fn write_event_log(
        &mut self,
        diagnostic: &o8v_core::Diagnostic,
        diag_id: &str,
    ) -> std::io::Result<()> {
        let Some(ref mut file) = self.event_file else {
            return Ok(());
        };

        let event = serde_json::json!({
            "run_id": self.run_id,
            "timestamp": self.timestamp,
            "diagnostic_id": diag_id,
            "location": loc_str(&diagnostic.location),
            "rule": diagnostic.rule.as_deref().unwrap_or(""),
            "severity": format!("{:?}", diagnostic.severity),
            "message": diagnostic.message.as_str(),
        });

        let line = format!("{event}\n");
        file.write_all(line.as_bytes())?;
        file.flush()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Open an event log file for writing. Returns None if creation fails.
///
/// This is intentionally best-effort: if the event log can't be opened,
/// we continue without it — series.json still gets written on finalize.
/// The Err arm is explicit (with a debug log) to satisfy the no-silent-failure
/// rule without using `.ok()` which is disallowed.
fn try_open_event_log(
    path: &std::path::Path,
    root: &o8v_fs::ContainmentRoot,
) -> Option<std::fs::File> {
    match o8v_fs::safe_create_file(path, root) {
        Ok(f) => Some(f),
        Err(e) => {
            tracing::debug!(error = ?e, "events: could not open event log");
            None
        }
    }
}

fn loc_str(loc: &o8v_core::Location) -> String {
    match loc {
        o8v_core::Location::File(p) => {
            // Defensive: File paths should never contain .. or be absolute.
            if p.contains("..") || std::path::Path::new(p).is_absolute() {
                warn!("suspicious path in diagnostic: {}", p);
                String::new()
            } else {
                p.clone()
            }
        }
        o8v_core::Location::Absolute(p) => {
            // Absolute paths are intentionally outside the project.
            // Log them for awareness but allow (they are not file-cacheable anyway).
            p.clone()
        }
        _ => String::new(),
    }
}

fn extract_lines(content: &str, span: &Option<o8v_core::Span>) -> Option<String> {
    let span = span.as_ref()?;
    let lines: Vec<&str> = content.lines().collect();

    let start = span.line.saturating_sub(1) as usize;
    let end = span
        .end_line
        .map(|l| l.saturating_sub(1) as usize)
        .unwrap_or(start);

    if start >= lines.len() {
        return None;
    }
    let end = end.min(lines.len() - 1);
    let extracted = lines[start..=end].join("\n");
    if extracted.is_empty() {
        None
    } else {
        Some(extracted)
    }
}

/// Rotate event logs in `events_dir`: keep at most 500 `.ndjson` files.
///
/// Files are sorted by modification time, oldest first. Any files beyond the
/// 500-file limit are deleted. Errors during deletion are logged as warnings
/// but not propagated — rotation is best-effort.
fn rotate_event_logs(events_dir: &std::path::Path) {
    const MAX_EVENT_FILES: usize = 500;

    let entries = match std::fs::read_dir(events_dir) {
        Ok(it) => it,
        Err(e) => {
            warn!(error = ?e, "events: could not read events dir for rotation");
            return;
        }
    };

    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!(error = ?e, "events: could not read dir entry during rotation");
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ndjson") {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        };
        files.push((mtime, path));
    }

    if files.len() <= MAX_EVENT_FILES {
        return;
    }

    // Sort oldest first so we can drop the front of the list.
    files.sort_unstable_by_key(|(mtime, _)| *mtime);

    let to_delete = files.len() - MAX_EVENT_FILES;
    for (_, path) in files.into_iter().take(to_delete) {
        if let Err(e) = std::fs::remove_file(&path) {
            warn!(error = ?e, path = %path.display(), "events: could not delete old event log");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_no_op_is_silent() {
        let mut w = EventWriter::no_op();
        assert!(w.inner.is_none());
        // finalize on no_op must not panic
        w.finalize(&make_empty_report());
    }

    #[test]
    #[ignore] // set_var("HOME") races with parallel tests; covered by e2e_events.rs
    #[allow(clippy::disallowed_methods)]
    fn test_open_creates_home_storage() {
        let home = tempfile::tempdir().expect("create home");
        let project = tempfile::tempdir().expect("create project");
        std::env::set_var("HOME", home.path());
        let root = o8v_fs::ContainmentRoot::new(project.path()).expect("root");

        let _w = EventWriter::open(&root).expect("open");
        assert!(home.path().join(".8v").join("events").exists());
    }

    #[test]
    fn test_extract_lines_basic() {
        let span = Some(o8v_core::Span::new(2, 1, None, None));
        assert_eq!(
            extract_lines("line1\nline2\nline3", &span),
            Some("line2".to_string())
        );
    }

    #[test]
    fn test_extract_lines_out_of_bounds() {
        let span = Some(o8v_core::Span::new(10, 1, None, None));
        assert_eq!(extract_lines("line1", &span), None);
    }

    #[test]
    fn test_extract_lines_no_span() {
        assert_eq!(extract_lines("line1\nline2", &None), None);
    }

    // Orphaned tmp cleanup is tested in o8v-workspace StorageDir tests.
    // Cannot test reliably here because set_var("HOME", ...) races with
    // other tests in the same process.

    #[test]
    #[ignore] // set_var("HOME") races with parallel tests; covered by e2e_events.rs
    #[allow(clippy::disallowed_methods)]
    fn test_finalize_twice_is_noop() {
        let home = tempfile::tempdir().expect("create home");
        let project = tempfile::tempdir().expect("create project");
        std::env::set_var("HOME", home.path());
        let root = o8v_fs::ContainmentRoot::new(project.path()).expect("root");
        let mut w = EventWriter::open(&root).expect("open");
        w.finalize(&make_empty_report());
        // Second call must not panic or corrupt state
        w.finalize(&make_empty_report());
        assert!(w.inner.is_none());
    }

    #[test]
    fn test_extract_lines_multi_line_beyond_eof() {
        // end_line beyond file length — should clamp to last line
        let span = Some(o8v_core::Span::new(2, 1, Some(100), None));
        assert_eq!(
            extract_lines("line1\nline2\nline3", &span),
            Some("line2\nline3".to_string())
        );
    }

    #[test]
    fn test_extract_lines_empty_content() {
        let span = Some(o8v_core::Span::new(1, 1, None, None));
        assert_eq!(extract_lines("", &span), None);
    }

    #[test]
    fn test_loc_str_rejects_path_traversal() {
        let loc = o8v_core::Location::File("../../etc/passwd".to_string());
        assert_eq!(loc_str(&loc), "", "path with .. must be rejected");
    }

    #[test]
    fn test_loc_str_accepts_valid_path() {
        let loc = o8v_core::Location::File("src/main.rs".to_string());
        assert_eq!(loc_str(&loc), "src/main.rs");
    }

    #[test]
    fn test_series_merge_preserves_first_seen_and_increments_run_count() {
        use std::collections::HashMap;

        let tmpdir = tempfile::tempdir().expect("create tmpdir");
        let dot8v = tmpdir.path().join(".8v");
        fs::create_dir(&dot8v).expect("create .8v");

        // Seed an existing series.json with run_count=3, first_seen=1000
        let diag_id = "deadbeef01234567".to_string();
        let existing = o8v_events::SeriesJson {
            run_id: "prev-run".to_string(),
            timestamp: 1000,
            baseline_run_id: None,
            version: String::new(),
            git_sha: None,
            diagnostics: {
                let mut m = HashMap::new();
                m.insert(
                    diag_id.clone(),
                    o8v_events::SeriesEntry {
                        file: "src/main.rs".to_string(),
                        rule: "test-rule".to_string(),
                        severity: "Error".to_string(),
                        message: "test message".to_string(),
                        line: 0,
                        tool: String::new(),
                        stack: String::new(),
                        project: String::new(),
                        first_seen: 1000,
                        run_count: 3,
                    },
                );
                m
            },
        };
        let bytes = o8v_events::serialize_series(&existing).expect("serialize");
        fs::write(dot8v.join("series.json"), &bytes).expect("write series.json");

        // Simulate a new run that encounters the same diagnostic
        let root = o8v_fs::ContainmentRoot::new(tmpdir.path()).expect("root");
        let containment = o8v_fs::ContainmentRoot::new(&dot8v).expect("containment");
        let config = o8v_fs::FsConfig::default();

        let mut series: o8v_events::SeriesJson =
            match o8v_fs::safe_read(&dot8v.join("series.json"), &containment, &config) {
                Ok(f) => {
                    let bytes = f.content().as_bytes();
                    match o8v_events::parse_series(bytes) {
                        Ok(s) => s,
                        Err(_) => {
                            tracing::debug!("series.json unreadable, starting empty");
                            o8v_events::SeriesJson::default()
                        }
                    }
                }
                Err(_) => o8v_events::SeriesJson::default(),
            };

        // Merge: new entry has different first_seen (9999) but same diag_id
        let mut new_entry = o8v_events::SeriesEntry {
            file: "src/main.rs".to_string(),
            rule: "test-rule".to_string(),
            severity: "Error".to_string(),
            message: "test message".to_string(),
            line: 0,
            tool: String::new(),
            stack: String::new(),
            project: String::new(),
            first_seen: 9999,
            run_count: 0,
        };
        if let Some(prev) = series.diagnostics.get(&diag_id) {
            new_entry.first_seen = prev.first_seen;
            new_entry.run_count = prev.run_count + 1;
        } else {
            new_entry.run_count = 1;
        }
        series.diagnostics.insert(diag_id.clone(), new_entry);

        let merged = &series.diagnostics[&diag_id];
        assert_eq!(merged.first_seen, 1000, "first_seen must be preserved");
        assert_eq!(merged.run_count, 4, "run_count must increment from 3 to 4");
        drop(root); // suppress unused warning
    }

    // ─── E2E: full lifecycle across two runs ─────────────────────────────────

    /// Full lifecycle test: simulates two `8v check` runs.
    ///
    /// Verifies:
    /// - series.json is created in ~/.8v/ on first run
    /// - diagnostic is recorded with run_count=1
    /// - second run: same diagnostic_id, first_seen preserved, run_count=2
    /// - run_id changes between runs
    /// - tmp file is absent after each finalize (clean atomic write)
    #[test]
    #[ignore] // set_var("HOME") races with parallel tests; covered by e2e_events.rs
    #[allow(clippy::disallowed_methods)]
    fn test_e2e_full_lifecycle_two_runs() {
        let home = tempfile::tempdir().expect("create home");
        let project = tempfile::tempdir().expect("create project");
        std::env::set_var("HOME", home.path());

        let root = o8v_fs::ContainmentRoot::new(project.path()).expect("root");
        let diag = o8v_core::DiagnosticBuilder::new("src/main.rs", "unused function `foo`")
            .rule("dead-code")
            .at_line(10)
            .build();

        // ── Run 1 ─────────────────────────────────────────────────────────
        let t_before_run1 = crate::util::unix_ms();
        let mut w1 = EventWriter::open(&root).expect("open run 1");
        w1.on_event(&diag, "clippy", "rust", "my-project");
        w1.finalize(&make_empty_report());

        let dot8v = home.path().join(".8v");
        let series_path = dot8v.join("series.json");
        let tmp_path = dot8v.join("series.json.tmp");

        assert!(series_path.exists(), "series.json must exist after run 1");
        assert!(
            !tmp_path.exists(),
            ".tmp must not exist after successful finalize"
        );

        let bytes1 = fs::read(&series_path).expect("read series.json run 1");
        let series1 = o8v_events::parse_series(&bytes1).expect("parse run 1");

        assert!(!series1.run_id.is_empty(), "run_id must be set");
        assert!(
            series1.timestamp >= t_before_run1,
            "timestamp must be after run start"
        );
        assert_eq!(series1.diagnostics.len(), 1, "exactly one diagnostic");

        let (diag_id, entry1) = series1.diagnostics.iter().next().expect("entry run 1");
        assert_eq!(entry1.run_count, 1, "run 1: run_count must be 1");
        assert!(entry1.first_seen >= t_before_run1, "first_seen must be set");
        assert_eq!(entry1.file, "src/main.rs");
        assert_eq!(entry1.rule, "dead-code");
        assert_eq!(entry1.severity, "Error");

        let diag_id = diag_id.clone();
        let first_seen = entry1.first_seen;
        let run_id_1 = series1.run_id.clone();

        // ── Run 2: same diagnostic ─────────────────────────────────────────
        let mut w2 = EventWriter::open(&root).expect("open run 2");
        w2.on_event(&diag, "clippy", "rust", "my-project");
        w2.finalize(&make_empty_report());

        let bytes2 = fs::read(&series_path).expect("read series.json run 2");
        let series2 = o8v_events::parse_series(&bytes2).expect("parse run 2");

        assert_eq!(
            series2.diagnostics.len(),
            1,
            "same diagnostic — still 1 entry, not 2"
        );

        let entry2 = series2
            .diagnostics
            .get(&diag_id)
            .expect("same diag_id in run 2");
        assert_eq!(entry2.run_count, 2, "run_count must be 2");
        assert_eq!(
            entry2.first_seen, first_seen,
            "first_seen must be preserved"
        );

        assert_ne!(series2.run_id, run_id_1, "each run gets a different run_id");
        assert!(
            series2.timestamp >= series1.timestamp,
            "timestamp must advance"
        );
        assert!(
            !tmp_path.exists(),
            ".tmp must not exist after run 2 finalize"
        );
    }

    fn make_empty_report() -> o8v_core::CheckReport {
        // CheckReport fields are pub(crate) — construct via the check runner in integration tests.
        // Here we use the public API: a check on a temp directory produces an empty report.
        let tmpdir = tempfile::tempdir().expect("tmpdir");
        let path = o8v_project::ProjectRoot::new(tmpdir.path()).expect("path");
        let interrupted = Box::leak(Box::new(std::sync::atomic::AtomicBool::new(false)));
        let config = o8v_core::CheckConfig {
            timeout: None,
            interrupted,
        };
        o8v_check::check(&path, &config, |_| {})
    }

    #[test]
    fn test_event_writer_no_op_is_safe() {
        let mut writer = EventWriter::no_op();
        let report = make_empty_report();

        // no_op writer should not panic when finalized
        writer.finalize(&report);
    }

    #[test]
    #[ignore] // set_var("HOME") races with parallel tests; covered by e2e_events.rs
    #[allow(clippy::disallowed_methods)]
    fn test_event_writer_open_creates_structure() {
        let home = tempfile::tempdir().expect("create home");
        let project = tempfile::tempdir().expect("create project");
        std::env::set_var("HOME", home.path());
        let root = o8v_fs::ContainmentRoot::new(project.path()).expect("root");

        let _writer = EventWriter::open(&root).expect("open");

        // Verify that events directory exists in ~/.8v/
        let events_dir = home.path().join(".8v").join("events");
        assert!(
            events_dir.exists(),
            "events directory should be created on open"
        );
    }
}
