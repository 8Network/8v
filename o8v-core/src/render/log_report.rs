// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Render types for `8v log` output surfaces.
//!
//! Three surfaces:
//! - [`SessionsTable`] — default sessions list (§4.1)
//! - [`DrillReport`]   — single-session drill-in (§4.2)
//! - [`SearchResults`] — `8v log search` results (§4.5)

use super::output::Output;
use crate::render::Renderable;
use crate::types::{TimestampMs, Warning};
use serde::Serialize;

/// Render a typed `Warning` into a single human-readable line.
/// Single source of truth for warning display — callers must never format
/// warnings themselves.
pub fn fmt_warning(w: &Warning) -> String {
    match w {
        Warning::CanonicalizeFailed { path, reason } => {
            format!("canonicalize_failed: {path} ({reason})")
        }
        Warning::DuplicateCompleted { run_id } => {
            format!("duplicate CommandCompleted run_id={run_id}: later dropped")
        }
        Warning::DuplicateStarted { run_id } => {
            format!("duplicate CommandStarted run_id={run_id}: first wins")
        }
        Warning::ReversedTimestamps {
            session,
            earlier,
            later,
        } => {
            format!(
                "reversed timestamps in session {}: started={} completed={}",
                session.as_str(),
                earlier.as_millis(),
                later.as_millis()
            )
        }
        Warning::EmptySessionId { at, reason } => {
            format!("empty session_id at ts={}: {reason}", at.as_millis())
        }
        Warning::FutureSince { since_ms, now_ms } => {
            format!("--since in the future: since={since_ms} now={now_ms}")
        }
        Warning::MalformedEventLine { line_no, reason } => {
            format!("line {line_no}: {reason}")
        }
        Warning::OrphanStarted { run_id } => {
            format!("orphan CommandStarted run_id={run_id}: no matching Completed")
        }
        Warning::OrphanCompleted { run_id } => {
            format!("orphan CommandCompleted run_id={run_id}: no matching Started")
        }
        Warning::NormalizerBasenameFallback { session, path } => {
            format!(
                "session {}: project_path unknown; basename fallback for {path}",
                session.as_str()
            )
        }
        Warning::PercentileOutOfRange { p } => {
            format!("percentile out of range: {p}")
        }
    }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

const BLIND_SPOTS: &str =
    "blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.";

/// Format a byte count as a human-readable string: KB / MB / B.
fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f64 / 1_000.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Format a token count: K suffix when ≥ 1000.
fn fmt_tokens(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{}", tokens)
    }
}

/// Format a duration in milliseconds as a human-readable string.
fn fmt_duration_ms(ms: u64) -> String {
    if ms >= 3_600_000 {
        format!("{}h{}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    } else if ms >= 60_000 {
        format!("{}m", ms / 60_000)
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}ms", ms)
    }
}

/// Format a unix-ms timestamp as `YYYY-MM-DD HH:MM` (UTC).
///
/// Negative timestamps (pre-1970) are not meaningful 8v events; rendering
/// them through the naive subtraction loop below produced garbage
/// `1970-xx-xx` strings in the POC. Surface the problem instead.
fn fmt_timestamp(ts: TimestampMs) -> String {
    let ms = ts.as_millis();
    if ms < 0 {
        return "(invalid timestamp)".to_string();
    }
    let secs = ms / 1000;
    let minutes_total = secs / 60;
    let hour = (minutes_total / 60) % 24;
    let minute = minutes_total % 60;
    let days = secs / 86400;

    let mut y = 1970i32;
    let mut d = days as i32;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let month_days: [i32; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i32;
    for md in &month_days {
        if d < *md {
            break;
        }
        d -= md;
        month += 1;
    }
    let day = d + 1;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, month, day, hour, minute)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ─── SessionsTable ────────────────────────────────────────────────────────────

/// One row in the sessions list.
#[derive(Debug, Serialize)]
pub struct SessionRow {
    pub session_id: String,
    /// `None` for sessions with no timestamps.
    pub started_ms: Option<TimestampMs>,
    /// Total session duration ms (`ended_ms - started_ms`). `None` when incomplete.
    pub duration_ms: Option<u64>,
    #[serde(rename = "commands")]
    pub command_count: usize,
    #[serde(rename = "fail")]
    pub fail_count: usize,
    pub output_bytes: u64,
    pub token_estimate: u64,
    /// Agent name + version string, if known.
    pub agent: Option<String>,
}

/// Sessions list (§4.1).
#[derive(Debug)]
pub struct SessionsTable {
    pub rows: Vec<SessionRow>,
    /// Total number of sessions before limiting.
    pub total_count: usize,
    pub limit: usize,
    pub warnings: Vec<Warning>,
}

impl super::Renderable for SessionsTable {
    fn render_plain(&self) -> Output {
        let mut out = String::new();

        // Header
        out.push_str(&format!(
            "  {:<12}  {:<20}  {:>5}  {:>5}  {:>5}  {:>8}  {:>7}  {}\n",
            "id", "when", "dur", "cmds", "fail", "out", "tokens", "agent"
        ));

        for row in &self.rows {
            let when = row
                .started_ms
                .map(fmt_timestamp)
                .unwrap_or_else(|| "-".to_string());
            let dur = row
                .duration_ms
                .map(fmt_duration_ms)
                .unwrap_or_else(|| "-".to_string());
            let agent = row.agent.clone().unwrap_or_else(|| "-".to_string());

            out.push_str(&format!(
                "  {:<12}  {:<20}  {:>5}  {:>5}  {:>5}  {:>8}  {:>7}  {}\n",
                row.session_id,
                when,
                dur,
                row.command_count,
                row.fail_count,
                fmt_bytes(row.output_bytes),
                fmt_tokens(row.token_estimate),
                agent,
            ));
        }

        if self.total_count > self.limit {
            out.push_str(&format!(
                "\n  (showing {} of {}, --all to see all, --limit N to change)\n",
                self.rows.len(),
                self.total_count
            ));
        }

        if !self.warnings.is_empty() {
            out.push('\n');
            for w in &self.warnings {
                out.push_str(&format!("  warning: {}\n", fmt_warning(w)));
            }
        }

        out.push('\n');
        out.push_str(BLIND_SPOTS);
        out.push('\n');

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        #[derive(Serialize)]
        struct View<'a> {
            sessions: &'a [SessionRow],
            total_count: usize,
            limit: usize,
            warnings: &'a [Warning],
        }
        let view = View {
            sessions: &self.rows,
            total_count: self.total_count,
            limit: self.limit,
            warnings: &self.warnings,
        };
        let s =
            serde_json::to_string_pretty(&view).expect("SessionsTable serialization is infallible");
        Output::new(format!("{s}\n"))
    }
}

// ─── DrillReport ─────────────────────────────────────────────────────────────

/// Per-command p95 latency entry.
#[derive(Debug)]
pub struct PerCommandP95 {
    pub command: String,
    pub p95_ms: u64,
}

/// A cluster entry (retry or failure) for the drill view.
#[derive(Debug)]
pub struct ClusterEntry {
    pub kind: ClusterKind,
    pub command: String,
    pub argv_shape: String,
    pub count: usize,
    /// For failure clusters: path context extracted from argv_shape.
    pub path_hint: Option<String>,
}

#[derive(Debug)]
pub enum ClusterKind {
    Retry,
    Failure,
}

/// Top-command entry: command name + invocation count.
#[derive(Debug)]
pub struct TopCommand {
    pub command: String,
    pub count: usize,
}

/// Single-session drill-in report (§4.2 / §4.6).
#[derive(Debug)]
pub struct DrillReport {
    pub session_id: String,
    pub caller: String,
    pub agent: Option<String>,
    pub started_ms: Option<TimestampMs>,
    pub ended_ms: Option<TimestampMs>,
    pub project_path: Option<String>,
    pub command_count: usize,
    pub ok_count: usize,
    pub fail_count: usize,
    pub incomplete_count: usize,
    /// `None` when n < 5 — render as `-`.
    pub p50_ms: Option<u64>,
    /// `None` when n < 5 — render as `-`.
    pub p95_ms: Option<u64>,
    pub output_bytes_total: u64,
    pub per_command_p95: Vec<PerCommandP95>,
    pub top_commands: Vec<TopCommand>,
    pub clusters: Vec<ClusterEntry>,
    pub warnings: Vec<Warning>,
}

impl super::Renderable for DrillReport {
    fn render_plain(&self) -> Output {
        let mut out = String::new();

        // Header line
        let when = match (self.started_ms, self.ended_ms) {
            (Some(s), Some(e)) => match e.checked_sub(s) {
                Some(dur) => format!(
                    "{}  ({})",
                    fmt_timestamp(s),
                    fmt_duration_ms(dur.as_millis())
                ),
                None => fmt_timestamp(s),
            },
            (Some(s), None) => fmt_timestamp(s),
            _ => "-".to_string(),
        };
        let agent_str = self.agent.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "session {}   {}  {}  {}\n",
            self.session_id, when, agent_str, self.caller
        ));

        if let Some(p) = &self.project_path {
            out.push_str(&format!("project {}\n", p));
        }

        out.push_str("---\n");

        // Summary stats line
        let p50 = self
            .p50_ms
            .map(|v| format!("p50 {}ms", v))
            .unwrap_or_else(|| "p50 -".to_string());
        let p95 = self
            .p95_ms
            .map(|v| format!("p95 {}ms", v))
            .unwrap_or_else(|| "p95 -".to_string());
        out.push_str(&format!(
            "  {} commands   {} ok   {} fail   {}  {}   {} out\n",
            self.command_count,
            self.ok_count,
            self.fail_count,
            p50,
            p95,
            fmt_bytes(self.output_bytes_total),
        ));

        // Per-cmd p95
        if !self.per_command_p95.is_empty() {
            out.push('\n');
            out.push_str("per-cmd p95");
            for pc in &self.per_command_p95 {
                out.push_str(&format!(
                    "    {} {}",
                    pc.command,
                    fmt_duration_ms(pc.p95_ms)
                ));
            }
            out.push('\n');
            out.push_str(
                "               (run '8v stats' for p50/p99, ok%, argv-shape breakdown)\n",
            );
        }

        // Top commands
        if !self.top_commands.is_empty() {
            out.push('\n');
            out.push_str("top commands");
            for tc in &self.top_commands {
                out.push_str(&format!("   {} {}", tc.command, tc.count));
            }
            out.push('\n');
        }

        // Clusters
        let failures: Vec<&ClusterEntry> = self
            .clusters
            .iter()
            .filter(|c| matches!(c.kind, ClusterKind::Failure))
            .collect();
        let retries: Vec<&ClusterEntry> = self
            .clusters
            .iter()
            .filter(|c| matches!(c.kind, ClusterKind::Retry))
            .collect();

        if !failures.is_empty() {
            out.push_str("failures");
            for f in &failures {
                out.push_str(&format!("       {} x{}\n", f.command, f.count));
            }
        }
        if !retries.is_empty() {
            out.push_str("retries");
            for r in &retries {
                out.push_str(&format!(
                    "        {} {}   x{} in {}s\n",
                    r.command, r.argv_shape, r.count, 90
                ));
            }
        }

        if !self.warnings.is_empty() {
            out.push('\n');
            for w in &self.warnings {
                out.push_str(&format!("warning: {}\n", fmt_warning(w)));
            }
        }

        out.push('\n');
        out.push_str(BLIND_SPOTS);
        out.push('\n');

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        // §4.6 stable JSON field names exactly.
        let per_cmd_p95: serde_json::Map<String, serde_json::Value> = self
            .per_command_p95
            .iter()
            .map(|pc| (pc.command.clone(), serde_json::Value::from(pc.p95_ms)))
            .collect();

        let top_cmds: Vec<serde_json::Value> = self
            .top_commands
            .iter()
            .map(|tc| serde_json::json!([tc.command, tc.count]))
            .collect();

        let clusters: Vec<serde_json::Value> = self
            .clusters
            .iter()
            .map(|c| {
                let kind = match c.kind {
                    ClusterKind::Retry => "retry",
                    ClusterKind::Failure => "failure",
                };
                serde_json::json!({
                    "kind": kind,
                    "command": c.command,
                    "argv_shape": c.argv_shape,
                    "count": c.count,
                    "path": c.path_hint,
                })
            })
            .collect();

        let agent_val = self.agent.as_ref().map(|a| {
            // Split "name version" if space present.
            let mut parts = a.splitn(2, ' ');
            let name = parts.next().unwrap_or(a.as_str());
            let version = parts.next().unwrap_or("");
            serde_json::json!({ "name": name, "version": version })
        });

        let json = serde_json::json!({
            "session_id": self.session_id,
            "caller": self.caller,
            "agent": agent_val,
            "started_ms": self.started_ms.map(|t| t.as_millis()),
            "ended_ms": self.ended_ms.map(|t| t.as_millis()),
            "commands": self.command_count,
            "ok": self.ok_count,
            "fail": self.fail_count,
            "incomplete": self.incomplete_count,
            "p50_ms": self.p50_ms,
            "p95_ms": self.p95_ms,
            "per_command_p95_ms": per_cmd_p95,
            "output_bytes_total": self.output_bytes_total,
            "top_commands": top_cmds,
            "clusters": clusters,
        });

        let s = match serde_json::to_string_pretty(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }
}

// ─── SearchResults ────────────────────────────────────────────────────────────

/// One row in the `8v log search` output (§4.5).
#[derive(Debug, Serialize)]
pub struct SearchResultRow {
    pub session_id: String,
    pub timestamp_ms: TimestampMs,
    pub command: String,
    pub argv_shape: String,
    pub success: Option<bool>,
}

/// Search results table (§4.5).
#[derive(Debug)]
pub struct SearchResults {
    pub query: String,
    pub rows: Vec<SearchResultRow>,
    pub session_count: usize,
    pub total_matches: usize,
}

impl super::Renderable for SearchResults {
    fn render_plain(&self) -> Output {
        let mut out = String::new();

        out.push_str(&format!(
            "{} sessions, {} matches\n\n",
            self.session_count, self.total_matches
        ));

        for row in &self.rows {
            let when = fmt_timestamp(row.timestamp_ms);
            let status = match row.success {
                Some(true) => "ok",
                Some(false) => "FAIL",
                None => "?",
            };
            out.push_str(&format!(
                "  {:<12}  {}  {:<10}  {:<40}  {}\n",
                row.session_id, when, row.command, row.argv_shape, status
            ));
        }

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        #[derive(Serialize)]
        struct View<'a> {
            query: &'a str,
            session_count: usize,
            total_matches: usize,
            results: &'a [SearchResultRow],
        }
        let view = View {
            query: &self.query,
            session_count: self.session_count,
            total_matches: self.total_matches,
            results: &self.rows,
        };
        let s =
            serde_json::to_string_pretty(&view).expect("SearchResults serialization is infallible");
        Output::new(format!("{s}\n"))
    }
}

/// Top-level report returned by `LogCommand::execute()`.
/// Dispatches `Renderable` to the appropriate inner type.
#[derive(Debug)]
pub enum LogReport {
    Sessions(Box<SessionsTable>),
    Drill(Box<DrillReport>),
    Search(Box<SearchResults>),
}

impl Renderable for LogReport {
    fn render_plain(&self) -> Output {
        match self {
            LogReport::Sessions(t) => t.render_plain(),
            LogReport::Drill(d) => d.render_plain(),
            LogReport::Search(s) => s.render_plain(),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            LogReport::Sessions(t) => t.render_json(),
            LogReport::Drill(d) => d.render_json(),
            LogReport::Search(s) => s.render_json(),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    fn make_row(id: &str, cmds: usize, fail: usize) -> SessionRow {
        SessionRow {
            session_id: id.to_string(),
            started_ms: Some(TimestampMs::from_millis(1_776_435_147_000)),
            duration_ms: Some(900_000),
            command_count: cmds,
            fail_count: fail,
            output_bytes: 34_816,
            token_estimate: 8_704,
            agent: Some("claude-code 2.1.112".to_string()),
        }
    }

    #[test]
    fn sessions_table_plain_has_blind_spots() {
        let table = SessionsTable {
            rows: vec![make_row("ses_abc123", 42, 4)],
            total_count: 1,
            limit: 10,
            warnings: vec![],
        };
        let out = table.render_plain();
        let s = out.as_str();
        assert!(
            s.contains(BLIND_SPOTS),
            "plain output must contain blind-spots footer"
        );
    }

    #[test]
    fn sessions_table_plain_has_header() {
        let table = SessionsTable {
            rows: vec![make_row("ses_abc123", 42, 4)],
            total_count: 1,
            limit: 10,
            warnings: vec![],
        };
        let out = table.render_plain();
        let s = out.as_str();
        assert!(s.contains("id"), "header must contain 'id' column");
        assert!(s.contains("cmds"), "header must contain 'cmds' column");
        assert!(s.contains("fail"), "header must contain 'fail' column");
    }

    #[test]
    fn sessions_table_plain_row_present() {
        let table = SessionsTable {
            rows: vec![make_row("ses_abc123", 42, 4)],
            total_count: 1,
            limit: 10,
            warnings: vec![],
        };
        let out = table.render_plain();
        let s = out.as_str();
        assert!(s.contains("ses_abc123"), "row session_id must appear");
        assert!(s.contains("42"), "command count must appear");
        assert!(s.contains("4"), "fail count must appear");
    }

    #[test]
    fn sessions_table_plain_pagination_footer() {
        let rows: Vec<SessionRow> = (0..5)
            .map(|i| make_row(&format!("ses_{:04}", i), 10, 0))
            .collect();
        let table = SessionsTable {
            rows,
            total_count: 87,
            limit: 5,
            warnings: vec![],
        };
        let out = table.render_plain();
        let s = out.as_str();
        assert!(
            s.contains("showing 5 of 87"),
            "pagination footer must appear; got: {s}"
        );
        assert!(s.contains("--all"), "pagination footer must mention --all");
    }

    #[test]
    fn sessions_table_json_fields() {
        let table = SessionsTable {
            rows: vec![make_row("ses_abc123", 42, 4)],
            total_count: 1,
            limit: 10,
            warnings: vec![],
        };
        let out = table.render_json();
        let s = out.as_str();
        let parsed: serde_json::Value = serde_json::from_str(s.trim()).expect("must be valid JSON");
        assert!(parsed["sessions"].is_array(), "sessions must be array");
        let row = &parsed["sessions"][0];
        assert_eq!(row["session_id"], "ses_abc123");
        assert_eq!(row["commands"], 42);
        assert_eq!(row["fail"], 4);
    }

    #[test]
    fn sessions_table_warnings_in_plain() {
        let table = SessionsTable {
            rows: vec![make_row("ses_abc123", 5, 0)],
            total_count: 1,
            limit: 10,
            warnings: vec![Warning::DuplicateStarted {
                run_id: "r1".to_string(),
            }],
        };
        let out = table.render_plain();
        assert!(
            out.as_str().contains("warning: duplicate CommandStarted"),
            "stdout: {}",
            out.as_str()
        );
    }

    fn make_drill() -> DrillReport {
        DrillReport {
            session_id: "ses_a1f3".to_string(),
            caller: "mcp".to_string(),
            agent: Some("claude-code 2.1.112".to_string()),
            started_ms: Some(TimestampMs::from_millis(1_776_435_147_000)),
            ended_ms: Some(TimestampMs::from_millis(1_776_436_047_000)),
            project_path: Some("/Users/user/proj".to_string()),
            command_count: 42,
            ok_count: 38,
            fail_count: 4,
            incomplete_count: 0,
            p50_ms: Some(5),
            p95_ms: Some(240),
            output_bytes_total: 34_816,
            per_command_p95: vec![
                PerCommandP95 {
                    command: "read".to_string(),
                    p95_ms: 18,
                },
                PerCommandP95 {
                    command: "write".to_string(),
                    p95_ms: 14,
                },
            ],
            top_commands: vec![
                TopCommand {
                    command: "read".to_string(),
                    count: 18,
                },
                TopCommand {
                    command: "write".to_string(),
                    count: 9,
                },
            ],
            clusters: vec![ClusterEntry {
                kind: ClusterKind::Failure,
                command: "write".to_string(),
                argv_shape: "write handler.rs --find <str>".to_string(),
                count: 3,
                path_hint: Some("handler.rs".to_string()),
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn drill_plain_has_session_id() {
        let drill = make_drill();
        let out = drill.render_plain();
        let s = out.as_str();
        assert!(s.contains("ses_a1f3"), "session_id must appear");
    }

    #[test]
    fn drill_plain_has_project_path() {
        let drill = make_drill();
        let out = drill.render_plain();
        let s = out.as_str();
        assert!(s.contains("/Users/user/proj"), "project path must appear");
    }

    #[test]
    fn drill_plain_has_stats_line() {
        let drill = make_drill();
        let out = drill.render_plain();
        let s = out.as_str();
        assert!(s.contains("42 commands"), "command count must appear");
        assert!(s.contains("38 ok"), "ok count must appear");
        assert!(s.contains("4 fail"), "fail count must appear");
        assert!(s.contains("p50"), "p50 must appear");
        assert!(s.contains("p95"), "p95 must appear");
    }

    #[test]
    fn drill_plain_has_blind_spots() {
        let drill = make_drill();
        let out = drill.render_plain();
        assert!(out.as_str().contains(BLIND_SPOTS));
    }

    #[test]
    fn drill_plain_dash_when_n_lt_5() {
        let mut drill = make_drill();
        drill.p50_ms = None;
        drill.p95_ms = None;
        let out = drill.render_plain();
        let s = out.as_str();
        assert!(s.contains("p50 -"), "p50 must render as '-' when None");
        assert!(s.contains("p95 -"), "p95 must render as '-' when None");
    }

    #[test]
    fn drill_json_stable_fields() {
        let drill = make_drill();
        let out = drill.render_json();
        let s = out.as_str();
        let parsed: serde_json::Value = serde_json::from_str(s.trim()).expect("must be valid JSON");
        assert_eq!(parsed["session_id"], "ses_a1f3");
        assert_eq!(parsed["caller"], "mcp");
        assert_eq!(parsed["commands"], 42);
        assert_eq!(parsed["ok"], 38);
        assert_eq!(parsed["fail"], 4);
        assert_eq!(parsed["incomplete"], 0);
        assert_eq!(parsed["p50_ms"], 5);
        assert_eq!(parsed["p95_ms"], 240);
        assert_eq!(parsed["output_bytes_total"], 34816);
        assert!(parsed["per_command_p95_ms"].is_object());
        assert!(parsed["top_commands"].is_array());
        assert!(parsed["clusters"].is_array());
    }

    #[test]
    fn drill_json_cluster_has_kind() {
        let drill = make_drill();
        let out = drill.render_json();
        let parsed: serde_json::Value =
            serde_json::from_str(out.as_str().trim()).expect("valid JSON");
        let cluster = &parsed["clusters"][0];
        assert_eq!(cluster["kind"], "failure");
        assert_eq!(cluster["command"], "write");
        assert_eq!(cluster["count"], 3);
    }

    #[test]
    fn search_results_plain_format() {
        let results = SearchResults {
            query: "foo".to_string(),
            rows: vec![SearchResultRow {
                session_id: "ses_a1f3".to_string(),
                timestamp_ms: TimestampMs::from_millis(1_776_435_147_000),
                command: "write".to_string(),
                argv_shape: "handler.rs --find <str>".to_string(),
                success: Some(false),
            }],
            session_count: 1,
            total_matches: 1,
        };
        let out = results.render_plain();
        let s = out.as_str();
        assert!(
            s.contains("1 sessions, 1 matches"),
            "summary line must appear"
        );
        assert!(s.contains("ses_a1f3"), "session_id must appear");
        assert!(s.contains("FAIL"), "FAIL status must appear");
        assert!(s.contains("write"), "command must appear");
    }

    #[test]
    fn search_results_json_fields() {
        let results = SearchResults {
            query: "foo".to_string(),
            rows: vec![SearchResultRow {
                session_id: "ses_a1f3".to_string(),
                timestamp_ms: TimestampMs::from_millis(1_776_435_147_000),
                command: "write".to_string(),
                argv_shape: "handler.rs --find <str>".to_string(),
                success: Some(false),
            }],
            session_count: 1,
            total_matches: 1,
        };
        let out = results.render_json();
        let parsed: serde_json::Value =
            serde_json::from_str(out.as_str().trim()).expect("valid JSON");
        assert_eq!(parsed["query"], "foo");
        assert_eq!(parsed["total_matches"], 1);
        assert!(parsed["results"].is_array());
        assert_eq!(parsed["results"][0]["command"], "write");
        assert_eq!(parsed["results"][0]["success"], false);
    }

    #[test]
    fn fmt_bytes_units() {
        assert_eq!(fmt_bytes(500), "500B");
        assert_eq!(fmt_bytes(1_500), "1.5KB");
        assert_eq!(fmt_bytes(3_200_000), "3.2MB");
    }

    #[test]
    fn fmt_tokens_units() {
        assert_eq!(fmt_tokens(500), "500");
        assert_eq!(fmt_tokens(8_500), "8.5K");
    }

    #[test]
    fn fmt_duration_ms_units() {
        assert_eq!(fmt_duration_ms(50), "50ms");
        assert_eq!(fmt_duration_ms(1_500), "1.5s");
        assert_eq!(fmt_duration_ms(90_000), "1m");
        assert_eq!(fmt_duration_ms(3_660_000), "1h1m");
    }

    #[test]
    fn fmt_timestamp_known_date() {
        // 2026-04-17 14:12:27 UTC = 1776435147 seconds = 1776435147000 ms
        let result = fmt_timestamp(TimestampMs::from_millis(1_776_435_147_000));
        assert_eq!(result, "2026-04-17 14:12");
    }

    #[test]
    fn fmt_timestamp_negative_renders_sentinel() {
        // POC bug C3: a negative timestamp walked the naive year-loop forward
        // and produced garbage `1970-xx-xx` strings. TimestampMs + guard now
        // surface the condition explicitly.
        assert_eq!(
            fmt_timestamp(TimestampMs::from_millis(-1)),
            "(invalid timestamp)"
        );
    }
}
