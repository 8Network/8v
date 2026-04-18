// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Cross-session failure aggregation result — a command+shape pair with repeated failures.

use serde::Serialize;

/// A (command, argv_shape) pair that accumulated repeated failures.
/// `top_path` / `top_path_count` identify the most-frequent first path-like
/// argv token among the failures, helping pinpoint the impacted target.
#[derive(Debug, Clone, Serialize)]
pub struct FailureHotspot {
    pub command: String,
    pub argv_shape: String,
    pub count: u64,
    /// Most frequent first path-like argv token among failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_path: Option<String>,
    /// How many times `top_path` appeared.
    pub top_path_count: u64,
}
