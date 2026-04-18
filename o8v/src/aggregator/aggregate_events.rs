// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Fold a stream of Events into Vec<SessionAggregate> with warnings.

use o8v_core::events::{CommandStarted, Event};
use o8v_core::types::{TimestampMs, Warning, WarningSink};
use std::collections::{HashMap, HashSet};

use crate::aggregator::argv_normalizer::ArgvNormalizer;
use crate::aggregator::cluster_detection::{detect_failure_clusters, detect_retry_clusters};
use crate::aggregator::session_aggregate::{CommandRecord, SessionAggregate};

// ─── Single-pass aggregation ─────────────────────────────────────────────────

/// Aggregate a flat list of events into per-session buckets.
///
/// **Single pass** (§6.2 — blocks merge if violated):
/// - First pass: collect `CommandStarted` into a pending map keyed by `run_id`.
/// - Simultaneously: join `CommandCompleted` as they arrive; insert completed records.
/// - After iteration: orphan `CommandStarted` without a matching Completed → incomplete.
///
/// Events with invalid or empty `session_id` are dropped with `Warning::EmptySessionId`
/// at the wire boundary (Layer 3) and never reach this function.
///
/// Duplicate `run_id` in `CommandStarted`: first wins; warning added via `warnings`.
///
/// Warnings are pushed directly into `warnings`; callers do not need to collect a
/// separate return value. Returns the sorted `Vec<SessionAggregate>`.
pub fn aggregate_events(
    events: &[Event],
    retry_window_ms: u64,
    normalizer: &mut ArgvNormalizer,
    warnings: &mut WarningSink,
) -> Vec<SessionAggregate> {
    // Map session_id → in-progress session builder
    let mut sessions: HashMap<String, SessionBuilder> = HashMap::new();

    for event in events {
        match event {
            Event::CommandStarted(started) => {
                if started.session_id.as_str().is_empty() {
                    warnings.push(Warning::EmptySessionId {
                        at: started.timestamp_ms,
                        reason: "empty session_id in CommandStarted".to_string(),
                    });
                    continue;
                }
                let session_key = started.session_id.as_str().to_string();
                let builder = sessions.entry(session_key).or_default();

                if builder.pending.contains_key(&started.run_id) {
                    warnings.push(Warning::DuplicateStarted {
                        run_id: started.run_id.clone(),
                    });
                    continue;
                }

                let argv_shape = normalizer.normalize_argv(
                    &started.argv,
                    started.project_path.as_deref(),
                    started.session_id.as_str(),
                    warnings,
                );

                builder.pending.insert(
                    started.run_id.clone(),
                    PendingRecord {
                        started: started.clone(),
                        argv_shape,
                    },
                );
            }

            Event::CommandCompleted(completed) => {
                // Find the session that owns this run_id
                // We must search all sessions because the CommandStarted
                // could have arrived in any session bucket.
                let session_key = find_session_for_run_id(&mut sessions, &completed.run_id);

                if let Some(key) = session_key {
                    let builder = sessions.get_mut(&key).expect("key must exist");
                    if let Some(pending) = builder.pending.remove(&completed.run_id) {
                        builder.completed_run_ids.insert(completed.run_id.clone());
                        builder.completed.push(CommandRecord {
                            argv_shape: pending.argv_shape,
                            started: pending.started,
                            completed: Some(completed.clone()),
                        });
                    }
                } else {
                    // No pending Started found — check if already completed (duplicate) or truly orphan
                    let already_completed = sessions
                        .values()
                        .any(|b| b.completed_run_ids.contains(completed.run_id.as_str()));
                    if already_completed {
                        warnings.push(Warning::DuplicateCompleted {
                            run_id: completed.run_id.clone(),
                        });
                    } else {
                        warnings.push(Warning::OrphanCompleted {
                            run_id: completed.run_id.clone(),
                        });
                    }
                }
            }

            Event::Unknown { .. } => {
                // Forward-compat: skip unknown event types
            }
        }
    }

    // Flush orphan (incomplete) CommandStarted records into completed list
    for builder in sessions.values_mut() {
        for (run_id, pending) in builder.pending.drain() {
            warnings.push(Warning::OrphanStarted {
                run_id: run_id.clone(),
            });
            builder.completed.push(CommandRecord {
                argv_shape: pending.argv_shape,
                started: pending.started,
                completed: None,
            });
        }
    }

    // Sort each session's commands by start timestamp, then detect clusters
    let mut result: Vec<SessionAggregate> = sessions
        .into_iter()
        .map(|(session_id, mut builder)| {
            builder.completed.sort_by_key(|c| c.started.timestamp_ms);

            let retry_clusters = detect_retry_clusters(&builder.completed, retry_window_ms);
            let failure_clusters = detect_failure_clusters(&builder.completed, retry_window_ms);

            SessionAggregate {
                session_id,
                commands: builder.completed,
                retry_clusters,
                failure_clusters,
            }
        })
        .collect();

    // Sort sessions by first-event timestamp (oldest first)
    result.sort_by_key(|s| {
        s.commands
            .first()
            .map(|c| c.started.timestamp_ms)
            .unwrap_or(TimestampMs::from_millis(i64::MAX))
    });

    result
}

/// Search all session builders for one containing the given run_id in pending.
fn find_session_for_run_id(
    sessions: &mut HashMap<String, SessionBuilder>,
    run_id: &str,
) -> Option<String> {
    sessions
        .iter()
        .find(|(_, b)| b.pending.contains_key(run_id))
        .map(|(k, _)| k.clone())
}

// ─── Internal builder types ──────────────────────────────────────────────────

#[derive(Default)]
struct SessionBuilder {
    /// CommandStarted events waiting for their matching Completed.
    pending: HashMap<String, PendingRecord>,
    /// Fully resolved (and orphan) records.
    completed: Vec<CommandRecord>,
    /// run_ids that have already been paired with a CommandCompleted.
    /// Used to distinguish DuplicateCompleted from OrphanCompleted.
    completed_run_ids: HashSet<String>,
}

struct PendingRecord {
    started: CommandStarted,
    argv_shape: String,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use o8v_core::events::lifecycle::{CommandCompleted, CommandStarted};
    use o8v_core::events::Event;
    use o8v_core::types::{SessionId, WarningSink};

    fn make_started(
        run_id: &str,
        session_id: &str,
        command: &str,
        argv: Vec<&str>,
        project_path: Option<&str>,
        timestamp_ms: i64,
    ) -> Event {
        let argv_s: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        let command_bytes = command.len() as u64;
        Event::CommandStarted(CommandStarted {
            event: "CommandStarted".to_string(),
            run_id: run_id.to_string(),
            timestamp_ms: TimestampMs::from_millis(timestamp_ms),
            version: "0.0.0".to_string(),
            caller: o8v_core::caller::Caller::Cli,
            command: command.to_string(),
            argv: argv_s,
            command_bytes,
            command_token_estimate: command_bytes / 4,
            project_path: project_path.map(|s| s.to_string()),
            agent_info: None,
            session_id: SessionId::from_raw_unchecked(session_id.to_string()),
        })
    }

    fn make_completed(
        run_id: &str,
        session_id: &str,
        duration_ms: u64,
        success: bool,
        timestamp_ms: i64,
    ) -> Event {
        Event::CommandCompleted(CommandCompleted {
            event: "CommandCompleted".to_string(),
            run_id: run_id.to_string(),
            timestamp_ms: TimestampMs::from_millis(timestamp_ms),
            output_bytes: 100,
            token_estimate: 25,
            duration_ms,
            success,
            session_id: SessionId::from_raw_unchecked(session_id.to_string()),
        })
    }

    #[test]
    fn basic_join_started_and_completed() {
        let events = vec![
            make_started(
                "r1",
                "ses_A",
                "read",
                vec!["read", "src/main.rs"],
                None,
                1000,
            ),
            make_completed("r1", "ses_A", 50, true, 1050),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.session_id, "ses_A");
        assert_eq!(s.commands.len(), 1);
        let cmd = &s.commands[0];
        assert!(cmd.is_complete());
        assert_eq!(cmd.success(), Some(true));
        assert_eq!(cmd.duration_ms(), Some(50));
    }

    #[test]
    fn orphan_started_is_incomplete() {
        let events = vec![
            make_started("r2", "ses_B", "check", vec!["check", "."], None, 2000),
            // No matching Completed
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.incomplete_count(), 1);
        assert!(!s.commands[0].is_complete());
    }

    #[test]
    fn duplicate_run_id_first_wins() {
        let events = vec![
            make_started("r4", "ses_C", "read", vec!["read", "a.rs"], None, 4000),
            make_started("r4", "ses_C", "read", vec!["read", "b.rs"], None, 4001), // dup
            make_completed("r4", "ses_C", 10, true, 4010),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);
        let dups = sink.into_inner();

        assert_eq!(dups.len(), 1);
        match &dups[0] {
            Warning::DuplicateStarted { run_id } => assert_eq!(run_id, "r4"),
            other => panic!("expected DuplicateStarted, got {other:?}"),
        }
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].commands.len(), 1);
    }

    #[test]
    fn multiple_sessions_separated() {
        let events = vec![
            make_started("r5", "ses_X", "read", vec!["read", "a.rs"], None, 5000),
            make_completed("r5", "ses_X", 10, true, 5010),
            make_started("r6", "ses_Y", "read", vec!["read", "b.rs"], None, 6000),
            make_completed("r6", "ses_Y", 10, true, 6010),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert_eq!(sessions.len(), 2);
        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        assert!(ids.contains(&"ses_X"));
        assert!(ids.contains(&"ses_Y"));
    }

    #[test]
    fn retry_cluster_detected() {
        // Same (command, argv_shape), two occurrences within window
        let events = vec![
            make_started("r7", "ses_D", "check", vec!["check", "."], None, 1000),
            make_completed("r7", "ses_D", 100, false, 1100),
            make_started("r8", "ses_D", "check", vec!["check", "."], None, 5000),
            make_completed("r8", "ses_D", 100, false, 5100),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        let s = &sessions[0];
        assert_eq!(s.retry_clusters.len(), 1);
        let cluster = &s.retry_clusters[0];
        assert_eq!(cluster.command, "check");
        assert_eq!(cluster.run_ids.len(), 2);
    }

    #[test]
    fn retry_cluster_outside_window_not_detected() {
        // Span = 60s, window = 30s → no cluster
        let events = vec![
            make_started("r9", "ses_E", "check", vec!["check", "."], None, 0),
            make_completed("r9", "ses_E", 100, false, 100),
            make_started("r10", "ses_E", "check", vec!["check", "."], None, 61_000),
            make_completed("r10", "ses_E", 100, false, 61_100),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert!(sessions[0].retry_clusters.is_empty());
    }

    #[test]
    fn failure_cluster_detected() {
        let events = vec![
            make_started("r11", "ses_F", "write", vec!["write", "a.rs"], None, 1000),
            make_completed("r11", "ses_F", 10, false, 1010),
            make_started("r12", "ses_F", "write", vec!["write", "a.rs"], None, 2000),
            make_completed("r12", "ses_F", 10, false, 2010),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        let s = &sessions[0];
        assert_eq!(s.failure_clusters.len(), 1);
        assert_eq!(s.failure_clusters[0].run_ids.len(), 2);
    }

    #[test]
    fn has_failures_flag() {
        let events = vec![
            make_started("r13", "ses_G", "check", vec!["check", "."], None, 1000),
            make_completed("r13", "ses_G", 10, false, 1010),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert!(sessions[0].has_failures());
    }

    #[test]
    fn argv_normalizer_quoted_string_replaced() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let shape = norm.normalize_argv(
            &[
                "write".to_string(),
                "src/main.rs".to_string(),
                "\"hello world\"".to_string(),
            ],
            None,
            "ses_test",
            &mut sink,
        );
        assert!(
            shape.contains("<str>"),
            "quoted string should become <str>; got: {shape}"
        );
    }

    #[test]
    fn argv_normalizer_tmp_path_replaced() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let shape = norm.normalize_argv(
            &["read".to_string(), "/tmp/fixture.rs".to_string()],
            None,
            "ses_test",
            &mut sink,
        );
        assert!(
            shape.contains("<tmp>"),
            "tmp path should become <tmp>; got: {shape}"
        );
    }

    #[test]
    fn argv_normalizer_warns_once_per_session_for_missing_project() {
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        // Two calls with different relative paths, same session, no project_path
        let _ = norm.normalize_argv(
            &["read".to_string(), "./src/main.rs".to_string()],
            None,
            "ses_warn",
            &mut sink,
        );
        let _ = norm.normalize_argv(
            &["read".to_string(), "./src/lib.rs".to_string()],
            None,
            "ses_warn",
            &mut sink,
        );
        // Should warn exactly once for that session
        let all_warnings = sink.into_inner();
        let session_warnings: Vec<_> = all_warnings
            .iter()
            .filter(|w| {
                matches!(
                    w,
                    Warning::NormalizerBasenameFallback { session, .. }
                        if session.as_str() == "ses_warn"
                )
            })
            .collect();
        assert_eq!(
            session_warnings.len(),
            1,
            "should warn once per session; got: {:?}",
            all_warnings
        );
    }

    #[test]
    fn sessions_sorted_oldest_first() {
        // ses_newer starts at 10000, ses_older at 100 — result should be [older, newer]
        let events = vec![
            make_started(
                "r20",
                "ses_newer",
                "read",
                vec!["read", "a.rs"],
                None,
                10_000,
            ),
            make_completed("r20", "ses_newer", 5, true, 10_005),
            make_started("r21", "ses_older", "read", vec!["read", "b.rs"], None, 100),
            make_completed("r21", "ses_older", 5, true, 105),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        assert_eq!(sessions[0].session_id, "ses_older");
        assert_eq!(sessions[1].session_id, "ses_newer");
    }

    // ─── Counterexample tests (POC-regression pins) ───────────────────────────

    #[test]
    fn empty_session_id_started_emits_warning_and_event_dropped() {
        // POC: empty session_id was silently mapped to "ses_legacy" bucket.
        // Now: Warning::EmptySessionId is emitted and the event is dropped (no session created).
        let events = vec![make_started(
            "r_empty",
            "",
            "check",
            vec!["check", "."],
            None,
            1000,
        )];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);
        let warnings = sink.into_inner();

        assert!(
            sessions.is_empty(),
            "event with empty session_id must be dropped"
        );
        assert_eq!(warnings.len(), 1, "must emit exactly one warning");
        assert!(
            matches!(&warnings[0], Warning::EmptySessionId { .. }),
            "expected EmptySessionId, got {:?}",
            warnings[0]
        );
    }

    #[test]
    fn empty_session_id_warning_carries_timestamp() {
        // POC: no timestamp was preserved for orphaned events (silent drop).
        // Now: Warning::EmptySessionId carries `at` so callers can correlate to timeline.
        let events = vec![make_started(
            "r_ts",
            "",
            "read",
            vec!["read", "a.rs"],
            None,
            9_000,
        )];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let _ = aggregate_events(&events, 30_000, &mut norm, &mut sink);
        let warnings = sink.into_inner();

        assert!(
            matches!(&warnings[0], Warning::EmptySessionId { at, .. } if at.as_millis() == 9_000),
            "EmptySessionId must carry the event's timestamp; got {:?}",
            warnings[0]
        );
    }

    #[test]
    fn duplicate_started_warning_carries_run_id() {
        // POC: duplicate run_id was silently overwritten (second write clobbered first).
        // Now: Warning::DuplicateStarted carries the exact run_id that was rejected.
        let events = vec![
            make_started("r_dup", "ses_A", "check", vec!["check", "."], None, 1000),
            make_started("r_dup", "ses_A", "check", vec!["check", ".."], None, 1001),
            make_completed("r_dup", "ses_A", 50, true, 1050),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);
        let warnings = sink.into_inner();

        assert_eq!(warnings.len(), 1);
        assert!(
            matches!(&warnings[0], Warning::DuplicateStarted { run_id } if run_id == "r_dup"),
            "DuplicateStarted must name the rejected run_id; got {:?}",
            warnings[0]
        );
        // First Started wins — only one command record.
        assert_eq!(sessions[0].commands.len(), 1);
    }

    #[test]
    fn percentile_below_min_samples_returns_none() {
        // POC: percentile was computed on any non-empty sample set (even a single value).
        // Now: returns None when sample count < MIN_PERCENTILE_SAMPLES (5).
        let events = vec![
            make_started("r1", "ses_P", "check", vec!["check", "."], None, 1000),
            make_completed("r1", "ses_P", 200, true, 1200),
            make_started("r2", "ses_P", "check", vec!["check", "."], None, 2000),
            make_completed("r2", "ses_P", 300, true, 2300),
        ];
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 30_000, &mut norm, &mut sink);

        let s = &sessions[0];
        // 2 samples < MIN_PERCENTILE_SAMPLES(5) → must return None.
        assert_eq!(
            s.duration_percentile(50),
            None,
            "duration_percentile must return None with fewer than {} samples; samples=2",
            SessionAggregate::MIN_PERCENTILE_SAMPLES
        );
    }

    #[test]
    fn percentile_at_exactly_min_samples_returns_some() {
        // POC: no minimum-sample guard existed.
        // Now: exactly MIN_PERCENTILE_SAMPLES (5) is the boundary — must return Some.
        let mut events = Vec::new();
        for i in 0..5_u64 {
            let run = format!("r_min{i}");
            events.push(make_started(
                &run,
                "ses_M",
                "check",
                vec!["check", "."],
                None,
                (i * 1000) as i64,
            ));
            events.push(make_completed(
                &run,
                "ses_M",
                100 + i * 10,
                true,
                (i * 1000 + 100) as i64,
            ));
        }
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 60_000, &mut norm, &mut sink);

        let s = &sessions[0];
        assert!(
            s.duration_percentile(50).is_some(),
            "duration_percentile must return Some at exactly {} samples",
            SessionAggregate::MIN_PERCENTILE_SAMPLES
        );
    }

    #[test]
    fn per_command_p95_skips_commands_below_min_samples() {
        // POC: per_command_p95 returned p95 even for commands with 1-2 samples.
        // Now: commands with fewer than MIN_PERCENTILE_SAMPLES samples are excluded.
        let mut events = Vec::new();
        // "check" — 4 samples (below threshold).
        for i in 0..4_u64 {
            let run = format!("r_c{i}");
            events.push(make_started(
                &run,
                "ses_C2",
                "check",
                vec!["check", "."],
                None,
                (i * 500) as i64,
            ));
            events.push(make_completed(
                &run,
                "ses_C2",
                100,
                true,
                (i * 500 + 50) as i64,
            ));
        }
        // "read" — 5 samples (at threshold, should appear).
        for i in 0..5_u64 {
            let run = format!("r_r{i}");
            events.push(make_started(
                &run,
                "ses_C2",
                "read",
                vec!["read", "a.rs"],
                None,
                (10_000 + i * 500) as i64,
            ));
            events.push(make_completed(
                &run,
                "ses_C2",
                200,
                true,
                (10_000 + i * 500 + 50) as i64,
            ));
        }
        let mut norm = ArgvNormalizer::new();
        let mut sink = WarningSink::new();
        let sessions = aggregate_events(&events, 60_000, &mut norm, &mut sink);

        let s = &sessions[0];
        let p95 = s.per_command_p95();
        let names: Vec<&str> = p95.iter().map(|(cmd, _)| cmd.as_str()).collect();
        assert!(
            !names.contains(&"check"),
            "check (4 samples) must be excluded from per_command_p95"
        );
        assert!(
            names.contains(&"read"),
            "read (5 samples) must appear in per_command_p95"
        );
    }
}
