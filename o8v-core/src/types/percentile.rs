// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `Percentile` — validated fraction in [0.0, 1.0].
//!
//! Replaces the raw `f64` accepted by `Histogram::percentile` in the POC,
//! which allowed 1.5 or NaN through to produce nonsensical output.

use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Percentile(f64);

#[derive(Debug, thiserror::Error)]
#[error("percentile must be in [0.0, 1.0] and not NaN, got {0}")]
pub struct PercentileOutOfRange(pub f64);

impl Percentile {
    pub const P50: Self = Self(0.50);
    pub const P95: Self = Self(0.95);
    pub const P99: Self = Self(0.99);

    pub fn new(p: f64) -> Result<Self, PercentileOutOfRange> {
        if p.is_nan() || !(0.0..=1.0).contains(&p) {
            Err(PercentileOutOfRange(p))
        } else {
            Ok(Self(p))
        }
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Percentile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", (self.0 * 100.0).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_valid() {
        assert_eq!(Percentile::P50.value(), 0.50);
        assert_eq!(Percentile::P95.value(), 0.95);
        assert_eq!(Percentile::P99.value(), 0.99);
    }

    #[test]
    fn new_rejects_out_of_range() {
        assert!(Percentile::new(-0.01).is_err());
        assert!(Percentile::new(1.01).is_err());
        assert!(Percentile::new(1.5).is_err());
    }

    #[test]
    fn new_rejects_nan() {
        assert!(Percentile::new(f64::NAN).is_err());
    }

    #[test]
    fn new_accepts_boundaries() {
        assert!(Percentile::new(0.0).is_ok());
        assert!(Percentile::new(1.0).is_ok());
    }

    #[test]
    fn display_is_integer_percent() {
        assert_eq!(format!("{}", Percentile::P50), "p50");
        assert_eq!(format!("{}", Percentile::P95), "p95");
        assert_eq!(format!("{}", Percentile::P99), "p99");
    }
}
