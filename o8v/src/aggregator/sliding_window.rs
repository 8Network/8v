// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Two-pointer maximal-run helper over time-ordered records.

use o8v_core::types::{DurationMs, TimestampMs};

/// Partition `records` into maximal contiguous runs where the first-to-last span
/// (i.e. `ts[end] - ts[start]`) is within `window` milliseconds.
///
/// Runs shorter than 2 elements are discarded (a single event cannot cluster).
/// Reversed timestamps (where `ts(records[i]) > ts(records[i+1])`) break the
/// run cleanly via `checked_sub` — no overflow.
pub(crate) fn sliding_windows<T>(
    records: &[T],
    window: DurationMs,
    ts: impl Fn(&T) -> TimestampMs,
) -> Vec<&[T]> {
    let mut result = Vec::new();
    if records.len() < 2 {
        return result;
    }
    let mut start = 0usize;
    let mut end = 1usize;
    while end <= records.len() {
        // Decide whether to extend the current run or flush it.
        let gap_ok = if end < records.len() {
            // Check total span from run start to candidate next element.
            ts(&records[end])
                .checked_sub(ts(&records[start]))
                .map(|d| d <= window)
                .unwrap_or(false) // reversed timestamp → break
        } else {
            false // past-the-end: always flush
        };

        if gap_ok {
            end += 1;
        } else {
            // Flush [start..end] if it has ≥ 2 elements.
            if end - start >= 2 {
                result.push(&records[start..end]);
            }
            start = end;
            end += 1;
        }
    }
    result
}

/// Sort `durations` in place and return the `p`-th percentile value.
///
/// Returns `None` only if `durations` is empty (callers guard on
/// `MIN_PERCENTILE_SAMPLES` before calling).
pub(crate) fn percentile_from_sorted(durations: &mut [u64], p: u32) -> Option<u64> {
    if durations.is_empty() {
        return None;
    }
    durations.sort_unstable();
    let idx = (durations.len() * p as usize / 100).min(durations.len() - 1);
    Some(durations[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts_slice(ms: &[i64]) -> Vec<TimestampMs> {
        ms.iter().copied().map(TimestampMs::from_millis).collect()
    }

    #[test]
    fn sliding_window_splits_long_timeline() {
        // Three events: 0ms, 100ms, 200_000ms.
        // Window = 1_000ms. First two fit; the third is far away.
        // Expect one cluster containing indices 0 and 1.
        let times = ts_slice(&[0, 100, 200_000]);
        let window = DurationMs::from_millis(1_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert_eq!(
            runs.len(),
            1,
            "expected exactly one cluster; got {}",
            runs.len()
        );
        assert_eq!(runs[0].len(), 2);
        assert_eq!(runs[0][0], TimestampMs::from_millis(0));
        assert_eq!(runs[0][1], TimestampMs::from_millis(100));
    }

    #[test]
    fn sliding_window_gap_breaks_run() {
        // Five events: two tight pairs separated by a large gap.
        // 0, 50, 100_000, 100_050, 100_100 — window = 1_000ms.
        // Expected: two clusters: [0,50] and [100_000, 100_050, 100_100].
        let times = ts_slice(&[0, 50, 100_000, 100_050, 100_100]);
        let window = DurationMs::from_millis(1_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert_eq!(runs.len(), 2, "expected two clusters; got {}", runs.len());
        assert_eq!(runs[0].len(), 2);
        assert_eq!(runs[1].len(), 3);
    }

    #[test]
    fn single_event_does_not_cluster() {
        let times = ts_slice(&[42]);
        let window = DurationMs::from_millis(1_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert!(runs.is_empty(), "a single event must not form a cluster");
    }

    #[test]
    fn reversed_timestamps_break_run() {
        // Timestamps going backwards: 1000, 500, 2000.
        // The step 1000→500 is reversed: checked_sub returns None → break.
        // Only [500, 2000] spans ≤ window, but 500 starts a new run of length 2
        // → that run [500, 2000] is within window=5_000 so it is emitted.
        // The first pair [1000, 500] must NOT be emitted (reversed).
        let times = ts_slice(&[1_000, 500, 2_000]);
        let window = DurationMs::from_millis(5_000);
        let runs = sliding_windows(&times, window, |t| *t);
        // [1000, 500]: reversed → breaks run, run length 1 → discarded.
        // [500, 2000]: gap = 1500 ≤ 5000 → cluster of length 2.
        assert_eq!(
            runs.len(),
            1,
            "only the non-reversed pair should cluster; got {runs:?}"
        );
        assert_eq!(runs[0][0], TimestampMs::from_millis(500));
        assert_eq!(runs[0][1], TimestampMs::from_millis(2_000));
    }

    #[test]
    fn total_span_exceeds_window_not_one_cluster() {
        // Three events at 0ms, 29_000ms, 58_000ms.
        // Each consecutive gap is 29_000ms ≤ 30_000ms window — so the
        // consecutive-gap check (the bug) would accept all three as one cluster.
        // Design §6 requires "first-to-last span ≤ window": 58_000 > 30_000 → NOT one cluster.
        let times = ts_slice(&[0, 29_000, 58_000]);
        let window = DurationMs::from_millis(30_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert!(
            runs.iter().all(|r| r.len() < 3),
            "a cluster spanning 58s must not be emitted under a 30s window; got {runs:?}"
        );
    }

    #[test]
    fn boundary_inclusive_span_equals_window() {
        // Two events exactly 30_000ms apart — span == window → must form one cluster (≤ is inclusive).
        let times = ts_slice(&[0, 30_000]);
        let window = DurationMs::from_millis(30_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert_eq!(
            runs.len(),
            1,
            "span == window must still form one cluster; got {runs:?}"
        );
        assert_eq!(runs[0].len(), 2);
    }

    #[test]
    fn boundary_exclusive_span_one_over_window() {
        // Two events 30_001ms apart — span > window → must NOT form a cluster.
        let times = ts_slice(&[0, 30_001]);
        let window = DurationMs::from_millis(30_000);
        let runs = sliding_windows(&times, window, |t| *t);
        assert!(
            runs.is_empty(),
            "span > window must not form a cluster; got {runs:?}"
        );
    }

    #[test]
    fn reversed_timestamp_in_sliding_windows_breaks_run_not_overflows() {
        // POC: sliding_windows computed gap as `(end_ts - start_ts) as u64`; when
        // end_ts < start_ts (reversed input), the subtraction underflowed to a huge
        // u64 that appeared within any window, forming phantom clusters.
        // Now: checked_sub returns None → unwrap_or(false) → run breaks cleanly.
        // Test at the sliding_windows unit level with reversed input directly.
        let reversed: Vec<TimestampMs> = vec![
            TimestampMs::from_millis(5000),
            TimestampMs::from_millis(1000), // reversed: 1000 < 5000
        ];
        let window = DurationMs::from_millis(30_000);
        let runs = sliding_windows(&reversed, window, |t| *t);
        // The reversed gap must not form a cluster window (run of length < 2 is discarded).
        assert!(
            runs.is_empty(),
            "reversed timestamps must break the sliding window run; got {runs:?}"
        );
    }
}
