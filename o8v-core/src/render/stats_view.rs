// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Presentation wrapper for a `StatsReport` — owns kind, label_key, and shape.
//! `StatsView` is what the CLI renders; `StatsReport` is domain data.

use serde::Serialize;

use crate::stats::{FailureHotspot, StatsReport, StatsRow};
use crate::types::Warning;

use super::output::Output;
use super::stats_report::{render_failure_hotspots, render_table};

// ── Enums (presentation concerns) ────────────────────────────────────────────

/// Which table layout to use.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Table,
    Drill,
    ByAgent,
}

/// What the label column represents in each row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKey {
    Command,
    ArgvShape,
    Agent,
}

// ── StatsView ─────────────────────────────────────────────────────────────────

/// Presentation wrapper that adds rendering context to a `StatsReport`.
/// Implements `Renderable`; `StatsReport` does not.
#[derive(Debug, Clone)]
pub struct StatsView {
    pub report: StatsReport,
    pub kind: ReportKind,
    pub label_key: LabelKey,
    /// Only set when `kind = Drill`.
    pub shape: Option<String>,
    /// True when the filtered event set contains at least one `caller="hook"` event.
    pub has_hook_events: bool,
}

impl StatsView {
    pub fn is_empty(&self) -> bool {
        self.report.is_empty()
    }
}

// ── JSON envelope ─────────────────────────────────────────────────────────────

/// Serializable view — single source of truth for the JSON wire contract.
///
/// Envelope shape: `{ kind, label_key, shape?, rows, warnings, failure_hotspots }`
/// - `kind ∈ {"table","drill","by_agent"}`
/// - `label_key ∈ {"command","argv_shape","agent"}`
/// - `shape` only when `kind = "drill"`
#[derive(Serialize)]
struct StatsViewJson<'a> {
    kind: &'a ReportKind,
    label_key: &'a LabelKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    rows: &'a [StatsRow],
    warnings: &'a [Warning],
    failure_hotspots: &'a [FailureHotspot],
}

// ── Renderable impl ───────────────────────────────────────────────────────────

impl super::Renderable for StatsView {
    fn render_plain(&self) -> Output {
        if self.is_empty() {
            return Output::new("no matching events\n".to_string());
        }

        let header_label = match self.kind {
            ReportKind::ByAgent => "agent",
            _ => "command",
        };

        let session_header = match &self.report.session_id {
            Some(id) => format!("session: {id}\n\n"),
            None => String::new(),
        };

        let table = match &self.shape {
            Some(shape) => format!(
                "shape: {shape}\n\n{}",
                render_table(header_label, &self.report.rows)
            ),
            None => render_table(header_label, &self.report.rows),
        };

        let text = format!(
            "{}{}{}\n{}",
            session_header,
            table,
            render_failure_hotspots(&self.report.failure_hotspots),
            super::log_report::blind_spots_footer(self.has_hook_events),
        );

        let mut stderr = String::new();
        for w in &self.report.warnings {
            stderr.push_str(&format!(
                "warning: {}\n",
                super::log_report::helpers::fmt_warning(w)
            ));
        }

        Output::new_with_stderr(text, stderr)
    }

    fn render_json(&self) -> Output {
        let view = StatsViewJson {
            kind: &self.kind,
            label_key: &self.label_key,
            shape: self.shape.as_deref(),
            session_id: self.report.session_id.as_deref(),
            rows: &self.report.rows,
            warnings: &self.report.warnings,
            failure_hotspots: &self.report.failure_hotspots,
        };
        let json = serde_json::to_string(&view).expect("StatsView serialization is infallible");
        Output::new(json)
    }
}
