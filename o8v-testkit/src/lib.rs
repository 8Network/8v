// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Test infrastructure for 8v's check pipeline.
//!
//! Provides fixture resolution, check execution, EXPECTED.toml parsing,
//! diagnostic queries, and assertion helpers. Used by integration tests
//! across o8v-check, o8v-core, and o8v.

mod assert;
pub mod benchmark;
mod binary;
mod diagnostic;
mod expected;
mod fixture;
mod integration;
pub mod json;
mod query;
pub mod release_server;
mod run;
mod sanitize;
mod scaffold;

pub use assert::{
    assert_error, assert_failed, assert_parse_status, assert_parsed_diagnostics, assert_passed,
};
pub use binary::{bin_path, run_bin};
pub use expected::{Expected, ExpectedCheck, ExpectedDiagnostic};
pub use fixture::Fixture;
pub use integration::{
    assert_e2e, assert_expected, assert_no_detection_errors, assert_project_count,
};
pub use json::{JsonCheck, JsonDiagnostic, JsonLocation, JsonOutput, JsonResult, JsonSummary};
// Re-export commonly used types so test modules don't need extra imports.
pub use diagnostic::DiagnosticBuilder;
pub use o8v_core::diagnostic::{ParseStatus, Severity};
pub use o8v_core::project::Stack;
pub use query::{all_check_names, collect_diagnostics, find_entry, find_result, has_check};
pub use release_server::ReleaseTestServer;
pub use run::{run_check, run_check_interrupted, run_check_path};
pub use sanitize::assert_sanitized;
pub use scaffold::{fixture_path, TempProject};

/// Generate a standard e2e violation test.
#[macro_export]
macro_rules! e2e_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let fixture = $crate::Fixture::e2e($fixture);
            let expected = $crate::Expected::load(&fixture);
            $crate::assert_e2e(&fixture, &expected);
        }
    };
    (#[ignore] $name:ident, $fixture:expr) => {
        #[test]
        #[ignore]
        fn $name() {
            let fixture = $crate::Fixture::e2e($fixture);
            let expected = $crate::Expected::load(&fixture);
            $crate::assert_e2e(&fixture, &expected);
        }
    };
}

#[cfg(test)]
mod tests;
