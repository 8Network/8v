// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The domain result of the stats computation — rows, warnings, and failure hotspots.
//! Presentation concerns (kind, label_key, shape) live in `o8v_core::render::stats_view`.

use crate::types::Warning;

use super::failure_hotspot::FailureHotspot;
use super::stats_row::StatsRow;

/// Domain result returned by stats computation — pure data, no presentation.
#[derive(Debug, Clone)]
pub struct StatsReport {
    pub rows: Vec<StatsRow>,
    pub warnings: Vec<Warning>,
    pub failure_hotspots: Vec<FailureHotspot>,
    /// True when the caller applied an explicit time filter (`--since`/`--until`)
    /// that produced zero matching events. Distinct from "no history at all",
    /// which is a valid first-run state and should exit 0.
    /// Used by the dispatch layer to emit exit code 2 (empty-window signal).
    pub filtered_empty: bool,

    /// Set when `--session <id>` was used. Drives the session-scoped header in
    /// plain output and the top-level `session_id` field in JSON output.
    pub session_id: Option<String>,
}

impl StatsReport {
    /// True when there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
