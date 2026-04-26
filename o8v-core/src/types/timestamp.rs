// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `TimestampMs` (signed milliseconds since Unix epoch) and `DurationMs`
//! (unsigned, positive-only).
//!
//! Motivation: the POC used raw `u64` for durations computed as
//! `(later - earlier) as u64`. When timestamps were out of order, the
//! subtraction wrapped to `u64::MAX`, silently suppressing retry-cluster
//! detection. Making the subtraction return `Option<DurationMs>` forces
//! every caller to handle the reversed-clock case explicitly.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMs(i64);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DurationMs(u64);

impl TimestampMs {
    pub const fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    /// Read the wall clock. Panics only if the system clock is before the
    /// Unix epoch (1970) — a non-recoverable configuration error.
    pub fn now() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
            .as_millis() as i64;
        Self(ms)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Compute `self - earlier`. Returns `None` if `earlier > self` — the
    /// caller must treat that as a reversed-clock warning, not silently
    /// coerce to zero or wrap.
    pub fn checked_sub(self, earlier: TimestampMs) -> Option<DurationMs> {
        let diff = self.0.checked_sub(earlier.0)?;
        if diff < 0 {
            None
        } else {
            Some(DurationMs(diff as u64))
        }
    }
}

impl DurationMs {
    pub const fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }

    /// Build a `DurationMs` from a signed value, rejecting negatives.
    pub fn try_from_i64(ms: i64) -> Option<Self> {
        if ms < 0 {
            None
        } else {
            Some(Self(ms as u64))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_sub_forward_in_time() {
        let a = TimestampMs::from_millis(1_000);
        let b = TimestampMs::from_millis(3_500);
        assert_eq!(b.checked_sub(a), Some(DurationMs::from_millis(2_500)));
    }

    #[test]
    fn checked_sub_reversed_returns_none() {
        // THE POC BUG: (1_000 - 3_500) as u64 = u64::MAX - ~2_500.
        // With `TimestampMs::checked_sub` we surface the reversal explicitly.
        let a = TimestampMs::from_millis(1_000);
        let b = TimestampMs::from_millis(3_500);
        assert_eq!(a.checked_sub(b), None);
    }

    #[test]
    fn checked_sub_equal_is_zero() {
        let t = TimestampMs::from_millis(5);
        assert_eq!(t.checked_sub(t), Some(DurationMs::from_millis(0)));
    }

    #[test]
    fn try_from_i64_rejects_negative() {
        assert_eq!(DurationMs::try_from_i64(-1), None);
        assert_eq!(
            DurationMs::try_from_i64(0),
            Some(DurationMs::from_millis(0))
        );
    }

    #[test]
    fn is_negative_flag() {
        assert!(TimestampMs::from_millis(-1).is_negative());
        assert!(!TimestampMs::from_millis(0).is_negative());
    }

    #[test]
    fn ordering_is_numeric() {
        assert!(TimestampMs::from_millis(1) < TimestampMs::from_millis(2));
    }
}
