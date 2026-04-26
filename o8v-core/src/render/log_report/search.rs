// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `SearchResults` — `8v log search` output surface (§4.5).

use super::helpers::{blind_spots_footer, fmt_timestamp};
use crate::render::output::Output;
use crate::render::Renderable;
use crate::types::TimestampMs;
use serde::Serialize;

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
    /// True when the filtered event set contains at least one `caller="hook"` event.
    pub has_hook_events: bool,
}

impl Renderable for SearchResults {
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

        out.push('\n');
        out.push_str(blind_spots_footer(self.has_hook_events));
        out.push('\n');

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
