// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Bucket aggregation helpers — one bucket per group key, yielding `StatsRow`s.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use o8v_core::events::Event;
use o8v_core::stats::{DurationStats, StatsRow};

use crate::aggregator::{CommandRecord, SessionAggregate};
use crate::stats_histogram::Histogram;

// ─── Bucket ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub(super) struct Bucket {
    pub n: u64,
    pub ok: u64,
    pub complete: u64,
    pub out_bytes_sum: u128,
    pub duration_ms_sum: u128,
    pub histogram: Histogram,
    pub retry_cluster_count: u64,
}

impl Bucket {
    pub fn ingest(&mut self, rec: &CommandRecord) {
        self.n += 1;
        if let Some(c) = rec.completed.as_ref() {
            self.complete += 1;
            if c.success {
                self.ok += 1;
            }
            self.out_bytes_sum += c.output_bytes as u128;
            self.duration_ms_sum += c.duration_ms as u128;
            self.histogram.record(c.duration_ms);
        }
    }

    pub fn to_row(&self, label: String) -> StatsRow {
        let p50 = self.histogram.percentile(0.50);
        let p95 = self.histogram.percentile(0.95);
        let p99 = self.histogram.percentile(0.99);
        let duration_ms = match (p50, p95, p99) {
            (Some(p50), Some(p95), Some(p99)) => Some(DurationStats { p50, p95, p99 }),
            _ => None,
        };
        let ok_rate = if self.complete > 0 {
            Some(self.ok as f64 / self.complete as f64)
        } else {
            None
        };
        let output_bytes_per_call_mean = if self.complete > 0 {
            Some(self.out_bytes_sum as f64 / self.complete as f64)
        } else {
            None
        };
        let mean_ms = if self.complete > 0 {
            Some(self.duration_ms_sum as f64 / self.complete as f64)
        } else {
            None
        };
        StatsRow {
            label,
            n: self.n,
            duration_ms,
            mean_ms,
            ok_rate,
            output_bytes_per_call_mean,
            retry_cluster_count: self.retry_cluster_count,
        }
    }
}

// ─── Row builders ────────────────────────────────────────────────────────────

pub(super) fn rows_by_command(sessions: &[SessionAggregate]) -> Vec<StatsRow> {
    let mut by_cmd: HashMap<String, Bucket> = HashMap::new();
    for s in sessions {
        for rec in &s.commands {
            let bucket = by_cmd.entry(rec.started.command.clone()).or_default();
            bucket.ingest(rec);
        }
        for cluster in &s.retry_clusters {
            if let Some(b) = by_cmd.get_mut(&cluster.command) {
                b.retry_cluster_count += 1;
            }
        }
    }
    let mut rows: Vec<StatsRow> = by_cmd
        .into_iter()
        .map(|(label, b)| b.to_row(label))
        .collect();
    rows.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.label.cmp(&b.label)));
    rows
}

pub(super) fn rows_by_argv_shape(sessions: &[SessionAggregate], command: &str) -> Vec<StatsRow> {
    let mut by_shape: HashMap<String, Bucket> = HashMap::new();
    for s in sessions {
        for rec in &s.commands {
            if rec.started.command != command {
                continue;
            }
            let bucket = by_shape.entry(rec.argv_shape.clone()).or_default();
            bucket.ingest(rec);
        }
        for cluster in &s.retry_clusters {
            if cluster.command == command {
                if let Some(b) = by_shape.get_mut(&cluster.argv_shape) {
                    b.retry_cluster_count += 1;
                }
            }
        }
    }
    // Roll shapes with n=1 into an "other" row.
    let mut rolled: HashMap<String, Bucket> = HashMap::new();
    let mut other = Bucket::default();
    let mut other_nonempty = false;
    for (shape, bucket) in by_shape {
        if bucket.n == 1 {
            other.n += bucket.n;
            other.ok += bucket.ok;
            other.complete += bucket.complete;
            other.out_bytes_sum += bucket.out_bytes_sum;
            // Histogram merging across single samples is lossy; for a 1-sample
            // bucket we skip — the "other" row's percentiles stay None unless
            // enough 1-sample shapes accumulate. This is the correct behavior
            // for the design's n<5 → None contract.
            other.retry_cluster_count += bucket.retry_cluster_count;
            other_nonempty = true;
        } else {
            rolled.insert(shape, bucket);
        }
    }
    let mut rows: Vec<StatsRow> = rolled
        .into_iter()
        .map(|(label, b)| b.to_row(label))
        .collect();
    if other_nonempty {
        rows.push(other.to_row("other".to_string()));
    }
    rows.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.label.cmp(&b.label)));
    rows
}

pub(super) fn rows_by_agent(sessions: &[SessionAggregate]) -> Vec<StatsRow> {
    let mut by_agent: HashMap<String, Bucket> = HashMap::new();
    for s in sessions {
        for rec in &s.commands {
            let label = rec
                .started
                .agent_info
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "(no agent / CLI)".to_string());
            let bucket = by_agent.entry(label).or_default();
            bucket.ingest(rec);
        }
    }
    let mut rows: Vec<StatsRow> = by_agent
        .into_iter()
        .map(|(label, b)| b.to_row(label))
        .collect();
    rows.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.label.cmp(&b.label)));
    rows
}

// ─── Duration parsing ────────────────────────────────────────────────────────

pub(super) fn parse_duration_ms(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("--since: empty duration".to_string());
    }
    let (num_part, suffix) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    if num_part.is_empty() {
        return Err(format!("--since: missing digits in '{s}'"));
    }
    let n: u64 = num_part
        .parse()
        .map_err(|e| format!("--since: '{num_part}' not a number: {e}"))?;
    let mult: u64 = match suffix {
        "" | "s" => 1_000,
        "ms" => 1,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        other => return Err(format!("--since: unknown unit '{other}' (use ms|s|m|h|d)")),
    };
    Ok(n.saturating_mul(mult))
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .expect("system clock is before Unix epoch")
}

pub(super) fn event_timestamp_ms(ev: &Event) -> u64 {
    match ev {
        Event::CommandStarted(s) => s.timestamp_ms.as_millis().max(0) as u64,
        Event::CommandCompleted(c) => c.timestamp_ms.as_millis().max(0) as u64,
        Event::Unknown { .. } => 0,
    }
}
