// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `ArgvShape` — normalized, grouping-stable form of an argv vector.
//!
//! The shape is opaque: only the argv normalizer constructs one, and it does
//! so by applying the rules in `log_stats_design.md` §6.1. Other code cannot
//! fabricate an `ArgvShape` from a raw string, which prevents accidental
//! mixing of normalized and unnormalized argv anywhere in the aggregator.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArgvShape(String);

impl ArgvShape {
    /// Only callable by the argv normalizer. The `crate::` visibility keeps
    /// construction private to `o8v-core`; the argv normalizer lives in `o8v`
    /// today, so until that moves in we expose `from_normalized_string` —
    /// see NOTE.
    ///
    /// NOTE: The argv normalizer currently lives in `o8v/src/aggregator.rs`.
    /// In Layer 2 it moves into `o8v/src/aggregator/argv_shape.rs` and this
    /// constructor is the only call site. Until then, the name advertises
    /// the invariant even though Rust cannot enforce it across crates.
    pub fn from_normalized_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for ArgvShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_inner_string() {
        let s = ArgvShape::from_normalized_string("write <path> --find <str> --replace <str>");
        assert_eq!(format!("{s}"), "write <path> --find <str> --replace <str>");
    }

    #[test]
    fn equality_is_string_equality() {
        let a = ArgvShape::from_normalized_string("read <path>");
        let b = ArgvShape::from_normalized_string("read <path>");
        let c = ArgvShape::from_normalized_string("read <abs>");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
