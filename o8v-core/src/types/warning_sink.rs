// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `WarningSink` — collects [`Warning`]s in emission order.
//!
//! Passed down through the pipeline so any layer that discovers a warning
//! appends it directly. Replaces the `(_, Vec<Warning>)` tuple pattern.
//!
//! Invariant: once a warning is pushed, it cannot be dropped. The only way
//! to obtain the collected warnings is to consume the sink via [`into_inner`].
//!
//! [`into_inner`]: WarningSink::into_inner

use super::Warning;

/// Collects `Warning`s in emission order. Passed down through the pipeline
/// so any layer that discovers a warning appends it directly. Replaces the
/// `(_, Vec<Warning>)` tuple pattern.
///
/// Invariant: once a warning is pushed, it cannot be dropped. The only way
/// to remove warnings is to replace the sink (tests do this via creating a
/// fresh `WarningSink::new()`).
#[derive(Debug, Default)]
pub struct WarningSink {
    warnings: Vec<Warning>,
}

impl WarningSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, w: Warning) {
        self.warnings.push(w);
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = Warning>) {
        self.warnings.extend(iter);
    }

    /// Consume the sink and return the collected warnings.
    pub fn into_inner(self) -> Vec<Warning> {
        self.warnings
    }

    pub fn as_slice(&self) -> &[Warning] {
        &self.warnings
    }

    pub fn len(&self) -> usize {
        self.warnings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.warnings.is_empty()
    }
}
