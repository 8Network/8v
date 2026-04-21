// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use o8v_core::stats::StatsRow;

use super::buckets::parse_duration_ms;
use super::report::apply_min_n;

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
    let started = CommandStarted::new(current_run_id.clone(), Caller::Cli, "stats", vec![], None);
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
            mean_ms: None,
            ok_rate: None,
            output_bytes_per_call_mean: None,
            retry_cluster_count: 0,
        },
        StatsRow {
            label: "b".into(),
            n: 10,
            duration_ms: None,
            mean_ms: None,
            ok_rate: None,
            output_bytes_per_call_mean: None,
            retry_cluster_count: 0,
        },
    ];
    let filtered = apply_min_n(rows, 5);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "b");
}
