// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Render types for `8v log` output surfaces.
//!
//! Three surfaces:
//! - [`SessionsTable`] — default sessions list (§4.1)
//! - [`DrillReport`]   — single-session drill-in (§4.2)
//! - [`SearchResults`] — `8v log search` results (§4.5)

pub mod drill;
pub mod helpers;
pub mod search;
pub mod sessions;

// Re-export the public API exactly as before so callers see no change.
pub use drill::{ClusterEntry, ClusterKind, DrillReport, PerCommandP95, TopCommand};
pub use helpers::{blind_spots_footer, fmt_warning, BLIND_SPOTS};
pub use search::{SearchResultRow, SearchResults};
pub use sessions::{SessionRow, SessionsTable};

use crate::render::output::Output;
use crate::render::Renderable;

/// Top-level report returned by `LogCommand::execute()`.
/// Dispatches `Renderable` to the appropriate inner type.
#[derive(Debug)]
pub enum LogReport {
    Sessions(Box<SessionsTable>),
    Drill(Box<DrillReport>),
    Search(Box<SearchResults>),
    /// Returned when `--session <id>` is supplied but no matching session exists.
    /// The dispatch layer converts this into exit code 1 (user error).
    Empty,
}

impl Renderable for LogReport {
    fn render_plain(&self) -> Output {
        match self {
            LogReport::Sessions(t) => t.render_plain(),
            LogReport::Drill(d) => d.render_plain(),
            LogReport::Search(s) => s.render_plain(),
            LogReport::Empty => Output::new("no session found\n".to_string()),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            LogReport::Sessions(t) => t.render_json(),
            LogReport::Drill(d) => d.render_json(),
            LogReport::Search(s) => s.render_json(),
            LogReport::Empty => Output::new("{\"error\":\"no session found\"}\n".to_string()),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;
    use crate::types::TimestampMs;
    use crate::types::Warning;

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
            has_hook_events: false,
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
            has_hook_events: false,
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
            has_hook_events: false,
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
            has_hook_events: false,
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
            has_hook_events: false,
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
            has_hook_events: false,
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
            retry_window_ms: 30_000,
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
            has_hook_events: false,
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
            has_hook_events: false,
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
        assert_eq!(helpers::fmt_bytes(500), "500B");
        assert_eq!(helpers::fmt_bytes(1_500), "1.5KB");
        assert_eq!(helpers::fmt_bytes(3_200_000), "3.2MB");
    }

    #[test]
    fn fmt_tokens_units() {
        assert_eq!(helpers::fmt_tokens(500), "500");
        assert_eq!(helpers::fmt_tokens(8_500), "8.5K");
    }

    #[test]
    fn fmt_duration_ms_units() {
        assert_eq!(helpers::fmt_duration_ms(50), "50ms");
        assert_eq!(helpers::fmt_duration_ms(1_500), "1.5s");
        assert_eq!(helpers::fmt_duration_ms(90_000), "1m");
        assert_eq!(helpers::fmt_duration_ms(3_660_000), "1h1m");
    }

    #[test]
    fn fmt_timestamp_known_date() {
        // 2026-04-17 14:12:27 UTC = 1776435147 seconds = 1776435147000 ms
        let result = helpers::fmt_timestamp(TimestampMs::from_millis(1_776_435_147_000));
        assert_eq!(result, "2026-04-17 14:12");
    }

    #[test]
    fn fmt_timestamp_negative_renders_sentinel() {
        // POC bug C3: a negative timestamp walked the naive year-loop forward
        // and produced garbage `1970-xx-xx` strings. TimestampMs + guard now
        // surface the condition explicitly.
        assert_eq!(
            helpers::fmt_timestamp(TimestampMs::from_millis(-1)),
            "(invalid timestamp)"
        );
    }

    // A8: drill header must show HH:MM-HH:MM (duration) instead of start-only timestamp
    #[test]
    fn drill_plain_header_shows_time_range() {
        // 2026-04-17 14:32:00 UTC = 1776436320 secs = 1776436320000 ms
        // 2026-04-17 14:47:00 UTC = 15m later = 1776437220000 ms
        let mut drill = make_drill();
        drill.started_ms = Some(TimestampMs::from_millis(1_776_436_320_000)); // 14:32 UTC
        drill.ended_ms = Some(TimestampMs::from_millis(1_776_437_220_000)); // 14:47 UTC
        let out = drill.render_plain();
        let s = out.as_str();
        // Must contain time range format HH:MM-HH:MM
        assert!(
            s.contains("14:32-14:47"),
            "header must contain time range 14:32-14:47, got: {}",
            s.lines().next().unwrap_or("")
        );
        assert!(
            s.contains("(15m)"),
            "header must contain duration (15m), got: {}",
            s.lines().next().unwrap_or("")
        );
    }

    // retries section must use retry_window_ms, not a hardcoded value
    #[test]
    fn drill_plain_retry_shows_window_from_field() {
        let mut drill = make_drill();
        drill.retry_window_ms = 45_000;
        drill.clusters = vec![ClusterEntry {
            kind: ClusterKind::Retry,
            command: "write".to_string(),
            argv_shape: "handler.rs --find <str>".to_string(),
            count: 2,
            path_hint: None,
        }];
        let out = drill.render_plain();
        let s = out.as_str();
        assert!(
            s.contains("in 45s"),
            "retry window must use retry_window_ms (45s), got: {}",
            s
        );
    }

    // A5: search results must include BLIND_SPOTS footer
    #[test]
    fn search_results_plain_has_blind_spots() {
        let results = SearchResults {
            query: "foo".to_string(),
            rows: vec![],
            session_count: 0,
            total_matches: 0,
            has_hook_events: false,
        };
        let out = results.render_plain();
        assert!(
            out.as_str().contains(BLIND_SPOTS),
            "search results must contain BLIND_SPOTS footer"
        );
    }
}
