// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Mode resolution and report assembly.

use o8v_core::render::stats_view::{LabelKey, ReportKind, StatsView};
use o8v_core::stats::{FailureHotspot, StatsReport, StatsRow};
use o8v_core::types::Warning;

use crate::aggregator::SessionAggregate;

use super::buckets::{rows_by_agent, rows_by_argv_shape, rows_by_command};
use super::Args;

pub(super) enum Mode<'a> {
    Default,
    Drill(&'a str),
    ByAgent,
}

pub(super) fn resolve_mode(args: &Args) -> Mode<'_> {
    if matches!(args.compare, Some(super::CompareBy::Agent)) {
        Mode::ByAgent
    } else if let Some(shape) = args.shape.as_deref() {
        // --shape takes precedence over positional command arg.
        Mode::Drill(shape)
    } else if let Some(cmd) = args.command.as_deref() {
        Mode::Drill(cmd)
    } else {
        Mode::Default
    }
}

pub(super) fn build_report(
    sessions: &[SessionAggregate],
    mode: Mode<'_>,
    min_n: u64,
    warnings: Vec<Warning>,
    failure_hotspots: Vec<FailureHotspot>,
    has_hook_events: bool,
) -> StatsView {
    match mode {
        Mode::Default => {
            let rows = rows_by_command(sessions);
            let rows = apply_min_n(rows, min_n);
            StatsView {
                report: StatsReport {
                    rows,
                    warnings,
                    failure_hotspots,
                    filtered_empty: false,
                    session_id: None,
                },
                kind: ReportKind::Table,
                label_key: LabelKey::Command,
                shape: None,
                has_hook_events,
            }
        }
        Mode::Drill(cmd) => {
            let rows = rows_by_argv_shape(sessions, cmd);
            let rows = apply_min_n(rows, min_n);
            StatsView {
                report: StatsReport {
                    rows,
                    warnings,
                    failure_hotspots,
                    filtered_empty: false,
                    session_id: None,
                },
                kind: ReportKind::Drill,
                label_key: LabelKey::ArgvShape,
                shape: Some(cmd.to_string()),
                has_hook_events,
            }
        }
        Mode::ByAgent => {
            let rows = rows_by_agent(sessions);
            let rows = apply_min_n(rows, min_n);
            StatsView {
                report: StatsReport {
                    rows,
                    warnings,
                    failure_hotspots,
                    filtered_empty: false,
                    session_id: None,
                },
                kind: ReportKind::ByAgent,
                label_key: LabelKey::Agent,
                shape: None,
                has_hook_events,
            }
        }
    }
}

pub(super) fn apply_min_n(rows: Vec<StatsRow>, min_n: u64) -> Vec<StatsRow> {
    rows.into_iter().filter(|r| r.n >= min_n).collect()
}
