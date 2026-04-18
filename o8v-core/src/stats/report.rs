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
}

impl StatsReport {
    /// True when there are no rows (used to decide exit code 2).
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
