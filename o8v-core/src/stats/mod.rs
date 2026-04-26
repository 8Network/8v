// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Domain types for the `8v stats` command — statistics, measurements, and report data.

pub mod duration_stats;
pub mod failure_hotspot;
pub mod report;
pub mod stats_row;

pub use duration_stats::DurationStats;
pub use failure_hotspot::FailureHotspot;
pub use report::StatsReport;
pub use stats_row::StatsRow;
