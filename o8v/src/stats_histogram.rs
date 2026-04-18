// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Log-spaced histogram for latency percentile computation.
//!
//! 60 buckets spanning 1 ms – 1 000 000 ms (≈ 1 000 s).
//! Bucket edges are computed once at compile time via `bucket_index`.
//! `percentile()` returns `None` when the sample count is below the minimum
//! required for a statistically meaningful estimate (n < 5).

const BUCKETS: usize = 60;
const MIN_MS: f64 = 1.0;
const MAX_MS: f64 = 1_000_000.0;
const MIN_SAMPLES: u64 = 5;

/// Map a duration in milliseconds to a bucket index in [0, BUCKETS).
///
/// Uses log-linear spacing between MIN_MS and MAX_MS.
/// Values below MIN_MS clamp to bucket 0; values above MAX_MS clamp to BUCKETS-1.
#[inline]
fn bucket_index(ms: u64) -> usize {
    let ms_f = ms as f64;
    if ms_f <= MIN_MS {
        return 0;
    }
    if ms_f >= MAX_MS {
        return BUCKETS - 1;
    }
    let log_min = MIN_MS.ln();
    let log_max = MAX_MS.ln();
    let idx = ((ms_f.ln() - log_min) / (log_max - log_min) * BUCKETS as f64) as usize;
    idx.min(BUCKETS - 1)
}

/// Upper bound of bucket `i` in milliseconds (used when returning percentile values).
///
/// Gives the highest ms value that maps to bucket `i`.
///
/// At low bucket indices the log-spaced float values are so close together that
/// `ceil()` would produce equal integers for adjacent buckets (e.g. buckets 0, 1, 2
/// all ceil to 2 ms).  We enforce strict monotonicity by taking the maximum of the
/// log-derived ceil and `(i + 1)` ms — a linear lower-bound that is tight only for
/// the first handful of buckets and has no effect on the large-ms end.
#[inline]
fn bucket_upper_ms(i: usize) -> u64 {
    let log_min = MIN_MS.ln();
    let log_max = MAX_MS.ln();
    let upper_f = (log_min + (i + 1) as f64 / BUCKETS as f64 * (log_max - log_min)).exp();
    let log_ceil = upper_f.ceil() as u64;
    // Guarantee strict monotonicity: bucket i's upper bound must be > bucket i-1's.
    // Since bucket 0's lower bound is 1 ms, bucket i's upper bound is at least i+1+1 = i+2
    // ... but the simplest safe lower bound is just (i + 2) for the first bucket and
    // (i + 1) for the rest.  Using (i as u64 + 2) covers both cases with a single rule.
    log_ceil.max(i as u64 + 2)
}

/// 60-bucket log-spaced histogram for latency values.
///
/// # Usage
/// ```rust
/// use o8v::stats_histogram::Histogram;
/// let mut h = Histogram::new();
/// h.record(50);   // 50 ms
/// h.record(120);
/// // ... add more samples
/// if let Some(p50) = h.percentile(0.50) {
///     println!("p50 = {} ms", p50);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: [u64; BUCKETS],
    count: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            buckets: [0u64; BUCKETS],
            count: 0,
        }
    }
}

impl Histogram {
    /// Create a new empty histogram.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single observation in milliseconds.
    pub fn record(&mut self, ms: u64) {
        let idx = bucket_index(ms);
        self.buckets[idx] += 1;
        self.count += 1;
    }

    /// Total number of observations recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Compute the p-th percentile (0.0–1.0).
    ///
    /// Returns `None` when fewer than `MIN_SAMPLES` observations have been recorded,
    /// because the estimate is not meaningful at very small n.
    ///
    /// Returns the upper bound of the bucket that contains the p-th percentile,
    /// which slightly over-estimates — acceptable for display purposes.
    pub fn percentile(&self, p: f64) -> Option<u64> {
        if self.count < MIN_SAMPLES {
            return None;
        }
        // Target rank: how many observations should be at or below this percentile.
        let target = (p * self.count as f64).ceil() as u64;
        let mut cumulative: u64 = 0;
        for (i, &bucket_count) in self.buckets.iter().enumerate() {
            cumulative += bucket_count;
            if cumulative >= target {
                return Some(bucket_upper_ms(i));
            }
        }
        // Fallback: return the last bucket's upper bound (shouldn't reach here).
        Some(bucket_upper_ms(BUCKETS - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_histogram_returns_none() {
        let h = Histogram::new();
        assert_eq!(h.percentile(0.50), None);
        assert_eq!(h.percentile(0.95), None);
        assert_eq!(h.percentile(0.99), None);
    }

    #[test]
    fn below_min_samples_returns_none() {
        let mut h = Histogram::new();
        for _ in 0..4 {
            h.record(100);
        }
        assert_eq!(h.count(), 4);
        assert_eq!(h.percentile(0.50), None, "n=4 must return None");
    }

    #[test]
    fn at_min_samples_returns_some() {
        let mut h = Histogram::new();
        for _ in 0..5 {
            h.record(100);
        }
        assert_eq!(h.count(), 5);
        assert!(h.percentile(0.50).is_some(), "n=5 must return Some");
    }

    #[test]
    fn record_clamps_below_min() {
        let mut h = Histogram::new();
        // Values <= 1ms all land in bucket 0.
        for _ in 0..5 {
            h.record(0);
        }
        // p50 should return the upper bound of bucket 0.
        let p50 = h.percentile(0.50).unwrap();
        assert!(
            p50 >= 1,
            "upper bound of bucket 0 must be >= 1ms, got {}",
            p50
        );
    }

    #[test]
    fn record_clamps_above_max() {
        let mut h = Histogram::new();
        for _ in 0..5 {
            h.record(u64::MAX);
        }
        let p99 = h.percentile(0.99).unwrap();
        assert_eq!(p99, bucket_upper_ms(BUCKETS - 1));
    }

    #[test]
    fn p50_p95_p99_ordering() {
        let mut h = Histogram::new();
        // Insert a spread of values: 10 small, 10 medium, 10 large.
        for _ in 0..10 {
            h.record(10);
        }
        for _ in 0..10 {
            h.record(1_000);
        }
        for _ in 0..10 {
            h.record(100_000);
        }
        let p50 = h.percentile(0.50).unwrap();
        let p95 = h.percentile(0.95).unwrap();
        let p99 = h.percentile(0.99).unwrap();
        assert!(p50 <= p95, "p50={} must be <= p95={}", p50, p95);
        assert!(p95 <= p99, "p95={} must be <= p99={}", p95, p99);
    }

    #[test]
    fn uniform_distribution_p50_is_midrange() {
        let mut h = Histogram::new();
        // Insert 100 values evenly from 1 to 100 ms.
        for ms in 1u64..=100 {
            h.record(ms);
        }
        let p50 = h.percentile(0.50).unwrap();
        // p50 should be somewhere around 50ms — not 1ms or 100ms.
        // Due to log spacing, the bucket may be wide; accept 5–200ms.
        assert!(
            (5..=200).contains(&p50),
            "p50 of 1..=100ms distribution should be in [5,200], got {}",
            p50
        );
    }

    /// Adversarial: verify boundary bucket math doesn't produce wrong percentile
    /// by checking that cumulative traversal hits exactly the right bucket.
    #[test]
    fn percentile_boundary_bucket_math() {
        let mut h = Histogram::new();
        // Put all 10 samples into bucket for ~1000ms.
        for _ in 0..10 {
            h.record(1_000);
        }
        let p50 = h.percentile(0.50).unwrap();
        let p99 = h.percentile(0.99).unwrap();

        // Both p50 and p99 should be in the same bucket (all values are identical).
        assert_eq!(
            p50, p99,
            "with uniform distribution all p-values must be in same bucket, got p50={} p99={}",
            p50, p99
        );

        // The bucket upper bound for 1000ms should be reasonable: 1ms–10000ms range.
        assert!(
            (500..=10_000).contains(&p50),
            "bucket upper bound for 1000ms observations should be in [500, 10000], got {}",
            p50
        );

        // Verify: 0-count buckets before the target bucket are not incorrectly selected.
        // Put 5 samples at 1ms and 5 at 1_000_000ms.
        let mut h2 = Histogram::new();
        for _ in 0..5 {
            h2.record(1);
        }
        for _ in 0..5 {
            h2.record(1_000_000);
        }
        let p40 = h2.percentile(0.40).unwrap();
        let p60 = h2.percentile(0.60).unwrap();
        // p40 (4th of 10 = 40th percentile) lands in the low bucket.
        // p60 (6th of 10 = 60th percentile) lands in the high bucket.
        assert!(
            p40 < p60,
            "p40 for bimodal 1ms/1000000ms dist must be < p60, got p40={} p60={}",
            p40,
            p60
        );
        // The split must be dramatic — low cluster < 100ms, high cluster > 100_000ms.
        assert!(
            p40 < 100,
            "p40 of bimodal should be in low cluster (< 100ms), got {}",
            p40
        );
        assert!(
            p60 > 100_000,
            "p60 of bimodal should be in high cluster (> 100000ms), got {}",
            p60
        );
    }

    #[test]
    fn bucket_index_is_monotonic() {
        // Verify that bucket_index is non-decreasing as ms increases.
        let test_points = [
            1u64, 2, 5, 10, 50, 100, 500, 1000, 5000, 10_000, 100_000, 1_000_000,
        ];
        let mut last_idx = 0usize;
        for &ms in &test_points {
            let idx = bucket_index(ms);
            assert!(
                idx >= last_idx,
                "bucket_index not monotonic: {}ms -> {} < previous {}",
                ms,
                idx,
                last_idx
            );
            last_idx = idx;
        }
    }

    #[test]
    fn bucket_upper_ms_is_monotonic() {
        // bucket_upper_ms must be strictly increasing.
        for i in 0..(BUCKETS - 1) {
            let lo = bucket_upper_ms(i);
            let hi = bucket_upper_ms(i + 1);
            assert!(
                hi > lo,
                "bucket_upper_ms not strictly increasing at [{}, {}]: {} >= {}",
                i,
                i + 1,
                lo,
                hi
            );
        }
    }
}
