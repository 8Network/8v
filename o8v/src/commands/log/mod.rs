// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `8v log` — session history and command drill-down.

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::log_report::LogReport;
use o8v_core::types::WarningSink;

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
                let session = sessions
                    .last()
                    .ok_or_else(|| CommandError::Execution("no sessions found".into()))?;
                Ok(LogReport::Drill(Box::new(drill::build_drill_report(
                    session,
                    all_warnings,
                ))))
            }
            Some(LogSubcommand::Show { id }) => {
                let session = resolve_session_prefix(&sessions, id)?;
                Ok(LogReport::Drill(Box::new(drill::build_drill_report(
                    session,
                    all_warnings,
                ))))
            }
            Some(LogSubcommand::Search { query }) => Ok(LogReport::Search(Box::new(
                search::build_search_results(&sessions, query, all_warnings),
            ))),
        }
    }
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
