// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `8v stats` — analytical aggregates over `~/.8v/events.ndjson`.
//!
//! Shares the single-pass aggregator with `8v log`. Projects three views:
//! default per-command table, drill by argv_shape for one command,
//! and `--compare agent` grouped by `agent_info.name`.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::events::Event;
use o8v_core::render::stats_view::{LabelKey, ReportKind, StatsView};
use o8v_core::stats::{DurationStats, FailureHotspot, StatsReport, StatsRow};
use o8v_core::types::{SessionId, Warning, WarningSink};

use crate::aggregator::{
    aggregate_events, compute_failure_hotspots, ArgvNormalizer, CommandRecord, SessionAggregate,
};
use crate::event_reader::read_events_lenient;
use crate::stats_histogram::Histogram;
use crate::workspace::StorageDir;

/// Valid dimensions for `--compare`.
#[derive(Clone, Debug, ValueEnum)]
pub enum CompareBy {
    /// Group results by agent name.
    Agent,
}

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Drill into one command (positional) — e.g. `8v stats write`.
    pub command: Option<String>,
    /// Drill into a full argv_shape (named flag) — e.g. `8v stats --shape 'cargo test <path>'`.
    /// Takes precedence over the positional `command` argument.
    #[arg(long)]
    pub shape: Option<String>,
    /// Time window lower bound (duration before now): `7d`, `1h`, `30m`, `0s`. Default: 7d.
    #[arg(long, default_value = "7d")]
    pub since: String,
    /// Time window upper bound (duration before now): events newer than this are excluded.
    /// Default: `0ms` (now).
    #[arg(long, default_value = "0ms")]
    pub until: String,
    /// Hard-fail on malformed NDJSON lines instead of skipping.
    #[arg(long)]
    pub strict: bool,
    /// Group by `agent` (the only dimension in v1).
    #[arg(long)]
    pub compare: Option<CompareBy>,
    /// Failure-hotspot row count (reserved — not rendered in v1 rows, see design §3.1).
    #[arg(long, default_value_t = 3)]
    pub top: usize,
    /// Minimum sample count per row; rows with `n < min_n` are hidden.
    #[arg(long = "min-n", default_value_t = 1)]
    pub min_n: u64,
    /// Retry-cluster window in milliseconds (default: 30_000).
    #[arg(long = "retry-window", default_value_t = 30_000)]
    pub retry_window: u64,
    /// Filter output to a single session by its ID (ses_<ULID>).
    #[arg(long, value_parser = |s: &str| SessionId::try_from_raw(s))]
    pub session: Option<SessionId>,
    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
}

pub struct StatsCommand {
    pub args: Args,
}

impl Command for StatsCommand {
    type Report = StatsView;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        let storage = ctx.extensions.get::<StorageDir>().ok_or_else(|| {
            CommandError::Execution(
                "storage unavailable: run 8v from within a project directory".into(),
            )
        })?;

        let mut sink = WarningSink::new();
        let events = read_events_lenient(storage, self.args.strict, &mut sink)
            .map_err(|e| CommandError::Execution(e.to_string()))?;

        // ── Session filter (takes precedence over time-window) ────────────────
        // When --session is set, retain only events belonging to that session and
        // skip the time-window filter entirely. If --since/--until are non-default,
        // warn the user that those flags are ignored.
        let session_filtered: Option<(Vec<&Event>, SessionId)> =
            if let Some(ref session_id) = self.args.session {
                let since_non_default = self.args.since != "7d";
                let until_non_default = self.args.until != "0ms";
                if since_non_default {
                    eprintln!("warning: --since ignored (session filter takes precedence)");
                    sink.push(Warning::FlagIgnoredForSession {
                        flag: "--since".to_string(),
                    });
                }
                if until_non_default {
                    eprintln!("warning: --until ignored (session filter takes precedence)");
                    sink.push(Warning::FlagIgnoredForSession {
                        flag: "--until".to_string(),
                    });
                }
                let retained: Vec<&Event> = events
                    .iter()
                    .filter(|ev| match ev {
                        Event::CommandStarted(s) => s.session_id.as_str() == session_id.as_str(),
                        Event::CommandCompleted(c) => c.session_id.as_str() == session_id.as_str(),
                        Event::Unknown { .. } => false,
                    })
                    .collect();
                Some((retained, session_id.clone()))
            } else {
                None
            };

        // ── Time-window filter (only when no session filter) ──────────────────
        let windowed_owned: Vec<Event>;
        let (candidate_events, session_id_for_report): (Vec<&Event>, Option<SessionId>) =
            if let Some((session_events, sid)) = session_filtered {
                (session_events, Some(sid))
            } else {
                let since_ms =
                    parse_duration_ms(&self.args.since).map_err(CommandError::Execution)?;
                let until_ms =
                    parse_duration_ms(&self.args.until).map_err(CommandError::Execution)?;
                let now_ms = now_ms();
                let window_start_ms = now_ms.saturating_sub(since_ms);
                let window_end_ms = now_ms.saturating_sub(until_ms);
                if window_start_ms > window_end_ms {
                    return Ok(StatsView {
                        report: StatsReport {
                            rows: Vec::new(),
                            warnings: sink.into_inner(),
                            failure_hotspots: Vec::new(),
                            filtered_empty: true,
                            session_id: None,
                        },
                        kind: ReportKind::Table,
                        label_key: LabelKey::Command,
                        shape: None,
                    });
                }
                windowed_owned = events
                    .iter()
                    .filter(|ev| {
                        let ts = event_timestamp_ms(ev);
                        ts >= window_start_ms && ts <= window_end_ms
                    })
                    .cloned()
                    .collect();
                (windowed_owned.iter().collect::<Vec<&Event>>(), None)
            };

        // Aggregate over events in window. Clone filtered set to feed aggregator.
        // Exclude events belonging to the currently-executing stats command so its
        // own CommandStarted (with no matching CommandCompleted yet) doesn't trigger
        // an orphan warning.
        let current_run_id = ctx
            .extensions
            .get::<crate::dispatch::CurrentRunId>()
            .map(|r| r.0.clone());
        let filtered: Vec<Event> = candidate_events
            .into_iter()
            .filter(|ev| match (ev, &current_run_id) {
                (Event::CommandStarted(s), Some(rid)) => &s.run_id != rid,
                (Event::CommandCompleted(c), Some(rid)) => &c.run_id != rid,
                _ => true,
            })
            .cloned()
            .collect();
        let mut normalizer = ArgvNormalizer::new();
        let sessions = aggregate_events(
            &filtered,
            self.args.retry_window,
            &mut normalizer,
            &mut sink,
        );

        let failure_hotspots = compute_failure_hotspots(&sessions);
        let mode = resolve_mode(&self.args);
        let mut view = build_report(
            &sessions,
            mode,
            self.args.min_n,
            sink.into_inner(),
            failure_hotspots,
        );

        // Set session_id on the report when session filter was active.
        view.report.session_id = session_id_for_report.map(|id| id.as_str().to_string());

        // Exit 2 when the window produced zero results but there is history that
        // could have matched. Two triggers:
        // 1. Explicit filter: user passed non-default --since/--until or --session.
        // 2. Default window: user_event_count > 0 means events exist (possibly
        //    older than 7 days) — the window filtered them out, so exit 2 is
        //    the correct signal that results exist but outside the window.
        let has_explicit_filter =
            self.args.since != "7d" || self.args.until != "0ms" || self.args.session.is_some();
        let user_event_count = events
            .iter()
            .filter(|ev| match (ev, &current_run_id) {
                (Event::CommandStarted(s), Some(rid)) => &s.run_id != rid,
                (Event::CommandCompleted(c), Some(rid)) => &c.run_id != rid,
                _ => true,
            })
            .count();
        view.report.filtered_empty =
            filtered.is_empty() && (has_explicit_filter || user_event_count > 0);
        Ok(view)
    }
}

enum Mode<'a> {
    Default,
    Drill(&'a str),
    ByAgent,
}

fn resolve_mode(args: &Args) -> Mode<'_> {
    if matches!(args.compare, Some(CompareBy::Agent)) {
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

fn build_report(
    sessions: &[SessionAggregate],
    mode: Mode<'_>,
    min_n: u64,
    warnings: Vec<Warning>,
    failure_hotspots: Vec<FailureHotspot>,
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
            }
        }
    }
}

fn apply_min_n(rows: Vec<StatsRow>, min_n: u64) -> Vec<StatsRow> {
    rows.into_iter().filter(|r| r.n >= min_n).collect()
}

// ─── Bucket helpers ─────────────────────────────────────────────────────────

#[derive(Default)]
struct Bucket {
    n: u64,
    ok: u64,
    complete: u64,
    out_bytes_sum: u128,
    histogram: Histogram,
    retry_cluster_count: u64,
}

impl Bucket {
    fn ingest(&mut self, rec: &CommandRecord) {
        self.n += 1;
        if let Some(c) = rec.completed.as_ref() {
            self.complete += 1;
            if c.success {
                self.ok += 1;
            }
            self.out_bytes_sum += c.output_bytes as u128;
            self.histogram.record(c.duration_ms);
        }
    }

    fn to_row(&self, label: String) -> StatsRow {
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
        StatsRow {
            label,
            n: self.n,
            duration_ms,
            ok_rate,
            output_bytes_per_call_mean,
            retry_cluster_count: self.retry_cluster_count,
        }
    }
}

fn rows_by_command(sessions: &[SessionAggregate]) -> Vec<StatsRow> {
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

fn rows_by_argv_shape(sessions: &[SessionAggregate], command: &str) -> Vec<StatsRow> {
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

fn rows_by_agent(sessions: &[SessionAggregate]) -> Vec<StatsRow> {
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

// ─── Duration parsing ───────────────────────────────────────────────────────

fn parse_duration_ms(s: &str) -> Result<u64, String> {
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .expect("system clock is before Unix epoch")
}

fn event_timestamp_ms(ev: &Event) -> u64 {
    match ev {
        Event::CommandStarted(s) => s.timestamp_ms.as_millis().max(0) as u64,
        Event::CommandCompleted(c) => c.timestamp_ms.as_millis().max(0) as u64,
        Event::Unknown { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_7d() {
        assert_eq!(parse_duration_ms("7d").unwrap(), 7 * 86_400_000);
    }

    #[test]
    fn parse_duration_zero_seconds() {
        assert_eq!(parse_duration_ms("0s").unwrap(), 0);
    }

    #[test]
    fn parse_duration_bare_number_means_seconds() {
        assert_eq!(parse_duration_ms("30").unwrap(), 30_000);
    }

    #[test]
    fn parse_duration_rejects_unknown_unit() {
        assert!(parse_duration_ms("1y").is_err());
    }

    #[test]
    fn parse_duration_rejects_empty() {
        assert!(parse_duration_ms("").is_err());
    }

    /// Regression: aggregate_events produces OrphanStarted for any CommandStarted
    /// without a matching CommandCompleted. When stats runs, its own CommandStarted
    /// is already in the event log but CommandCompleted hasn't been written yet.
    /// Filtering events by current run_id before aggregation must suppress this warning.
    #[test]
    fn orphan_warning_suppressed_when_current_run_id_excluded() {
        use crate::aggregator::{aggregate_events, ArgvNormalizer};
        use o8v_core::caller::Caller;
        use o8v_core::events::{CommandStarted, Event};
        use o8v_core::types::{Warning, WarningSink};

        let current_run_id = "current-run-000".to_string();

        // Simulate the log: only a CommandStarted for the current invocation, no CommandCompleted yet.
        let started =
            CommandStarted::new(current_run_id.clone(), Caller::Cli, "stats", vec![], None);
        let events = vec![Event::CommandStarted(started)];

        // Without filtering: must see an OrphanStarted warning (proves the bug exists pre-fix).
        let mut sink_unfiltered = WarningSink::new();
        let mut normalizer = ArgvNormalizer::new();
        aggregate_events(&events, 30_000, &mut normalizer, &mut sink_unfiltered);
        let warnings_unfiltered = sink_unfiltered.into_inner();
        let has_orphan = warnings_unfiltered
            .iter()
            .any(|w| matches!(w, Warning::OrphanStarted { run_id } if run_id == &current_run_id));
        assert!(
            has_orphan,
            "pre-fix: aggregate_events must emit OrphanStarted for an open CommandStarted"
        );

        // With filtering: exclude the current run_id — must see no OrphanStarted.
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|ev| match ev {
                Event::CommandStarted(s) => s.run_id != current_run_id,
                Event::CommandCompleted(c) => c.run_id != current_run_id,
                Event::Unknown { .. } => true,
            })
            .collect();
        let mut sink_filtered = WarningSink::new();
        let mut normalizer2 = ArgvNormalizer::new();
        aggregate_events(&filtered, 30_000, &mut normalizer2, &mut sink_filtered);
        let warnings_filtered = sink_filtered.into_inner();
        let still_has_orphan = warnings_filtered
            .iter()
            .any(|w| matches!(w, Warning::OrphanStarted { run_id } if run_id == &current_run_id));
        assert!(
            !still_has_orphan,
            "post-fix: no OrphanStarted warning should appear after filtering current run_id"
        );
    }

    #[test]
    fn apply_min_n_filters_below_threshold() {
        let rows = vec![
            StatsRow {
                label: "a".into(),
                n: 2,
                duration_ms: None,
                ok_rate: None,
                output_bytes_per_call_mean: None,
                retry_cluster_count: 0,
            },
            StatsRow {
                label: "b".into(),
                n: 10,
                duration_ms: None,
                ok_rate: None,
                output_bytes_per_call_mean: None,
                retry_cluster_count: 0,
            },
        ];
        let filtered = apply_min_n(rows, 5);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "b");
    }
}
