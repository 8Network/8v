// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Cross-session failure hotspot reducer.

use crate::aggregator::argv_normalizer::looks_like_path;
use crate::aggregator::session_aggregate::SessionAggregate;
use std::collections::HashMap;

/// Groups failures across sessions by `(command, argv_shape)` and returns the
/// top-10 hotspots sorted by failure count descending (ties: command asc,
/// argv_shape asc).  Uses raw argv tokens (not the normalized shape) so paths
/// are not hidden behind `<path>` tokens.
pub(crate) fn compute_failure_hotspots(
    sessions: &[SessionAggregate],
) -> Vec<o8v_core::stats::FailureHotspot> {
    // key: (command, argv_shape) → (count, HashMap<raw_path_token, frequency>)
    let mut map: HashMap<(String, String), (u64, HashMap<String, u64>)> = HashMap::new();

    for s in sessions {
        for rec in &s.commands {
            if let Some(c) = rec.completed.as_ref() {
                if c.success {
                    continue;
                }
            } else {
                // No completed record — still in-flight or abandoned; skip.
                continue;
            }
            let key = (rec.started.command.clone(), rec.argv_shape.clone());
            let (count, paths) = map.entry(key).or_default();
            *count += 1;
            // Use raw argv tokens (not the normalized shape) so paths are not
            // replaced with `<path>` tokens by ArgvNormalizer.
            if let Some(tok) = rec.started.argv.iter().find(|t| looks_like_path(t)) {
                *paths.entry(tok.clone()).or_default() += 1;
            }
        }
    }

    let mut hotspots: Vec<o8v_core::stats::FailureHotspot> = map
        .into_iter()
        .map(|((command, argv_shape), (count, paths))| {
            // Deterministic top_path: iterate paths sorted lexicographically,
            // keep running best; update only on strict improvement (ties → lex-smallest wins).
            let (top_path, top_path_count) = {
                let mut sorted_paths: Vec<(String, u64)> = paths.into_iter().collect();
                sorted_paths.sort_by(|(a, _), (b, _)| a.cmp(b));
                let mut best_path: Option<String> = None;
                let mut best_count: u64 = 0;
                for (p, c) in sorted_paths {
                    if c > best_count {
                        best_path = Some(p);
                        best_count = c;
                    }
                }
                (best_path, best_count)
            };
            o8v_core::stats::FailureHotspot {
                command,
                argv_shape,
                count,
                top_path,
                top_path_count,
            }
        })
        .collect();

    hotspots.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.command.cmp(&b.command))
            .then_with(|| a.argv_shape.cmp(&b.argv_shape))
    });
    hotspots.truncate(10);
    hotspots
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::events::lifecycle::{CommandCompleted, CommandStarted};
    use o8v_core::types::{SessionId, TimestampMs};

    use crate::aggregator::session_aggregate::{CommandRecord, SessionAggregate};
    use o8v_core::caller::Caller;

    /// Build a failed CommandRecord with the given raw argv tokens.
    /// `argv_shape` is the pre-normalized shape stored alongside.
    fn make_failed_record(command: &str, raw_argv: Vec<&str>, argv_shape: &str) -> CommandRecord {
        CommandRecord {
            started: CommandStarted {
                event: "CommandStarted".to_string(),
                run_id: "r1".to_string(),
                timestamp_ms: TimestampMs::from_millis(1000),
                version: "0.0.0".to_string(),
                caller: Caller::Cli,
                command: command.to_string(),
                argv: raw_argv.into_iter().map(str::to_string).collect(),
                command_bytes: command.len() as u64,
                command_token_estimate: (command.len() / 4) as u64,
                project_path: None,
                agent_info: None,
                session_id: SessionId::from_raw_unchecked("ses_TEST".to_string()),
            },
            completed: Some(CommandCompleted {
                event: "CommandCompleted".to_string(),
                run_id: "r1".to_string(),
                timestamp_ms: TimestampMs::from_millis(1050),
                output_bytes: 0,
                token_estimate: 0,
                duration_ms: 50,
                success: false,
                session_id: SessionId::from_raw_unchecked("ses_TEST".to_string()),
            }),
            argv_shape: argv_shape.to_string(),
        }
    }

    fn make_hotspot_session(commands: Vec<CommandRecord>) -> SessionAggregate {
        SessionAggregate {
            session_id: "ses_TEST".to_string(),
            commands,
            retry_clusters: vec![],
            failure_clusters: vec![],
        }
    }

    /// Pre-fix: `first_path_token` searched `argv_shape`, which normalizer
    /// replaces with `<path>` tokens → `top_path` was always `None`.
    /// Post-fix: raw argv is searched → `top_path` is the most frequent real
    /// path token.
    #[test]
    fn failure_hotspots_top_path_reads_raw_argv_not_shape() {
        // 3 failures of the same (command, argv_shape).
        // raw paths: src/a.rs, src/b.rs, src/a.rs  →  src/a.rs wins (count=2)
        let shape = "check <path>";
        let mut r1 = make_failed_record("check", vec!["check", "src/a.rs"], shape);
        r1.started.run_id = "r1".to_string();
        let mut r2 = make_failed_record("check", vec!["check", "src/b.rs"], shape);
        r2.started.run_id = "r2".to_string();
        let mut r3 = make_failed_record("check", vec!["check", "src/a.rs"], shape);
        r3.started.run_id = "r3".to_string();

        let sessions = vec![make_hotspot_session(vec![r1, r2, r3])];
        let hotspots = compute_failure_hotspots(&sessions);

        assert_eq!(hotspots.len(), 1);
        let h = &hotspots[0];
        assert_eq!(h.count, 3);
        assert_eq!(
            h.top_path.as_deref(),
            Some("src/a.rs"),
            "top_path must be the raw path token, not None or <path>"
        );
        assert_eq!(h.top_path_count, 2);
    }

    /// Failures whose argv contains no path-like token must produce
    /// `top_path == None` and `top_path_count == 0`.
    #[test]
    fn failure_hotspots_top_path_none_when_no_path_arg() {
        let shape = "cargo --version";
        let mut r = make_failed_record("cargo", vec!["cargo", "--version"], shape);
        r.started.run_id = "r1".to_string();

        let sessions = vec![make_hotspot_session(vec![r])];
        let hotspots = compute_failure_hotspots(&sessions);

        assert_eq!(hotspots.len(), 1);
        let h = &hotspots[0];
        assert_eq!(h.count, 1);
        assert_eq!(h.top_path, None);
        assert_eq!(h.top_path_count, 0);
    }
}
