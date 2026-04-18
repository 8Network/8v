// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

pub(crate) mod check_human;
pub(crate) mod check_json;
pub(crate) mod check_plain;
pub(crate) mod fmt_human;
pub(crate) mod fmt_json;
pub(crate) mod fmt_plain;
pub mod output;
// Streaming renderers — per-event output for check commands.
pub mod stream_human;
pub mod stream_json;
pub mod stream_plain;
// Report types — structured data for each command.
pub mod build_report;
pub mod hooks_report;
pub mod log_report;
pub mod ls_report;
pub mod read_report;
pub mod run_report;
pub mod search_report;
pub mod stats_report;
pub mod stats_view;
pub mod test_report;
pub mod upgrade_report;
pub mod write_report;

// Domain types — each in its own file.
pub mod config;
pub mod impls;
pub mod renderable;
pub mod sanitize;
pub mod summary;

// Re-exports — external crates and call sites import from here.
pub use config::RenderConfig;
pub use output::Output;
pub use renderable::{render, Audience, Renderable};
pub use sanitize::sanitize_for_display;
pub use summary::Summary;

pub use crate::DisplayStr;
