// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `8v log` — session history and command drill-down.

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::log_report::LogReport;
use o8v_core::types::{SessionId, Warning, WarningSink};

mod drill;
mod search;
mod sessions;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Maximum number of sessions to show (default: 20).
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Show all sessions (overrides --limit).
    #[arg(long)]
    pub all: bool,
    /// Fail on unrecognised log lines instead of skipping.
    #[arg(long)]
    pub strict: bool,
    /// Retry-cluster window in milliseconds (default: 30 000).
    #[arg(long = "retry-window", default_value_t = 30_000)]
    pub retry_window: u64,
    /// Show only this session (exits 2 if not found).
    #[arg(long, value_parser = |s: &str| SessionId::try_from_raw(s))]
    pub session: Option<SessionId>,
    #[command(flatten)]
    pub format: super::output_format::OutputFormat,
    #[command(subcommand)]
    pub subcommand: Option<LogSubcommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum LogSubcommand {
    /// Show detail for the most recent session.
    Last,
    /// Show detail for a specific session (prefix match).
    Show {
        /// Session ID or unambiguous prefix.
        id: String,
    },
    /// Search commands across all sessions.
    Search {
        /// Substring to search for in command names and argv shapes.
        query: String,
        /// Maximum number of results to return (default: 20).
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Return all results (overrides --limit).
        #[arg(long)]
        all: bool,
    },
}

pub struct LogCommand {
    pub args: Args,
}

impl Command for LogCommand {
    type Report = LogReport;

    async fn execute(&self, ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        let storage = ctx
            .extensions
            .get::<crate::workspace::StorageDir>()
            .ok_or_else(|| {
                CommandError::Execution(
                    "storage unavailable: run 8v from within a project directory".into(),
                )
            })?;

        let mut sink = WarningSink::new();
        let events = crate::event_reader::read_events_lenient(storage, self.args.strict, &mut sink)
            .map_err(|e| CommandError::Execution(e.to_string()))?;

        let mut normalizer = crate::aggregator::ArgvNormalizer::new();
        let sessions = crate::aggregator::aggregate_events(
            &events,
            self.args.retry_window,
            &mut normalizer,
            &mut sink,
        );

        let all_warnings = sink.into_inner();

        if let Some(ref session_id) = self.args.session {
            let mut extra_sink = WarningSink::new();
            if self.args.all {
                extra_sink.push(Warning::FlagIgnoredForSession {
                    flag: "--all".to_string(),
                });
            }
            if self.args.limit != 20 {
                extra_sink.push(Warning::FlagIgnoredForSession {
                    flag: "--limit".to_string(),
                });
            }
            let session = sessions
                .iter()
                .find(|s| s.session_id == session_id.as_str());
            return match session {
                None => Ok(LogReport::Empty),
                Some(s) => {
                    let mut warnings = all_warnings;
                    warnings.extend(extra_sink.into_inner());
                    Ok(LogReport::Drill(Box::new(drill::build_drill_report(
                        s,
                        warnings,
                        self.args.retry_window,
                    ))))
                }
            };
        }

        match &self.args.subcommand {
            None => {
                let limit = if self.args.all {
                    sessions.len()
                } else {
                    self.args.limit
                };
                Ok(LogReport::Sessions(Box::new(
                    sessions::build_sessions_table(&sessions, limit, all_warnings),
                )))
            }
            Some(LogSubcommand::Last) => {
                let session = resolve_last_session(&sessions)
                    .ok_or_else(|| CommandError::Execution("no sessions found".into()))?;
                Ok(LogReport::Drill(Box::new(drill::build_drill_report(
                    session,
                    all_warnings,
                    self.args.retry_window,
                ))))
            }
            Some(LogSubcommand::Show { id }) => {
                let session = resolve_session_prefix(&sessions, id)?;
                Ok(LogReport::Drill(Box::new(drill::build_drill_report(
                    session,
                    all_warnings,
                    self.args.retry_window,
                ))))
            }
            Some(LogSubcommand::Search { query, limit, all }) => {
                Ok(LogReport::Search(Box::new(search::build_search_results(
                    &sessions,
                    query,
                    if *all { usize::MAX } else { *limit },
                    all_warnings,
                ))))
            }
        }
    }
}

/// Return the session with the greatest last-activity timestamp.
///
/// "Last activity" = the `timestamp_ms` of the final `CommandStarted` in the
/// session's command list (commands are stored in chronological order).
/// This differs from `sessions.last()` (which picks the session with the
/// latest *first*-seen event) when sessions interleave in the event log.
fn resolve_last_session(
    sessions: &[crate::aggregator::SessionAggregate],
) -> Option<&crate::aggregator::SessionAggregate> {
    sessions.iter().max_by_key(|s| {
        s.commands
            .last()
            .map(|c| {
                c.completed
                    .as_ref()
                    .map(|cc| cc.timestamp_ms.as_millis())
                    .unwrap_or_else(|| c.started.timestamp_ms.as_millis())
            })
            .unwrap_or(i64::MIN)
    })
}

fn resolve_session_prefix<'a>(
    sessions: &'a [crate::aggregator::SessionAggregate],
    prefix: &str,
) -> Result<&'a crate::aggregator::SessionAggregate, CommandError> {
    let matches: Vec<_> = sessions
        .iter()
        .filter(|s| s.session_id.starts_with(prefix))
        .collect();
    match matches.len() {
        0 => Err(CommandError::Execution(format!(
            "no session matches prefix: {}",
            prefix
        ))),
        1 => Ok(matches[0]),
        _ => Err(CommandError::Execution(format!(
            "ambiguous prefix '{}' matches {} sessions",
            prefix,
            matches.len()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::{CommandRecord, SessionAggregate};
    use o8v_core::caller::Caller;
    use o8v_core::events::lifecycle::{CommandCompleted, CommandStarted};
    use o8v_core::types::{SessionId, TimestampMs};

    fn make_record(
        session_id: &str,
        run_id: &str,
        started_ms: i64,
        completed_ms: i64,
    ) -> CommandRecord {
        CommandRecord {
            started: CommandStarted {
                event: "CommandStarted".to_string(),
                run_id: run_id.to_string(),
                timestamp_ms: TimestampMs::from_millis(started_ms),
                version: "0.0.0".to_string(),
                caller: Caller::Cli,
                command: "check".to_string(),
                argv: vec!["check".to_string(), ".".to_string()],
                command_bytes: 5,
                command_token_estimate: 1,
                project_path: None,
                agent_info: None,
                session_id: SessionId::from_raw_unchecked(session_id.to_string()),
            },
            completed: Some(CommandCompleted {
                event: "CommandCompleted".to_string(),
                run_id: run_id.to_string(),
                timestamp_ms: TimestampMs::from_millis(completed_ms),
                output_bytes: 10,
                token_estimate: 2,
                duration_ms: (completed_ms - started_ms) as u64,
                success: true,
                session_id: SessionId::from_raw_unchecked(session_id.to_string()),
            }),
            argv_shape: "check .".to_string(),
        }
    }

    /// A7 regression: session with latest last-activity wins, not latest first-event.
    #[test]
    fn resolve_last_session_picks_by_latest_activity() {
        // Session A: starts at T=100, completes at T=5000 (long-running)
        // Session B: starts at T=200, completes at T=300 (short, starts later)
        // Aggregator sort: A first (first-event=100), B last (first-event=200).
        // sessions.last() => B (wrong). resolve_last_session => A (correct).
        let session_a = SessionAggregate {
            session_id: "ses_A".to_string(),
            commands: vec![make_record("ses_A", "run_a", 100, 5000)],
            retry_clusters: vec![],
            failure_clusters: vec![],
        };
        let session_b = SessionAggregate {
            session_id: "ses_B".to_string(),
            commands: vec![make_record("ses_B", "run_b", 200, 300)],
            retry_clusters: vec![],
            failure_clusters: vec![],
        };
        let sessions = vec![session_a, session_b];
        let last = resolve_last_session(&sessions).expect("non-empty");
        assert_eq!(
            last.session_id, "ses_A",
            "must pick session A (last-activity T=5000 > B's T=300)"
        );
    }
}
