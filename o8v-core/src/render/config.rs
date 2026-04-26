// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

/// Configuration for rendering a `CheckReport`.
#[derive(Debug)]
pub struct RenderConfig {
    /// Max lines of error detail per check. `None` = no limit.
    pub limit: Option<usize>,
    /// Show extra context (project path, timing).
    pub verbose: bool,
    /// Whether the output target supports color.
    pub color: bool,
    /// Page number (1-based). Default 1 (first page).
    pub page: usize,
    /// When true, render extracted errors above raw stderr on build failure.
    pub errors_first: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            limit: Some(10),
            verbose: false,
            color: false,
            page: 1,
            errors_first: true,
        }
    }
}
