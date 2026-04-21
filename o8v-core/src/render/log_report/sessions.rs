// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `SessionsTable` — the default `8v log` sessions list (§4.1).

use super::helpers::{
    blind_spots_footer, fmt_bytes, fmt_duration_ms, fmt_timestamp, fmt_tokens, fmt_warning,
};
use crate::render::output::Output;
use crate::render::Renderable;
use crate::types::TimestampMs;
use crate::types::Warning;
use serde::Serialize;

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
    /// True when the filtered event set contains at least one `caller="hook"` event.
    pub has_hook_events: bool,
}

impl Renderable for SessionsTable {
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
        out.push_str(blind_spots_footer(self.has_hook_events));
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
