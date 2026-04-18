// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Shared aggregation layer for `8v log` and `8v stats`.
//!
//! Single-pass over `Vec<Event>` — produces `Vec<SessionAggregate>`.
//! Both log and stats read the same aggregate; they project different views.
//!
//! Design: docs/design/log-command.md §6.1–§6.3, §4.
//! Stats: docs/design/stats-command.md §4.

mod argv_normalizer;
pub use argv_normalizer::ArgvNormalizer;

mod sliding_window;
pub(crate) use sliding_window::sliding_windows;

mod cluster_detection;

mod failure_hotspot;
pub(crate) use failure_hotspot::compute_failure_hotspots;

mod session_aggregate;
pub use session_aggregate::{CommandRecord, FailureCluster, RetryCluster, SessionAggregate};

mod aggregate_events;
pub use aggregate_events::aggregate_events;
