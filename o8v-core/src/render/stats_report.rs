// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Column-formatting helpers for stats tables.
//! Presentation concerns (ReportKind, LabelKey, Renderable) live in `render::stats_view`.

use super::log_report::fmt_warning;
use crate::stats::{FailureHotspot, StatsRow};
use crate::types::Warning;

// ── column formatting helpers ────────────────────────────────────────────────

fn fmt_ms(v: Option<u64>) -> String {
    match v {
        Some(ms) => format!("{ms}"),
        None => "-".to_string(),
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    match v {
        Some(p) => format!("{:.1}%", p * 100.0),
        None => "-".to_string(),
    }
}

fn fmt_bytes(v: Option<f64>) -> String {
    match v {
        Some(b) if b >= 1_048_576.0 => format!("{:.1}M", b / 1_048_576.0),
        Some(b) if b >= 1_024.0 => format!("{:.1}K", b / 1_024.0),
        Some(b) => format!("{:.0}B", b),
        None => "-".to_string(),
    }
}

pub fn render_table(header_label: &str, rows: &[StatsRow]) -> String {
    // Column widths — at least as wide as the header.
    let label_w = rows
        .iter()
        .map(|r| r.label.len())
        .max()
        .unwrap_or(0)
        .max(header_label.len());

    let mut out = String::new();

    // Header
    // plain headers are display shorthand; JSON field names are the contract.
    out.push_str(&format!(
        "{:<label_w$}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}  {:>7}\n",
        header_label,
        "n",
        "p50",
        "p95",
        "p99",
        "ok%",
        "out/call",
        "retries",
        label_w = label_w,
    ));
    // Separator
    out.push_str(&format!(
        "{}\n",
        "-".repeat(label_w + 2 + 6 + 2 + 6 + 2 + 6 + 2 + 6 + 2 + 6 + 2 + 8 + 2 + 7)
    ));

    for row in rows {
        let (p50, p95, p99) = match &row.duration_ms {
            Some(d) => (Some(d.p50), Some(d.p95), Some(d.p99)),
            None => (None, None, None),
        };
        out.push_str(&format!(
            "{:<label_w$}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>8}  {:>7}\n",
            row.label,
            row.n,
            fmt_ms(p50),
            fmt_ms(p95),
            fmt_ms(p99),
            fmt_pct(row.ok_rate),
            fmt_bytes(row.output_bytes_per_call_mean),
            row.retry_cluster_count,
            label_w = label_w,
        ));
    }

    out
}

pub fn render_warnings(warnings: &[Warning]) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut out = String::from("\nwarnings:\n");
    for w in warnings {
        out.push_str(&format!("  warning: {}\n", fmt_warning(w)));
    }
    out
}

pub fn render_failure_hotspots(hotspots: &[FailureHotspot]) -> String {
    if hotspots.is_empty() {
        return String::new();
    }
    let mut out = String::from("\nfailure hotspots:\n");
    for h in hotspots {
        let path_info = match (&h.top_path, h.top_path_count) {
            (Some(p), c) if c > 0 => format!(" top={p} ({c}x)"),
            _ => String::new(),
        };
        out.push_str(&format!(
            "  {} {} count={}{}\n",
            h.command, h.argv_shape, h.count, path_info
        ));
    }
    out
}
