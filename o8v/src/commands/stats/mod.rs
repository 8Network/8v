// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `8v stats` — analytical aggregates over `~/.8v/events.ndjson`.
//!
//! Shares the single-pass aggregator with `8v log`. Projects three views:
//! default per-command table, drill by argv_shape for one command,
//! and `--compare agent` grouped by `agent_info.name`.

mod buckets;
mod report;
#[cfg(test)]
mod tests;

use clap::ValueEnum;
use o8v_core::caller::Caller;
use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::events::Event;
use o8v_core::render::stats_view::{LabelKey, ReportKind, StatsView};
use o8v_core::stats::StatsReport;
use o8v_core::types::{SessionId, WarningSink};

use crate::aggregator::{aggregate_events, compute_failure_hotspots, ArgvNormalizer};
use crate::event_reader::read_events_lenient;
use crate::workspace::StorageDir;

use buckets::{event_timestamp_ms, now_ms, parse_duration_ms};
use report::{build_report, resolve_mode};

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
                    sink.push(o8v_core::types::Warning::FlagIgnoredForSession {
                        flag: "--since".to_string(),
                    });
                }
                if until_non_default {
                    eprintln!("warning: --until ignored (session filter takes precedence)");
                    sink.push(o8v_core::types::Warning::FlagIgnoredForSession {
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
                let now = now_ms();
                let window_start_ms = now.saturating_sub(since_ms);
                let window_end_ms = now.saturating_sub(until_ms);
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
                        has_hook_events: false,
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
        let has_hook_events = filtered
            .iter()
            .any(|ev| matches!(ev, Event::CommandStarted(s) if s.caller == Caller::Hook));
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
            has_hook_events,
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
