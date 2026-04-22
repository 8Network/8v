// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! # o8v-core
//!
//! Foundation types and contracts for 8v.
//!
//! ## Architecture
//!
//! ```text
//! o8v-fs  →  o8v-core  →  o8v-stacks  →  o8v-check
//! (safe      (types)      (toolchains)    (orchestrate)
//!  I/O)         ↑              ↑
//!          o8v-process    o8v-process
//!       (run tools safely)
//! ```
//!
//! - `o8v-core` defines: Check trait, Diagnostic, Renderable, Command, events
//! - `o8v-stacks` defines: per-language tool configurations, parsers, fmt
//! - `o8v-check` orchestrates: detect → plan → run → report

pub mod project;

pub mod caller;
pub mod command;
pub mod event_bus;
pub mod events;
pub mod extensions;
pub mod mime;
pub mod process_report;
pub mod render;
pub mod stats;
pub mod symbols;
pub mod task;
pub mod timeout;

pub mod check;
pub mod diagnostic;
pub mod diagnostic_builder;
pub mod display_str;
pub mod fmt;
pub mod types;

pub use check::{
    Check, CheckConfig, CheckContext, CheckEntry, CheckEvent, CheckOutcome, CheckReport,
    CheckResult, DeltaSummary, ErrorKind,
};
pub use diagnostic::{
    sanitize, Applicability, Diagnostic, Edit, Location, ParseResult, ParseStatus, RelatedSpan,
    Severity, Span, Suggestion,
};
pub use diagnostic_builder::DiagnosticBuilder;
pub use display_str::DisplayStr;
pub use fmt::{FmtConfig, FmtEntry, FmtOutcome, FmtReport};
pub use process_report::{exit_code_number, exit_label};
pub use timeout::{parse_timeout, validate_timeout, MAX_TIMEOUT_SECS};
