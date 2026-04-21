// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Structured benchmark report — JSON builder.
//!
//! Pure function: `ExperimentResult → ReportJson`. No IO.
//! See docs/design/structured-benchmark-report.md.

mod builder;
mod markdown;
#[cfg(test)]
mod tests;
pub mod types;

pub use builder::build_report;
pub use markdown::render_markdown;
pub use types::{
    ConditionReport, Confidence, DeltaReport, GateCount, LandmineReport, ReportJson, RunRecord,
    StatBlock, TaskInfo, TokenBreakdown, VerificationSummary,
};
