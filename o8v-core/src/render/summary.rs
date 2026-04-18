// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

/// Computed summary of a `CheckReport` — shared by all renderers.
pub struct Summary {
    pub passed: u32,
    pub failed: u32,
    pub errors: u32,
    pub detection_errors: usize,
    pub total_duration: std::time::Duration,
    pub success: bool,
}

impl Summary {
    /// Compute summary from a report.
    #[must_use]
    pub fn from_report(report: &crate::CheckReport) -> Self {
        let mut passed = 0u32;
        let mut failed = 0u32;
        let mut errors = 0u32;
        let mut total_duration = std::time::Duration::ZERO;

        for result in report.results() {
            for entry in result.entries() {
                total_duration += entry.duration();
                match entry.outcome() {
                    crate::CheckOutcome::Passed { .. } => passed += 1,
                    crate::CheckOutcome::Failed { .. } => failed += 1,
                    // Error + any future non_exhaustive variants count as errors.
                    _ => errors += 1,
                }
            }
        }

        let det = report.detection_errors().len();
        // Single source of truth: CheckReport::is_ok() defines success.
        let success = report.is_ok();

        Self {
            passed,
            failed,
            errors,
            detection_errors: det,
            total_duration,
            success,
        }
    }
}
